use log::info;
use std::io;

use crate::handlers::{CommandCtx, CommandHandler};
use crate::rules::FilterAction;
use crate::server::{ProtoWrite, connect_netd, proxy_transparent};

pub struct GetHostByAddrHandler;

impl CommandHandler for GetHostByAddrHandler {
    fn handle(&self, ctx: CommandCtx) -> io::Result<()> {
        let CommandCtx {
            client,
            cmd_line,
            rules,
        } = ctx;

        let tokens: Vec<&str> = cmd_line.split_whitespace().collect();
        // 格式：gethostbyaddr <address> <length> <type> <net_id>
        if tokens.len() < 2 {
            let mut netd = connect_netd()?;
            netd.write_cmd(cmd_line)?;
            return proxy_transparent(client, netd);
        }

        let addr_str = tokens[1];
        info!("  address (gethostbyaddr): {addr_str}");

        // 反查时以 IP 作为标识匹配规则
        let rule = rules
            .iter()
            .find(|r| r.matches(addr_str))
            .expect("BUG: no catch-all rule in rules list");

        match &rule.action {
            FilterAction::Block | FilterAction::Fake(_) => {
                // 如果是 Block 或 Fake，直接响应 DNS 操作失败，不予反查
                crate::dns::send_dns_operation_failed(client)?;
                info!(" BLOCKED (gethostbyaddr)");
            }
            FilterAction::Allow | FilterAction::Redirect(_) => {
                // gethostbyaddr 的重定向一般在移动端沙箱无明显应用场景，默认放行
                let mut netd = connect_netd()?;
                netd.write_cmd(cmd_line)?;
                proxy_transparent(client, netd)?;
            }
        }
        Ok(())
    }
}
