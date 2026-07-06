use std::io::{self, ErrorKind};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixStream;
use tracing::{error, trace};

use crate::handlers::{CommandCtx, get_registry};
use crate::protocol::ProtoWrite;
use crate::rules::RuleSet;

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
    real_socket: &str,
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
            real_socket,
        };
        handler.handle(ctx).await?;
    } else {
        trace!(" [debug] Transparent proxy for unsupported command: {cmd_name}");
        let mut netd = UnixStream::connect(real_socket).await.map_err(|e| {
            error!("connect real netd: {e}");
            e
        })?;
        netd.write_cmd(&cmd).await?;
        proxy_transparent(&mut client, &mut netd).await?;
    }

    Ok(())
}
