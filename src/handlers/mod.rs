use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use tokio::net::UnixStream;

use crate::rules::RuleSet;

pub mod getaddrinfo;
pub mod gethostbyaddr;
pub mod gethostbyname;
pub mod resnsend;

pub struct CommandCtx<'a> {
    pub client: &'a mut UnixStream,
    pub cmd_line: &'a str,
    pub rules: Arc<RuleSet>,
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
        m.insert(
            "gethostbyaddr".to_string(),
            Box::new(gethostbyaddr::GetHostByAddrHandler),
        );
        m
    })
}
