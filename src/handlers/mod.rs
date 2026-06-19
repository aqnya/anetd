use std::collections::HashMap;
use std::io;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, OnceLock};

use crate::rules::FilterRule;

pub mod getaddrinfo;
pub mod gethostbyaddr;
pub mod gethostbyname;
pub mod resnsend;
pub mod setoperatoraddress;

/// 传递给处理函数的上下文
pub struct CommandCtx<'a> {
    pub client: &'a mut UnixStream,
    pub cmd_line: &'a str,
    pub rules: Arc<Vec<FilterRule>>,
}

/// 统一的命令处理器接口
pub trait CommandHandler: Send + Sync {
    fn handle(&self, ctx: CommandCtx) -> io::Result<()>;
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
        m.insert(
            "setoperatoraddress".to_string().to_lowercase(),
            Box::new(setoperatoraddress::SetOperatorAddressHandler),
        );
        m
    })
}
