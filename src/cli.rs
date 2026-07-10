use crate::config::{self, DEFAULT_CONFIG_FILE, DNS_SERVER_PORT, DNS_UPSTREAM, WEBUI_SOCKET};

#[derive(Debug)]
pub struct Args {
    /// Path to rule file(s) or directory. Supports comma-separated values
    pub rules: String,

    /// Run as a background daemon and log to file
    pub standalone: bool,

    /// Enable multi thread
    pub multi_thread: bool,

    /// Enable built-in DNS server (UDP/TCP)
    pub dns_server: bool,

    /// DNS server listen port (default: 53)
    pub dns_port: u16,

    /// Upstream DNS server address (default: 8.8.8.8:53)
    pub dns_upstream: String,

    /// Enable battery saver mode: smaller cache, single netd connection,
    /// reduced worker threads.
    pub battery_saver: bool,

    /// Path for the web UI Unix socket.
    pub webui_socket: String,
}

fn print_help() {
    println!(
        "Usage: anetd --rules <PATH> [OPTIONS]

Options:
  -r, --rules <PATH>         Path to rule file(s) or directory. Supports comma-separated values
  -f, --config-file <PATH>   Path to TOML configuration file (default: /data/adb/anetd/config.toml)
  -s, --standalone           Run as a background daemon and log to file
  -m, --multi-thread         Enable multi thread
      --dns-server           Enable built-in DNS server (UDP/TCP)
      --dns-port <PORT>      DNS server listen port (default: 53)
      --dns-upstream <ADDR>  Upstream DNS server address (default: 8.8.8.8:53)
      --battery-saver        Enable battery saver mode (smaller cache, fewer connections)
      --webui-socket <PATH> Path for web UI Unix socket (default: /data/adb/modules/anetd/webui.sock)
  -h, --help                 Print help"
    );
}

/// Parse CLI arguments, load optional config file, and merge into a final `Args`.
///
/// Precedence: CLI args > config file > hardcoded defaults.
/// The only required value is `rules` (rule files path), which may come from either
/// `--rules` or the config file's `rules` key.
pub fn parse_args() -> Args {
    // --- CLI parsing: track explicitly-set values ---
    let mut rules: Option<String> = None;
    let mut config_file: Option<String> = None;
    let mut standalone = false;
    let mut standalone_set = false;
    let mut multi_thread = false;
    let mut multi_thread_set = false;
    let mut dns_server = false;
    let mut dns_server_set = false;
    let mut dns_port: Option<u16> = None;
    let mut dns_upstream: Option<String> = None;
    let mut battery_saver = false;
    let mut battery_saver_set = false;
    let mut webui_socket: Option<String> = None;

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-r" | "--rules" => {
                let val = it.next().unwrap_or_else(|| {
                    eprintln!("error: --rules requires a value");
                    std::process::exit(1);
                });
                rules = Some(val);
            }
            s if s.starts_with("--rules=") => {
                rules = Some(s["--rules=".len()..].to_string());
            }
            s if s.starts_with("-r=") => {
                rules = Some(s["-r=".len()..].to_string());
            }
            "-f" | "--config-file" => {
                let val = it.next().unwrap_or_else(|| {
                    eprintln!("error: --config-file requires a value");
                    std::process::exit(1);
                });
                config_file = Some(val);
            }
            s if s.starts_with("--config-file=") => {
                config_file = Some(s["--config-file=".len()..].to_string());
            }
            s if s.starts_with("-f=") => {
                config_file = Some(s["-f=".len()..].to_string());
            }
            "-s" | "--standalone" => {
                standalone = true;
                standalone_set = true;
            }
            "-m" | "--multi-thread" => {
                multi_thread = true;
                multi_thread_set = true;
            }
            "--dns-server" => {
                dns_server = true;
                dns_server_set = true;
            }
            "--dns-port" => {
                let val = it.next().unwrap_or_else(|| {
                    eprintln!("error: --dns-port requires a value");
                    std::process::exit(1);
                });
                dns_port = Some(val.parse().unwrap_or_else(|_| {
                    eprintln!("error: --dns-port must be a valid port number");
                    std::process::exit(1);
                }));
            }
            s if s.starts_with("--dns-port=") => {
                dns_port = Some(s["--dns-port=".len()..].parse().unwrap_or_else(|_| {
                    eprintln!("error: --dns-port must be a valid port number");
                    std::process::exit(1);
                }));
            }
            "--dns-upstream" => {
                let val = it.next().unwrap_or_else(|| {
                    eprintln!("error: --dns-upstream requires a value");
                    std::process::exit(1);
                });
                dns_upstream = Some(val);
            }
            s if s.starts_with("--dns-upstream=") => {
                dns_upstream = Some(s["--dns-upstream=".len()..].to_string());
            }
            "--battery-saver" => {
                battery_saver = true;
                battery_saver_set = true;
            }
            "--webui-socket" => {
                let val = it.next().unwrap_or_else(|| {
                    eprintln!("error: --webui-socket requires a value");
                    std::process::exit(1);
                });
                webui_socket = Some(val);
            }
            s if s.starts_with("--webui-socket=") => {
                webui_socket = Some(s["--webui-socket=".len()..].to_string());
            }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            other => {
                eprintln!("error: unrecognized argument '{}'", other);
                print_help();
                std::process::exit(1);
            }
        }
    }

    // --- Load config file (explicit or default) ---
    let file_path = config_file.as_deref().unwrap_or(DEFAULT_CONFIG_FILE);

    if let Ok(cf) = config::load_config_file(file_path) {
        // Merge: CLI values take precedence; fall back to config file.
        if rules.is_none() {
            rules = cf.rules;
        }
        if !standalone_set {
            standalone = cf.standalone;
        }
        if !multi_thread_set {
            multi_thread = cf.multi_thread;
        }
        if !dns_server_set {
            dns_server = cf.dns_server;
        }
        if dns_port.is_none() {
            dns_port = Some(cf.dns_port);
        }
        if dns_upstream.is_none() {
            dns_upstream = Some(cf.dns_upstream);
        }
        if !battery_saver_set {
            battery_saver = cf.battery_saver;
        }
        if webui_socket.is_none() {
            webui_socket = Some(cf.webui_socket);
        }
    } else if config_file.is_some() {
        // Explicit --config-file that failed to load → fatal.
        eprintln!("error: failed to load config file '{}'", file_path);
        std::process::exit(1);
    }
    // If no --config-file given and default doesn't exist, silently skip.

    // --- Final validation ---
    let rules = rules.unwrap_or_else(|| {
        eprintln!(
            "error: the following required argument was not provided: --rules <PATH>\n\
             Hint: you may also set `rules` in {}",
            DEFAULT_CONFIG_FILE
        );
        print_help();
        std::process::exit(1);
    });

    Args {
        rules,
        standalone,
        multi_thread,
        dns_server,
        dns_port: dns_port.unwrap_or(DNS_SERVER_PORT),
        dns_upstream: dns_upstream.unwrap_or_else(|| DNS_UPSTREAM.to_string()),
        battery_saver,
        webui_socket: webui_socket.unwrap_or_else(|| WEBUI_SOCKET.to_string()),
    }
}
