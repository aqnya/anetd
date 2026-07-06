use arc_swap::ArcSwap;
use std::io::{self, ErrorKind};
use std::net::SocketAddr;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener as StdUnixListener;
use std::sync::Arc;
use tokio::net::UnixListener;
use tokio::time::{Duration, sleep};
use tracing::{error, info};

use crate::cli::Args;
use crate::config::{PROXY_SOCKET, REAL_SOCKET};
use crate::dns::cache::DnsCache;
use crate::dns_server;
use crate::network::NetworkMonitor;
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

/// Wait for `path` to appear using inotify on the parent directory.
/// Falls back to periodic polling if inotify fails.
async fn wait_for_socket(path: &str) -> io::Result<()> {
    if std::path::Path::new(path).exists() {
        return Ok(());
    }

    info!("waiting for {path} to appear...");

    // Try inotify first.
    match wait_for_socket_inotify(path).await {
        Ok(()) => {
            info!("{path} found (inotify)");
            return Ok(());
        }
        Err(_) => {
            // Fallback to polling.
            info!("inotify unavailable, falling back to poll for {path}");
        }
    }

    while !std::path::Path::new(path).exists() {
        sleep(Duration::from_millis(500)).await;
    }
    info!("{path} found (poll)");
    Ok(())
}

/// Wait for `path` to appear by watching its parent directory with inotify.
async fn wait_for_socket_inotify(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    use inotify::{EventMask, Inotify, WatchMask};

    let p = std::path::Path::new(path);
    let parent = p.parent().unwrap_or(p);
    let file_name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");

    let inotify = Inotify::init()?;
    inotify
        .watches()
        .add(parent, WatchMask::CREATE | WatchMask::MOVED_TO)?;

    let mut buffer = vec![0u8; 4096];
    let mut event_stream = inotify.into_event_stream(&mut buffer)?;

    use tokio_stream::StreamExt;
    while let Some(event_res) = event_stream.next().await {
        let event = event_res?;
        if event.mask.contains(EventMask::CREATE) || event.mask.contains(EventMask::MOVED_TO) {
            // Check if this event is about our target file.
            if let Some(name) = event.name {
                if name == file_name && std::path::Path::new(path).exists() {
                    return Ok(());
                }
            }
        }
    }

    Err("inotify stream ended".into())
}

pub async fn init(args: &Args) -> io::Result<()> {
    setup_signals();

    static RULES: std::sync::OnceLock<ArcSwap<RuleSet>> = std::sync::OnceLock::new();
    let store = RULES.get_or_init(|| ArcSwap::new(Arc::new(load_rules(&args.rules))));
    spawn_reload_watcher(args.rules.clone(), store);

    // DNS cache: shared across all DNS server handlers.
    static DNS_CACHE: std::sync::OnceLock<DnsCache> = std::sync::OnceLock::new();
    let cache =
        DNS_CACHE.get_or_init(|| DnsCache::new(if args.battery_saver { 512 } else { 2048 }));

    if args.dns_server {
        // Network monitor: detects WiFi ↔ mobile-data handover via netlink.
        let net_monitor = NetworkMonitor::spawn().unwrap_or_else(|e| {
            info!("network monitor unavailable ({e}), proceeding without it");
            NetworkMonitor::inert()
        });

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
        dns_server::run(bind_addr, upstream, store, cache, net_monitor).await?;
    } else {
        // Socket-hijacking mode: intercept netd dnsproxyd socket.
        //
        // Each handler creates a fresh Unix socket to the real netd per
        // request (following DnsResolver's per-query socket pattern).
        // This avoids stale-connection races on network switch — no
        // connection pool to invalidate.

        if std::path::Path::new(REAL_SOCKET).exists() {
            return Err(io::Error::new(
                ErrorKind::AlreadyExists,
                format!("{REAL_SOCKET} already exists, please reboot first"),
            ));
        }

        wait_for_socket(PROXY_SOCKET).await?;

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
                        if let Err(e) = handle_client(client, rules, REAL_SOCKET).await {
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
