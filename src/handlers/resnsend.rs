// src/handlers/resnsend.rs
use base64::{Engine, prelude::BASE64_STANDARD};
use log::info;
use std::io;
use std::net::Ipv4Addr;
use std::str::FromStr;

use crate::handlers::{CommandCtx, CommandHandler};
use crate::rules::FilterAction;
use crate::server::{ProtoWrite, connect_netd, proxy_transparent};

pub struct ResNsendHandler;

impl CommandHandler for ResNsendHandler {
    fn handle(&self, ctx: CommandCtx) -> io::Result<()> {
        let CommandCtx {
            client,
            cmd_line,
            rules,
        } = ctx;

        let tokens: Vec<&str> = cmd_line.split_whitespace().collect();
        if tokens.len() < 4 {
            let mut netd = connect_netd()?;
            netd.write_cmd(cmd_line)?;
            return proxy_transparent(client, netd);
        }

        let net_id = tokens[1];
        let flags = tokens[2];
        let b64_query = tokens[3];

        let Ok(raw_dns) = BASE64_STANDARD.decode(b64_query) else {
            let mut netd = connect_netd()?;
            netd.write_cmd(cmd_line)?;
            return proxy_transparent(client, netd);
        };

        let Some(hostname) = parse_dns_query_name(&raw_dns) else {
            let mut netd = connect_netd()?;
            netd.write_cmd(cmd_line)?;
            return proxy_transparent(client, netd);
        };

        info!("  hostname (resnsend): {hostname}");
        // 修复：用 expect 明确说明规则不变量
        let rule = rules
            .iter()
            .find(|r| r.matches(&hostname))
            .expect("BUG: no catch-all rule in rules list");

        match &rule.action {
            FilterAction::Block => {
                crate::dns::send_dns_hard_block(client)?;
                info!(" BLOCKED (resnsend)");
            }
            FilterAction::Fake(ip) => {
                if let Ok(addr) = Ipv4Addr::from_str(ip) {
                    if let Some(fake_dns_packet) = make_dns_a_record_response(&raw_dns, addr) {
                        crate::dns::send_resnsend_raw_packet(client, &fake_dns_packet)?;
                        info!(" FAKE {ip} (resnsend)");
                    } else {
                        crate::dns::send_dns_hard_block(client)?;
                    }
                } else {
                    crate::dns::send_dns_hard_block(client)?;
                }
            }
            FilterAction::Redirect(target) => {
                if let Some(modified_dns) = modify_dns_query_name(&raw_dns, target) {
                    let new_b64 = BASE64_STANDARD.encode(modified_dns);
                    let new_cmd = format!("resnsend {net_id} {flags} {new_b64}");
                    info!(" REDIRECT to {target} (resnsend)");
                    let mut netd = connect_netd()?;
                    netd.write_cmd(&new_cmd)?;
                    proxy_transparent(client, netd)?;
                } else {
                    let mut netd = connect_netd()?;
                    netd.write_cmd(cmd_line)?;
                    proxy_transparent(client, netd)?;
                }
            }
            FilterAction::Allow => {
                let mut netd = connect_netd()?;
                netd.write_cmd(cmd_line)?;
                proxy_transparent(client, netd)?;
            }
        }

        Ok(())
    }
}

fn parse_dns_query_name(packet: &[u8]) -> Option<String> {
    if packet.len() < 12 {
        return None;
    }
    let mut pos = 12;
    let mut labels = Vec::new();

    loop {
        if pos >= packet.len() {
            return None;
        }
        let len = packet[pos] as usize;
        if len == 0 {
            break;
        }
        if (len & 0xC0) == 0xC0 {
            return None;
        }
        if len > 63 {
            return None;
        }
        pos += 1;
        if pos + len > packet.len() {
            return None;
        }
        let label = std::str::from_utf8(&packet[pos..pos + len]).ok()?;
        labels.push(label);
        pos += len;
    }
    Some(labels.join("."))
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

fn make_dns_a_record_response(query: &[u8], ip: Ipv4Addr) -> Option<Vec<u8>> {
    if query.len() < 12 {
        return None;
    }
    let mut ans = query[0..12].to_vec();
    ans[2] = 0x81;
    ans[3] = 0x80; // Flags: Standard response, No error
    ans[6] = 0;
    ans[7] = 1; // Answer count = 1

    ans[8] = 0;
    ans[9] = 0;
    ans[10] = 0;
    ans[11] = 0;

    let mut pos = 12;
    loop {
        if pos >= query.len() {
            break;
        }
        let len = query[pos] as usize;
        if len == 0 {
            pos += 1;
            break;
        }
        pos += 1 + len;
    }
    if pos + 4 > query.len() {
        return None;
    }
    ans.extend_from_slice(&query[12..pos + 4]);

    ans.extend_from_slice(&[0xC0, 0x0C]);
    ans.extend_from_slice(&[0x00, 0x01]);
    ans.extend_from_slice(&[0x00, 0x01]);
    ans.extend_from_slice(&[0x00, 0x00, 0x00, 0x05]);
    ans.extend_from_slice(&[0x00, 0x04]);
    ans.extend_from_slice(&ip.octets());

    Some(ans)
}
