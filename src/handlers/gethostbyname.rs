use std::io;
use tokio::net::UnixStream;
use tracing::{info, trace};

use crate::dns::response::addrinfo;
use crate::handlers::{CommandCtx, CommandHandler, format_pseudo_url};
use crate::protocol::ProtoWrite;
use crate::rules::FilterAction;
use crate::session::proxy_transparent;

pub struct GetHostByNameHandler;

impl CommandHandler for GetHostByNameHandler {
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
            if tokens.len() < 3 {
                let mut netd = UnixStream::connect(real_socket).await?;
                netd.write_cmd(cmd_line).await?;
                return proxy_transparent(client, &mut netd).await;
            }

            let _net_id = tokens[1];
            let hostname = tokens[2];
            let _ai_family = tokens.get(3).unwrap_or(&"2");

            trace!("  hostname (gethostbyname): {hostname}");

            let pseudo_url = format_pseudo_url(hostname);

            let action = rules.matches(&pseudo_url, hostname, "other");

            match &action {
                FilterAction::Block => {
                    addrinfo::send_nxdomain(client).await?;
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
