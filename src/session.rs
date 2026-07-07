use std::io::{self, ErrorKind};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::net::UnixStream;
use tracing::{error, trace};

use crate::handlers::{CommandCtx, get_registry};
use crate::protocol::ProtoWrite;
use crate::rules::RuleSet;

/// Maximum length of a dnsproxyd command line, including the NUL terminator.
/// Prevents memory-exhaustion DoS from a client that never sends a NUL byte.
const MAX_CMD_LINE: usize = 8192;

/// Read a NUL-terminated command line from `reader`, enforcing `MAX_CMD_LINE`.
///
/// Returns the raw bytes (without the NUL terminator) on success, or an error
/// if the input exceeds the limit or the reader produces an I/O error.
async fn read_cmd_line<R: AsyncRead + Unpin>(reader: &mut R) -> io::Result<Vec<u8>> {
    let mut raw = Vec::with_capacity(128);
    let mut byte = [0u8; 1];
    loop {
        if raw.len() >= MAX_CMD_LINE {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                format!("command line too long (max {MAX_CMD_LINE} bytes)"),
            ));
        }
        match reader.read(&mut byte).await {
            Ok(0) => break, // EOF
            Ok(_) => {
                if byte[0] == 0 {
                    break;
                }
                raw.push(byte[0]);
            }
            Err(e) => return Err(e),
        }
    }
    Ok(raw)
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
    real_socket: &str,
) -> io::Result<()> {
    let raw = read_cmd_line(&mut client).await?;
    let cmd = String::from_utf8(raw).map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn read_cmd_line_normal() {
        // Normal NUL-terminated command
        let (mut client, mut server) = tokio::io::duplex(64);
        server.write_all(b"getaddrinfo example.com ^ 0 0 0 0 0\0").await.unwrap();
        drop(server); // close write side so read sees EOF after the message

        let raw = read_cmd_line(&mut client).await.unwrap();
        assert_eq!(
            std::str::from_utf8(&raw).unwrap(),
            "getaddrinfo example.com ^ 0 0 0 0 0"
        );
    }

    #[tokio::test]
    async fn read_cmd_line_eof_without_nul() {
        // Client closes without sending NUL — should return whatever was sent
        let (mut client, mut server) = tokio::io::duplex(64);
        server.write_all(b"getaddrinfo").await.unwrap();
        drop(server);

        let raw = read_cmd_line(&mut client).await.unwrap();
        assert_eq!(std::str::from_utf8(&raw).unwrap(), "getaddrinfo");
    }

    #[tokio::test]
    async fn read_cmd_line_empty() {
        // Just a NUL byte
        let (mut client, mut server) = tokio::io::duplex(64);
        server.write_all(b"\0").await.unwrap();
        drop(server);

        let raw = read_cmd_line(&mut client).await.unwrap();
        assert!(raw.is_empty());
    }

    #[tokio::test]
    async fn read_cmd_line_exceeds_limit() {
        // Write exactly MAX_CMD_LINE bytes without NUL → should error
        let (mut client, mut server) = tokio::io::duplex(MAX_CMD_LINE + 64);
        let payload = vec![b'A'; MAX_CMD_LINE];
        server.write_all(&payload).await.unwrap();

        let result = read_cmd_line(&mut client).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
        assert!(err.to_string().contains("too long"));
    }

    #[tokio::test]
    async fn read_cmd_line_at_limit_with_nul() {
        // MAX_CMD_LINE - 1 non-NUL bytes + NUL = exactly at limit
        let (mut client, mut server) = tokio::io::duplex(MAX_CMD_LINE + 64);
        let mut payload = vec![b'B'; MAX_CMD_LINE - 1];
        payload.push(0);
        server.write_all(&payload).await.unwrap();
        drop(server);

        let raw = read_cmd_line(&mut client).await.unwrap();
        assert_eq!(raw.len(), MAX_CMD_LINE - 1);
        assert!(raw.iter().all(|&b| b == b'B'));
    }
}
