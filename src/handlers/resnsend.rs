use base64::{Engine, prelude::BASE64_STANDARD};
use std::io;
use tokio::net::UnixStream;
use tracing::{info, trace};

use crate::dns::response::raw;
use crate::dns::wire::parse_dns_query_name;
use crate::handlers::{CommandCtx, CommandHandler, format_pseudo_url};
use crate::protocol::ProtoWrite;
use crate::rules::FilterAction;
use crate::session::proxy_transparent;

pub struct ResNsendHandler;

impl CommandHandler for ResNsendHandler {
    fn handle<'a>(
        &'a self,
        ctx: CommandCtx<'a>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = io::Result<()>> + Send + 'a>> {
        Box::pin(async move {
            let CommandCtx {
                client,
                cmd_line,
                rules,
                real_socket,
            } = ctx;

            let tokens: Vec<&str> = cmd_line.split_whitespace().collect();
            if tokens.len() < 4 {
                let mut netd = UnixStream::connect(real_socket).await?;
                netd.write_cmd(cmd_line).await?;
                return proxy_transparent(client, &mut netd).await;
            }

            let _net_id = tokens[1];
            let _flags = tokens[2];
            let b64_query = tokens[3];

            let Ok(raw_dns) = BASE64_STANDARD.decode(b64_query) else {
                trace!(
                    " [I] Failed to decode base64 in resnsend command, falling back to transparent proxy"
                );
                let mut netd = UnixStream::connect(real_socket).await?;
                netd.write_cmd(cmd_line).await?;
                return proxy_transparent(client, &mut netd).await;
            };

            let Some(hostname) = parse_dns_query_name(&raw_dns) else {
                trace!(
                    " [I] Failed to parse DNS query name in resnsend command, falling back to transparent proxy"
                );
                let mut netd = UnixStream::connect(real_socket).await?;
                netd.write_cmd(cmd_line).await?;
                return proxy_transparent(client, &mut netd).await;
            };

            trace!("  hostname (resnsend): {hostname}");

            let pseudo_url = format_pseudo_url(&hostname);

            let action = rules.matches(&pseudo_url, &hostname, "other");

            match &action {
                FilterAction::Block => {
                    raw::send_block(client, &raw_dns).await?;
                    info!("[BLOCKED] cmd: \"{}\"", cmd_line.trim());
                }
                FilterAction::Allow => {
                    let mut netd = UnixStream::connect(real_socket).await?;
                    netd.write_cmd(cmd_line).await?;
                    return proxy_transparent(client, &mut netd).await;
                }
            }
            Ok(())
        })
    }
}
