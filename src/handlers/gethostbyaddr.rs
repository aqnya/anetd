use log::{info, trace};
use std::io;

use crate::handlers::{CommandCtx, CommandHandler};
use crate::protocol::ProtoWrite;
use crate::proxy::{connect_netd, proxy_transparent};
use crate::rules::FilterAction;

pub struct GetHostByAddrHandler;

impl CommandHandler for GetHostByAddrHandler {
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
            if tokens.len() < 2 {
                let mut netd = connect_netd().await?;
                netd.write_cmd(cmd_line).await?;
                return proxy_transparent(client, &mut netd).await;
            }

            let addr_str = tokens[1];
            trace!("  address (gethostbyaddr): {addr_str}");

            let mut pseudo_url = String::with_capacity(9 + addr_str.len());
            pseudo_url.push_str("https://");
            pseudo_url.push_str(addr_str);
            pseudo_url.push('/');

            let action = rules.matches(&pseudo_url, addr_str, "other");

            match &action {
                FilterAction::Block => {
                    crate::dns::send_dns_operation_failed(client).await?;
                    info!("[BLOCKED] cmd: \"{}\"", cmd_line.trim());
                }
                FilterAction::Allow | FilterAction::Redirect(_) => {
                    let mut netd = connect_netd().await?;
                    netd.write_cmd(cmd_line).await?;
                    proxy_transparent(client, &mut netd).await?;
                }
            }
            Ok(())
        })
    }
}
