// src/handlers/getaddrinfo.rs
use log::info;
use std::io;
use std::net::Ipv4Addr;
use std::str::FromStr;

use crate::handlers::{CommandCtx, CommandHandler};
use crate::rules::FilterAction;
use crate::server::{ProtoWrite, connect_netd, proxy_transparent};

// ... Request 结构体和实现保持不变 ...
#[derive(Debug, Clone)]
struct GetAddrInfoRequest {
    hostname: Option<String>,
    servname: Option<String>,
    ai_flags: i32,
    ai_family: i32,
    ai_socktype: i32,
    ai_protocol: i32,
    net_id: u32,
}

impl GetAddrInfoRequest {
    fn parse(cmd: &str) -> Option<Self> {
        let tokens: Vec<&str> = cmd.split(' ').collect();
        if tokens.len() != 8 {
            return None;
        }
        if !tokens[0].eq_ignore_ascii_case("getaddrinfo") {
            return None;
        }
        let tok = |s: &str| if s == "^" { None } else { Some(s.to_string()) };
        Some(Self {
            hostname: tok(tokens[1]),
            servname: tok(tokens[2]),
            ai_flags: tokens[3].parse().ok()?,
            ai_family: tokens[4].parse().ok()?,
            ai_socktype: tokens[5].parse().ok()?,
            ai_protocol: tokens[6].parse().ok()?,
            net_id: tokens[7].parse().ok()?,
        })
    }

    fn to_cmd(&self) -> String {
        format!(
            "getaddrinfo {} {} {} {} {} {} {}",
            self.hostname.as_deref().unwrap_or("^"),
            self.servname.as_deref().unwrap_or("^"),
            self.ai_flags,
            self.ai_family,
            self.ai_socktype,
            self.ai_protocol,
            self.net_id,
        )
    }

    fn hostname_str(&self) -> &str {
        self.hostname.as_deref().unwrap_or("(null)")
    }
}

pub struct GetAddrInfoHandler;

impl CommandHandler for GetAddrInfoHandler {
    fn handle(&self, ctx: CommandCtx) -> io::Result<()> {
        let CommandCtx {
            client,
            cmd_line,
            rules,
        } = ctx;

        let Some(req) = GetAddrInfoRequest::parse(cmd_line) else {
            info!(" [I] Failed to parse getaddrinfo command, falling back to transparent proxy");
            let mut netd = connect_netd()?;
            netd.write_cmd(cmd_line)?;
            return proxy_transparent(client, netd);
        };

        let hostname = req.hostname_str();
        info!("  hostname: {hostname}");

        // 修复：用 expect 明确说明规则不变量
        let rule = rules
            .iter()
            .find(|r| r.matches(hostname))
            .expect("BUG: no catch-all rule in rules list");

        match &rule.action {
            FilterAction::Block => {
                crate::dns::send_dns_hard_block(client)?;
                info!(" BLOCKED (getaddrinfo)");
            }
            FilterAction::Fake(ip) => {
                if let Ok(addr) = Ipv4Addr::from_str(ip) {
                    crate::dns::send_addrinfo_fake_response(client, addr)?;
                    info!(" FAKE {ip} (getaddrinfo)");
                } else {
                    crate::dns::send_dns_hard_block(client)?;
                    info!(" FAKE FAILED (invalid IP), BLOCKED (getaddrinfo)");
                }
            }
            FilterAction::Redirect(target) => {
                let mut new_req = req.clone();
                new_req.hostname = Some(target.clone());
                info!(" REDIRECT to {target}");
                let mut netd = connect_netd()?;
                netd.write_cmd(&new_req.to_cmd())?;
                proxy_transparent(client, netd)?;
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
