//! Network change detection via netlink route monitoring.
//!
//! Opens a `NETLINK_ROUTE` socket and listens for default-route changes
//! (RTM_NEWROUTE / RTM_DELROUTE with zero-length destination prefix).
//! When the default route changes — which happens on WiFi ↔ mobile-data
//! handover — the module notifies the server loop to flush the DNS cache,
//! invalidate the netd connection pool, and rebind the upstream DNS socket.

use std::io;
use std::sync::Arc;
use std::thread;
use tokio::sync::Notify;
use tracing::{info, warn};

const AF_NETLINK: libc::c_int = 16;
const NETLINK_ROUTE: libc::c_int = 0;

#[cfg(target_os = "android")]
const SOCK_FLAGS: libc::c_int = libc::SOCK_RAW | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK;
#[cfg(not(target_os = "android"))]
const SOCK_FLAGS: libc::c_int = libc::SOCK_RAW | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK;

const RTMGRP_IPV4_ROUTE: u32 = 0x0040;
const RTMGRP_IPV6_ROUTE: u32 = 0x0100;

const RTM_NEWROUTE: u16 = 24;
const RTM_DELROUTE: u16 = 25;

// nlmsghdr size: 16 bytes (4+2+2+4+4)
const NLMSG_HDRLEN: usize = 16;
// rtmsg size: 12 bytes (1+1+1+1+1+1+1+1+4)
const RTMSG_LEN: usize = 12;

/// Handle to the network-change notifier.
///
/// Call [`NetworkMonitor::notified()`] to asynchronously wait for the next
/// default-route change event.
///
/// Cloning is cheap — wraps an `Arc` internally.
#[derive(Clone)]
pub struct NetworkMonitor {
    notify: Arc<Notify>,
}

impl NetworkMonitor {
    /// Spawn a background thread that reads netlink route events and notifies
    /// on every default-route change.
    pub fn spawn() -> io::Result<Self> {
        let notify = Arc::new(Notify::new());

        let fd = unsafe { libc::socket(AF_NETLINK, SOCK_FLAGS, NETLINK_ROUTE) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }

        // Bind to the route multicast groups we care about.
        let mut sa: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
        sa.nl_family = AF_NETLINK as libc::c_ushort;
        sa.nl_groups = RTMGRP_IPV4_ROUTE | RTMGRP_IPV6_ROUTE;

        let ret = unsafe {
            libc::bind(
                fd,
                &sa as *const _ as *const libc::sockaddr,
                std::mem::size_of::<libc::sockaddr_nl>() as u32,
            )
        };
        if ret < 0 {
            let e = io::Error::last_os_error();
            unsafe {
                libc::close(fd);
            }
            return Err(e);
        }

        // Clone the Arc for the background thread.
        let thread_notify = Arc::clone(&notify);

        thread::Builder::new()
            .name("anetd-netlink".into())
            .spawn(move || {
                let mut buf = vec![0u8; 4096];
                loop {
                    let n = unsafe { libc::recv(fd, buf.as_mut_ptr().cast(), buf.len(), 0) };
                    if n < 0 {
                        let e = io::Error::last_os_error();
                        if e.kind() == io::ErrorKind::Interrupted {
                            continue;
                        }
                        // Socket closed or fatal error — exit thread.
                        if e.kind() == io::ErrorKind::WouldBlock {
                            // Spurious wake on non-blocking socket; sleep briefly.
                            thread::sleep(std::time::Duration::from_millis(100));
                            continue;
                        }
                        warn!("netlink recv error: {e}");
                        break;
                    }
                    let n = n as usize;
                    if n == 0 {
                        break;
                    }

                    if has_default_route_change(&buf[..n]) {
                        info!("network change detected (default route changed)");
                        thread_notify.notify_one();
                    }
                }
                unsafe {
                    libc::close(fd);
                }
            })?;

        info!("network monitor started (netlink route watcher)");
        Ok(Self { notify })
    }

    /// Create a no-op monitor that never fires.  Used as a graceful fallback
    /// when the netlink socket cannot be opened (e.g. insufficient permissions).
    pub fn inert() -> Self {
        Self {
            notify: Arc::new(Notify::new()),
        }
    }

    /// Wait asynchronously for the next default-route change.
    pub async fn notified(&self) {
        self.notify.notified().await;
    }
}

/// Parse netlink messages in `buf` and return `true` if any of them is a
/// default-route add/delete.
fn has_default_route_change(buf: &[u8]) -> bool {
    let mut pos = 0;
    while pos + NLMSG_HDRLEN <= buf.len() {
        // SAFETY: we bounds-checked above.
        let nlmsg_len =
            u32::from_ne_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]) as usize;

        if nlmsg_len < NLMSG_HDRLEN || pos + nlmsg_len > buf.len() {
            break;
        }

        let nlmsg_type = u16::from_ne_bytes([buf[pos + 4], buf[pos + 5]]);

        if nlmsg_type == RTM_NEWROUTE || nlmsg_type == RTM_DELROUTE {
            let rtmsg_start = pos + NLMSG_HDRLEN;
            if rtmsg_start + RTMSG_LEN <= pos + nlmsg_len {
                // rtm_dst_len is the first byte of rtmsg (after family).
                let rtm_dst_len = buf[rtmsg_start + 1];
                if rtm_dst_len == 0 {
                    // Default route (0.0.0.0/0 or ::/0).
                    return true;
                }
            }
        }

        // Align to 4-byte boundary (NLMSG_ALIGN).
        let aligned = (nlmsg_len + 3) & !3;
        pos += aligned;
    }
    false
}
