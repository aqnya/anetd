use arc_swap::ArcSwap;
use std::io::{self, ErrorKind};
use std::net::SocketAddr;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener as StdUnixListener;
use std::sync::Arc;
use tokio::net::UnixListener;
use tracing::{error, info, warn};

use crate::cli::Args;
use crate::config::{PROXY_SOCKET, REAL_SOCKET};
use crate::dns_server;
use crate::rules::{RuleSet, load_rules, spawn_reload_watcher};
use crate::session::handle_client;
use crate::signal::{ORIGINAL_SOCKET_RENAMED, setup_signals};

use std::ffi::CString;

fn set_selinux_context(path: &str, value: &[u8]) -> io::Result<()> {
    let path = CString::new(path).unwrap();
    let name = CString::new("security.selinux").unwrap();

    let ret = unsafe {
        libc::setxattr(
            path.as_ptr(),
            name.as_ptr(),
            value.as_ptr().cast(),
            value.len(),
            0,
        )
    };

    if ret == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn setup_socket_permissions() -> io::Result<()> {
    #[cfg(unix)]
    {
        set_selinux_context(PROXY_SOCKET, b"u:object_r:dnsproxyd_socket:s0")?;
        std::fs::set_permissions(PROXY_SOCKET, std::fs::Permissions::from_mode(0o660))?;
        unsafe {
            let c_path = std::ffi::CString::new(PROXY_SOCKET).unwrap();
            libc::chown(c_path.as_ptr(), 0, 3003);
        }
    }
    Ok(())
}

pub async fn init(args: &Args) -> io::Result<()> {
    setup_signals();

    static RULES: std::sync::OnceLock<ArcSwap<RuleSet>> = std::sync::OnceLock::new();
    let store = RULES.get_or_init(|| ArcSwap::new(Arc::new(load_rules(&args.rules))));
    spawn_reload_watcher(args.rules.clone(), store);

    if args.dns_server {
        // Built-in DNS server mode (UDP/TCP on the specified port)
        let bind_addr: SocketAddr = format!("0.0.0.0:{}", args.dns_port)
            .parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let upstream: SocketAddr = args
            .dns_upstream
            .parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

        info!(
            "Starting DNS server on port {}, upstream {}",
            args.dns_port, args.dns_upstream
        );
        dns_server::run(bind_addr, upstream, store).await?;
    } else {
        // Socket-hijacking mode: intercept netd dnsproxyd socket
        if std::path::Path::new(REAL_SOCKET).exists() {
            warn!("{} already exists, removing", REAL_SOCKET);
            std::fs::remove_file(REAL_SOCKET)?;
        }

        std::fs::rename(PROXY_SOCKET, REAL_SOCKET)?;
        ORIGINAL_SOCKET_RENAMED.store(true, std::sync::atomic::Ordering::SeqCst);

        let std_listener = StdUnixListener::bind(PROXY_SOCKET)?;
        setup_socket_permissions()?;

        std_listener.set_nonblocking(true)?;
        let listener = UnixListener::from_std(std_listener)?;

        info!("listening on {PROXY_SOCKET}");
        info!("[*] forwarding to {REAL_SOCKET}");

        loop {
            match listener.accept().await {
                Ok((client, _addr)) => {
                    let rules = store.load_full();
                    tokio::spawn(async move {
                        if let Err(e) = handle_client(client, rules).await {
                            if e.kind() != ErrorKind::UnexpectedEof
                                && e.kind() != ErrorKind::BrokenPipe
                                && e.kind() != ErrorKind::ConnectionReset
                            {
                                error!("[client error] {e}");
                            }
                        }
                    });
                }
                Err(e) => error!("[accept error] {e}"),
            }
        }
    }

    Ok(())
}
