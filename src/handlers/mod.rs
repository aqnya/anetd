use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use std::sync::atomic::AtomicU64;
use tokio::net::UnixStream;

use crate::rules::RuleSet;

pub mod getaddrinfo;
pub mod gethostbyname;
pub mod resnsend;

/// Global counter: total blocked DNS/hostname requests.
pub static BLOCKED_COUNT: AtomicU64 = AtomicU64::new(0);
/// Global counter: total DNS queries served (used in DNS server mode).
pub static DNS_QUERIES: AtomicU64 = AtomicU64::new(0);

pub struct CommandCtx<'a> {
    pub client: &'a mut UnixStream,
    pub cmd_line: &'a str,
    pub rules: Arc<RuleSet>,
    pub real_socket: &'a str,
}

pub trait CommandHandler: Send + Sync {
    fn handle<'a>(
        &'a self,
        ctx: CommandCtx<'a>,
    ) -> Pin<Box<dyn Future<Output = io::Result<()>> + Send + 'a>>;
}

type HandlerRegistry = HashMap<String, Box<dyn CommandHandler>>;

pub fn get_registry() -> &'static HandlerRegistry {
    static REGISTRY: OnceLock<HandlerRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut m: HandlerRegistry = HashMap::new();

        m.insert(
            "getaddrinfo".to_string(),
            Box::new(getaddrinfo::GetAddrInfoHandler),
        );
        m.insert("resnsend".to_string(), Box::new(resnsend::ResNsendHandler));
        m.insert(
            "gethostbyname".to_string(),
            Box::new(gethostbyname::GetHostByNameHandler),
        );
        m
    })
}

/// Build a pseudo-URL from a hostname for adblock rule matching.
/// Format: "https://<hostname>/"
#[inline]
pub fn format_pseudo_url(hostname: &str) -> String {
    let mut url = String::with_capacity(9 + hostname.len());
    url.push_str("https://");
    url.push_str(hostname);
    url.push('/');
    url
}
