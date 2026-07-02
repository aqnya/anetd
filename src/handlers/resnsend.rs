use base64::{Engine, prelude::BASE64_STANDARD};
use std::io;
use tracing::{info, trace};

use crate::dns::proto::resnsend;
use crate::handlers::{CommandCtx, CommandHandler};
use crate::protocol::ProtoWrite;
use crate::proxy::{connect_netd, proxy_transparent};
use crate::rules::FilterAction;

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
            } = ctx;

            let tokens: Vec<&str> = cmd_line.split_whitespace().collect();
            if tokens.len() < 4 {
                let mut netd = connect_netd().await?;
                netd.write_cmd(cmd_line).await?;
                return proxy_transparent(client, &mut netd).await;
            }

            let _net_id = tokens[1];
            let _flags = tokens[2];
            let b64_query = tokens[3];

            let Ok(raw_dns) = BASE64_STANDARD.decode(b64_query) else {
                let mut netd = connect_netd().await?;
                netd.write_cmd(cmd_line).await?;
                return proxy_transparent(client, &mut netd).await;
            };

            let Some(hostname) = parse_dns_query_name(&raw_dns) else {
                let mut netd = connect_netd().await?;
                netd.write_cmd(cmd_line).await?;
                return proxy_transparent(client, &mut netd).await;
            };

            trace!("  hostname (resnsend): {hostname}");

            let mut pseudo_url = String::with_capacity(9 + hostname.len());
            pseudo_url.push_str("https://");
            pseudo_url.push_str(&hostname);
            pseudo_url.push('/');

            let action = rules.matches(&pseudo_url, &hostname, "other");

            match &action {
                FilterAction::Block => {
                    resnsend::send_block(client, &raw_dns).await?;
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

fn parse_dns_query_name(packet: &[u8]) -> Option<String> {
    if packet.len() < 12 {
        return None;
    }
    let mut pos = 12;
    let mut parts = Vec::new();
    loop {
        if pos >= packet.len() {
            return None;
        }
        let len = packet[pos] as usize;
        if len == 0 {
            break;
        }
        if (len & 0xC0) != 0 {
            return None;
        }
        pos += 1;
        if pos + len > packet.len() {
            return None;
        }
        let label = std::str::from_utf8(&packet[pos..pos + len]).ok()?;
        parts.push(label);
        pos += len;
    }
    Some(parts.join("."))
}
