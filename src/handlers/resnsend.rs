use base64::{Engine, prelude::BASE64_STANDARD};
use log::{info, trace};
use std::io;

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

            let net_id = tokens[1];
            let flags = tokens[2];
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
                    crate::dns::send_resnsend_block(client, &raw_dns).await?;
                    info!("[BLOCKED] cmd: \"{}\"", cmd_line.trim());
                }
                FilterAction::Redirect(target) => {
                    if let Some(modified_dns) = modify_dns_query_name(&raw_dns, target) {
                        let new_b64 = BASE64_STANDARD.encode(&modified_dns);
                        let new_cmd = format!("resnsend {net_id} {flags} {new_b64}");
                        info!(" REDIRECT to {target} (resnsend)");
                        let mut netd = connect_netd().await?;
                        netd.write_cmd(&new_cmd).await?;
                        proxy_transparent(client, &mut netd).await?;
                    } else {
                        let mut netd = connect_netd().await?;
                        netd.write_cmd(cmd_line).await?;
                        proxy_transparent(client, &mut netd).await?;
                    }
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

fn modify_dns_query_name(packet: &[u8], new_name: &str) -> Option<Vec<u8>> {
    if packet.len() < 12 {
        return None;
    }
    let mut pos = 12;
    loop {
        if pos >= packet.len() {
            return None;
        }
        let len = packet[pos] as usize;
        if len == 0 {
            pos += 1;
            break;
        }
        pos += 1 + len;
    }
    if pos + 4 > packet.len() {
        return None;
    }
    let tail = &packet[pos..pos + 4];

    let mut new_packet = packet[0..12].to_vec();
    for label in new_name.split('.') {
        if label.is_empty() {
            continue;
        }
        new_packet.push(label.len() as u8);
        new_packet.extend_from_slice(label.as_bytes());
    }
    new_packet.push(0);
    new_packet.extend_from_slice(tail);
    Some(new_packet)
}
