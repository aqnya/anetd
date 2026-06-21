use arc_swap::ArcSwap;
use log::{error, info, warn};
use std::io::{self, ErrorKind};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener as StdUnixListener;
use std::sync::Arc;
use tokio::net::UnixListener;

use crate::proxy::handle_client;
use crate::rules::{RuleSet, load_rules, spawn_reload_watcher};
use crate::signal::{ORIGINAL_SOCKET_RENAMED, PROXY_SOCKET, REAL_SOCKET, setup_signals};

fn setup_socket_permissions() -> io::Result<()> {
    #[cfg(unix)]
    {
        xattr::set(
            PROXY_SOCKET,
            "security.selinux",
            b"u:object_r:dnsproxyd_socket:s0",
        )?;
        std::fs::set_permissions(PROXY_SOCKET, std::fs::Permissions::from_mode(0o660))?;
        unsafe {
            let c_path = std::ffi::CString::new(PROXY_SOCKET).unwrap();
            libc::chown(c_path.as_ptr(), 0, 3003);
        }
    }
    Ok(())
}

pub async fn init(rules_path: String) -> io::Result<()> {
    setup_signals();

    static RULES: std::sync::OnceLock<ArcSwap<RuleSet>> = std::sync::OnceLock::new();
    let store = RULES.get_or_init(|| ArcSwap::new(Arc::new(load_rules(&rules_path.clone()))));
    spawn_reload_watcher(rules_path.clone(), store);

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
