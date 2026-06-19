use log::info;
use std::io;
use std::net::Ipv4Addr;
use std::str::FromStr;

use crate::handlers::{CommandCtx, CommandHandler};
use crate::rules::FilterAction;
use crate::server::{ProtoWrite, connect_netd, proxy_transparent};

pub struct GetHostByNameHandler;

impl CommandHandler for GetHostByNameHandler {
    fn handle(&self, ctx: CommandCtx) -> io::Result<()> {
        let CommandCtx {
            client,
            cmd_line,
            rules,
        } = ctx;

        let tokens: Vec<&str> = cmd_line.split_whitespace().collect();
        // 格式：gethostbyname <net_id> <hostname> <ai_family>
        if tokens.len() < 3 {
            let mut netd = connect_netd()?;
            netd.write_cmd(cmd_line)?;
            return proxy_transparent(client, netd);
        }

        let net_id = tokens[1];
        let hostname = tokens[2];
        let ai_family = tokens.get(3).unwrap_or(&"2"); // 默认 AF_INET = 2

        info!("  hostname (gethostbyname): {hostname}");
        let rule = rules
            .iter()
            .find(|r| r.matches(hostname))
            .expect("BUG: no catch-all rule in rules list");

        match &rule.action {
            FilterAction::Block => {
                crate::dns::send_dns_hard_block(client)?;
                info!(" BLOCKED (gethostbyname)");
            }
            FilterAction::Fake(ip) => {
                if let Ok(addr) = Ipv4Addr::from_str(ip) {
                    crate::dns::send_hostent_fake_response(client, hostname, addr)?;
                    info!(" FAKE {ip} (gethostbyname)");
                } else {
                    crate::dns::send_dns_hard_block(client)?;
                }
            }
            FilterAction::Redirect(target) => {
                let new_cmd = format!("gethostbyname {net_id} {target} {ai_family}");
                info!(" REDIRECT to {target} (gethostbyname)");
                let mut netd = connect_netd()?;
                netd.write_cmd(&new_cmd)?;
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
