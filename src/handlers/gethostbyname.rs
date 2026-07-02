use std::io;
use tracing::{info, trace};

use crate::dns::proto::getaddrinfo;
use crate::handlers::{CommandCtx, CommandHandler};
use crate::protocol::ProtoWrite;
use crate::proxy::{connect_netd, proxy_transparent};
use crate::rules::FilterAction;

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
            } = ctx;

            let tokens: Vec<&str> = cmd_line.split_whitespace().collect();
            if tokens.len() < 3 {
                let mut netd = connect_netd().await?;
                netd.write_cmd(cmd_line).await?;
                return proxy_transparent(client, &mut netd).await;
            }

            let _net_id = tokens[1];
            let hostname = tokens[2];
            let _ai_family = tokens.get(3).unwrap_or(&"2");

            trace!("  hostname (gethostbyname): {hostname}");

            let mut pseudo_url = String::with_capacity(9 + hostname.len());
            pseudo_url.push_str("https://");
            pseudo_url.push_str(hostname);
            pseudo_url.push('/');

            let action = rules.matches(&pseudo_url, hostname, "other");

            match &action {
                FilterAction::Block => {
                    getaddrinfo::send_nxdomain(client).await?;
                    info!("[BLOCKED] cmd: \"{}\"", cmd_line.trim());
                }
                FilterAction::Allow => {
                    let mut netd = connect_netd().await?;
                    netd.write_cmd(cmd_line).await?;
                    proxy_transparent(client, &mut netd).await?;
                }
            }

            Ok(())
        })
    }
}
