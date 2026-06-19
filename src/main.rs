use arc_swap::ArcSwap;
use std::io::{self, ErrorKind, Read, Write};
use std::net::{Ipv4Addr, Shutdown};
use std::os::unix::net::{UnixListener, UnixStream};
use std::str::FromStr;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

const PROXY_SOCKET: &str = "/dev/socket/dnsproxyd";
const REAL_SOCKET: &str = "/dev/socket/dnsproxyd_real";
const RULES_FILE: &str = "/data/local/tmp/anetd/rules.conf";

#[derive(Debug, Clone)]
struct GetAddrInfoRequest {
    hostname: Option<String>,
    servname: Option<String>,
    ai_flags: i32,
    ai_family: i32,
    ai_socktype: i32,
    ai_protocol: i32,
    net_id: u32,
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
        })
    }

    fn to_cmd(&self) -> String {
        format!(
            "getaddrinfo {} {} {} {} {} {} {}",
            self.hostname.as_deref().unwrap_or("^"),
            self.servname.as_deref().unwrap_or("^"),
            self.ai_flags,
            self.ai_family,
            self.ai_socktype,
            self.ai_protocol,
            self.net_id,
        )
    }

    fn hostname_str(&self) -> &str {
        self.hostname.as_deref().unwrap_or("(null)")
    }
}

#[derive(Debug, Clone)]
enum FilterAction {
    Allow,
    Block,
    Redirect(String),
    Fake(String),
}

#[derive(Debug, Clone)]
struct FilterRule {
    pattern: Option<String>,
    action: FilterAction,
}

impl FilterRule {
    fn matches(&self, hostname: &str) -> bool {
        let Some(pat) = &self.pattern else {
            return true;
        };
        let (h, p) = (hostname.to_lowercase(), pat.to_lowercase());
        h == p
            || (h.len() > p.len() && h.ends_with(&p) && h.as_bytes()[h.len() - p.len() - 1] == b'.')
    }
}

fn pattern(s: &str) -> Option<String> {
    if s == "*" { None } else { Some(s.into()) }
}

fn parse_rules(text: &str) -> Vec<FilterRule> {
    let mut rules = Vec::new();
    for (lineno, raw) in text.lines().enumerate() {
        let line = match raw.find('#') {
            Some(i) => &raw[..i],
            None => raw,
        }
        .trim();
        if line.is_empty() {
            continue;
        }

        let cols: Vec<&str> = line.split_whitespace().collect();
        let rule = match cols.as_slice() {
            ["allow", pat] => FilterRule {
                pattern: pattern(pat),
                action: FilterAction::Allow,
            },
            ["block", pat] => FilterRule {
                pattern: pattern(pat),
                action: FilterAction::Block,
            },
            ["fake", pat, ip] => FilterRule {
                pattern: pattern(pat),
                action: FilterAction::Fake((*ip).into()),
            },
            ["redirect", pat, dest] => FilterRule {
                pattern: pattern(pat),
                action: FilterAction::Redirect((*dest).into()),
            },
            _ => {
                eprintln!("[rules] line {}: unrecognized: {:?}", lineno + 1, line);
                continue;
            }
        };
        rules.push(rule);
    }
    if !rules.iter().any(|r| r.pattern.is_none()) {
        rules.push(FilterRule {
            pattern: None,
            action: FilterAction::Allow,
        });
    }
    rules
}

fn load_rules(path: &str) -> Vec<FilterRule> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let rules = parse_rules(&text);
            println!("[rules] loaded {} rules from {}", rules.len(), path);
            rules
        }
        Err(e) => {
            eprintln!("[rules] failed to load {path}: {e}, using default allow-all");
            vec![FilterRule {
                pattern: None,
                action: FilterAction::Allow,
            }]
        }
    }
}

fn mtime(path: &str) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

fn spawn_reload_watcher(path: &'static str, store: &'static ArcSwap<Vec<FilterRule>>) {
    thread::spawn(move || {
        let mut last = mtime(path);
        loop {
            thread::sleep(Duration::from_secs(3));
            let cur = mtime(path);
            if cur != last {
                last = cur;
                store.store(Arc::new(load_rules(path)));
                println!("[rules] reloaded");
            }
        }
    });
}

trait ProtoRead: Read {
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

trait ProtoWrite: Write {
    fn write_cmd(&mut self, s: &str) -> io::Result<()> {
        let mut buf = Vec::with_capacity(s.len() + 1);
        buf.extend_from_slice(s.as_bytes());
        buf.push(0);
        self.write_all(&buf)
    }

    fn write_u32be(&mut self, v: u32) -> io::Result<()> {
        self.write_all(&v.to_be_bytes())
    }

    fn write_u32ne(&mut self, v: u32) -> io::Result<()> {
        self.write_all(&v.to_ne_bytes())
    }
}

impl<T: Read> ProtoRead for T {}
impl<T: Write> ProtoWrite for T {}

fn send_block(w: &mut impl Write) -> io::Result<()> {
    w.write_cmd("222 DnsProxyQueryResult")?;
    w.write_u32ne(0)?;
    println!("  → BLOCKED");
    Ok(())
}

fn send_fake(w: &mut impl Write, ip: &str) -> io::Result<()> {
    let Ok(addr) = Ipv4Addr::from_str(ip) else {
        return send_block(w);
    };

    let mut sockaddr = [0u8; 16];
    sockaddr[0..2].copy_from_slice(&(libc::AF_INET as u16).to_ne_bytes());
    sockaddr[2..4].copy_from_slice(&[0u8; 2]);
    sockaddr[4..8].copy_from_slice(&addr.octets());

    w.write_cmd("222 DnsProxyQueryResult")?;
    w.write_u32ne(1)?;
    w.write_u32ne(0)?;
    w.write_u32ne(libc::AF_INET as u32)?;
    w.write_u32ne(libc::SOCK_STREAM as u32)?;
    w.write_u32ne(0)?;
    w.write_u32be(sockaddr.len() as u32)?;
    w.write_all(&sockaddr)?;
    w.write_u32be(0)?;
    w.write_u32ne(0)?;

    println!("  → FAKE {ip}");
    Ok(())
}

fn connect_netd() -> io::Result<UnixStream> {
    UnixStream::connect(REAL_SOCKET).map_err(|e| {
        eprintln!("connect real netd: {e}");
        e
    })
}

fn proxy_transparent(mut client: UnixStream, mut netd: UnixStream) -> io::Result<()> {
    let mut client_clone = client.try_clone()?;
    let mut netd_clone = netd.try_clone()?;

    thread::spawn(move || {
        let _ = io::copy(&mut netd_clone, &mut client_clone);
        let _ = client_clone.shutdown(Shutdown::Both);
    });

    let _ = io::copy(&mut client, &mut netd);
    let _ = netd.shutdown(Shutdown::Both);

    Ok(())
}

fn handle_client(mut client: UnixStream, rules: Arc<Vec<FilterRule>>) -> io::Result<()> {
    let cmd = client.read_line_nul()?;
    println!("[>] cmd: \"{cmd}\"");

    let Some(req) = GetAddrInfoRequest::parse(&cmd) else {
        println!("  → [BYPASS] Transparent proxy for unsupported command");
        let mut netd = connect_netd()?;
        netd.write_cmd(&cmd)?;
        return proxy_transparent(client, netd);
    };

    let hostname = req.hostname_str();
    println!("  hostname: {hostname}");

    let rule = rules.iter().find(|r| r.matches(hostname)).unwrap();

    match &rule.action {
        FilterAction::Block => {
            send_block(&mut client)?;
        }
        FilterAction::Fake(ip) => {
            send_fake(&mut client, ip)?;
        }
        FilterAction::Redirect(target) => {
            let mut new_req = req.clone();
            new_req.hostname = Some(target.clone());
            println!("  → REDIRECT to {target}");
            let mut netd = connect_netd()?;
            netd.write_cmd(&new_req.to_cmd())?;
            proxy_transparent(client, netd)?;
        }
        FilterAction::Allow => {
            let mut netd = connect_netd()?;
            netd.write_cmd(&cmd)?;
            proxy_transparent(client, netd)?;
        }
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

fn main() -> io::Result<()> {
    setup_signals();

    static RULES: std::sync::OnceLock<ArcSwap<Vec<FilterRule>>> = std::sync::OnceLock::new();
    let store = RULES.get_or_init(|| ArcSwap::new(Arc::new(load_rules(RULES_FILE))));
    spawn_reload_watcher(RULES_FILE, store);

    if std::path::Path::new(REAL_SOCKET).exists() {
        eprintln!("[warn] {REAL_SOCKET} already exists, removing");
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

    println!("[*] listening on {PROXY_SOCKET}");
    println!("[*] forwarding to {REAL_SOCKET}");
    println!("[*] rules: {RULES_FILE}\n");

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
                            eprintln!("[client error] {e}");
                        }
                    }
                });
            }
            Err(e) => eprintln!("[accept error] {e}"),
        }
    }

    Ok(())
}
