use std::io::{self, ErrorKind};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::Mutex;
use tracing::{error, trace};

use crate::handlers::{CommandCtx, get_registry};
use crate::protocol::ProtoWrite;
use crate::rules::RuleSet;

/// A simple pool of pre-connected Unix sockets to the real netd service.
///
/// Reusing connections avoids the per-request socket creation overhead
/// (socket, connect, close syscalls) — a modest but measurable battery
/// saving on Android.
pub struct NetdPool {
    path: String,
    pool: Mutex<Vec<UnixStream>>,
    max_size: usize,
}

impl NetdPool {
    pub fn new(path: &str, max_size: usize) -> Self {
        Self {
            path: path.to_string(),
            pool: Mutex::new(Vec::with_capacity(max_size)),
            max_size,
        }
    }

    /// Acquire a connection from the pool, or create a new one if the pool
    /// is empty and hasn't reached capacity.
    pub async fn acquire(&self) -> io::Result<UnixStream> {
        {
            let mut pool = self.pool.lock().await;
            if let Some(stream) = pool.pop() {
                // Verify the connection is still alive.
                if stream.writable().await.is_ok() {
                    return Ok(stream);
                }
                // Dead connection — discard and continue.
            }
        }
        // No available connection — create a new one.
        UnixStream::connect(&self.path).await.map_err(|e| {
            error!("connect real netd: {e}");
            e
        })
    }

    /// Drop all pooled connections.  Called on network change because netd
    /// may restart or reinitialize, invalidating previously connected sockets.
    pub async fn invalidate_all(&self) {
        self.pool.lock().await.clear();
    }

    /// Return a connection to the pool for reuse, or drop it if the pool
    /// is full.
    pub async fn release(&self, stream: UnixStream) {
        let mut pool = self.pool.lock().await;
        if pool.len() < self.max_size {
            pool.push(stream);
        }
        // If full, stream is dropped (closed).
    }
}

pub(crate) async fn proxy_transparent(
    client: &mut UnixStream,
    netd: &mut UnixStream,
) -> io::Result<()> {
    tokio::io::copy_bidirectional(client, netd).await?;
    Ok(())
}

pub async fn handle_client(
    mut client: UnixStream,
    rules: Arc<RuleSet>,
    pool: &NetdPool,
) -> io::Result<()> {
    let mut buf_reader = BufReader::with_capacity(512, &mut client);
    let mut raw = Vec::with_capacity(128);
    buf_reader.read_until(0, &mut raw).await?;

    if raw.last() == Some(&0) {
        raw.pop();
    }
    let cmd = String::from_utf8(raw).map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
    drop(buf_reader);

    let cmd_name = cmd.split_whitespace().next().unwrap_or("").to_lowercase();
    let registry = get_registry();

    if let Some(handler) = registry.get(&cmd_name) {
        let ctx = CommandCtx {
            client: &mut client,
            cmd_line: &cmd,
            rules,
            pool,
        };
        handler.handle(ctx).await?;
    } else {
        trace!(" [debug] Transparent proxy for unsupported command: {cmd_name}");
        let mut netd = pool.acquire().await?;
        netd.write_cmd(&cmd).await?;
        proxy_transparent(&mut client, &mut netd).await?;
        pool.release(netd).await;
    }

    Ok(())
}
