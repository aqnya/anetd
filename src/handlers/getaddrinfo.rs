use std::io;
use tracing::{info, trace};

use crate::handlers::{CommandCtx, CommandHandler};
use crate::rules::FilterAction;

use crate::dns::proto::getaddrinfo;
use crate::protocol::ProtoWrite;
use crate::proxy::{connect_netd, proxy_transparent};

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct GetAddrInfoRequest {
    hostname: Option<String>,
    servname: Option<String>,
    ai_flags: i32,
    ai_family: i32,
    ai_socktype: i32,
    ai_protocol: i32,
    net_id: u32,
    raw_cmd: String,
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
            raw_cmd: cmd.to_string(),
        })
    }

    fn hostname_str(&self) -> &str {
        self.hostname.as_deref().unwrap_or("")
    }
}

pub struct GetAddrInfoHandler;

impl CommandHandler for GetAddrInfoHandler {
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

            let Some(req) = GetAddrInfoRequest::parse(cmd_line) else {
                trace!(
                    " [I] Failed to parse getaddrinfo command, falling back to transparent proxy"
                );
                let mut netd = connect_netd().await?;
                netd.write_cmd(cmd_line).await?;
                return proxy_transparent(client, &mut netd).await;
            };

            let hostname = req.hostname_str();

            let pseudo_url = format_pseudo_url(hostname);
            let action = rules.matches(&pseudo_url, "", "");

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

#[inline]
fn format_pseudo_url(hostname: &str) -> String {
    let mut url = String::with_capacity(9 + hostname.len());
    url.push_str("https://");
    url.push_str(hostname);
    url.push('/');
    url
}
