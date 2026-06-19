use arc_swap::ArcSwap;
use log::{error, info, warn};
use std::io::{self, ErrorKind, Read, Write};
use std::net::Shutdown;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::Arc;
use std::thread;

use crate::handlers::{CommandCtx, get_registry};
use crate::rules::{FilterRule, load_rules, spawn_reload_watcher};

const PROXY_SOCKET: &str = "/dev/socket/dnsproxyd";
const REAL_SOCKET: &str = "/dev/socket/dnsproxyd_real";

pub trait ProtoRead: Read {
    fn read_line_nul(&mut self) -> io::Result<String> {
        let mut buf = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            self.read_exact(&mut byte)?;
            if byte[0] == 0 {
                break;
            }
            buf.push(byte[0]);
            if buf.len() > 4096 {
                return Err(io::Error::new(ErrorKind::InvalidData, "line too long"));
            }
        }
        String::from_utf8(buf).map_err(|e| io::Error::new(ErrorKind::InvalidData, e))
    }
}

pub trait ProtoWrite: Write {
    fn write_cmd(&mut self, s: &str) -> io::Result<()> {
        let mut buf = Vec::with_capacity(s.len() + 1);
        buf.extend_from_slice(s.as_bytes());
        buf.push(0);
        self.write_all(&buf)
    }
}

impl<T: Read> ProtoRead for T {}
impl<T: Write> ProtoWrite for T {}

pub(crate) fn connect_netd() -> io::Result<UnixStream> {
    UnixStream::connect(REAL_SOCKET).map_err(|e| {
        error!("connect real netd: {e}");
        e
    })
}

pub(crate) fn proxy_transparent(client: &mut UnixStream, mut netd: UnixStream) -> io::Result<()> {
    let mut client_clone = client.try_clone()?;
    let mut netd_clone = netd.try_clone()?;

    thread::spawn(move || {
        let _ = io::copy(&mut netd_clone, &mut client_clone);
        let _ = client_clone.shutdown(Shutdown::Both);
    });

    let _ = io::copy(client, &mut netd);
    let _ = netd.shutdown(Shutdown::Both);

    Ok(())
}

fn handle_client(mut client: UnixStream, rules: Arc<Vec<FilterRule>>) -> io::Result<()> {
    let cmd = client.read_line_nul()?;
    info!(" cmd: \"{cmd}\"");

    let cmd_name = cmd.split_whitespace().next().unwrap_or("").to_lowercase();
    let registry = get_registry();

    if let Some(handler) = registry.get(&cmd_name) {
        let ctx = CommandCtx {
            client: &mut client,
            cmd_line: &cmd,
            rules,
        };
        handler.handle(ctx)?;
    } else {
        info!(" [I] Transparent proxy for unsupported command: {cmd_name}");
        let mut netd = connect_netd()?;
        netd.write_cmd(&cmd)?;
        proxy_transparent(&mut client, netd)?;
    }

    Ok(())
}

static ORIGINAL_SOCKET_RENAMED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

unsafe extern "C" fn on_exit_signal(_: libc::c_int) {
    if ORIGINAL_SOCKET_RENAMED.load(std::sync::atomic::Ordering::SeqCst) {
        let _ = std::fs::remove_file(PROXY_SOCKET);
        let _ = std::fs::rename(REAL_SOCKET, PROXY_SOCKET);
    }
    unsafe {
        libc::_exit(0);
    }
}

fn setup_signals() {
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_IGN);
        libc::signal(
            libc::SIGINT,
            on_exit_signal as *const () as libc::sighandler_t,
        );
        libc::signal(
            libc::SIGTERM,
            on_exit_signal as *const () as libc::sighandler_t,
        );
    }
}

pub fn init(rules_path: String) -> io::Result<()> {
    setup_signals();

    static RULES: std::sync::OnceLock<ArcSwap<Vec<FilterRule>>> = std::sync::OnceLock::new();
    let store = RULES.get_or_init(|| ArcSwap::new(Arc::new(load_rules(&rules_path.clone()))));
    spawn_reload_watcher(rules_path.clone(), store);

    if std::path::Path::new(REAL_SOCKET).exists() {
        warn!("{} already exists, removing", REAL_SOCKET);
        std::fs::remove_file(REAL_SOCKET)?;
    }

    std::fs::rename(PROXY_SOCKET, REAL_SOCKET)?;
    ORIGINAL_SOCKET_RENAMED.store(true, std::sync::atomic::Ordering::SeqCst);

    let listener = UnixListener::bind(PROXY_SOCKET)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
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

    info!("listening on {PROXY_SOCKET}");
    info!("[*] forwarding to {REAL_SOCKET}");

    for stream in listener.incoming() {
        match stream {
            Ok(client) => {
                let rules = store.load_full();
                thread::spawn(move || {
                    if let Err(e) = handle_client(client, rules) {
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

    Ok(())
}
