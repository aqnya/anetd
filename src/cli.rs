use crate::config::{DNS_SERVER_PORT, DNS_UPSTREAM};

#[derive(Debug)]
pub struct Args {
    /// Path to rule file(s) or directory. Supports comma-separated values
    pub config: String,

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
}

fn print_help() {
    println!(
        "Usage: anetd --config <PATH> [OPTIONS]

Options:
  -c, --config <PATH>       Path to rule file(s) or directory. Supports comma-separated values
  -s, --standalone          Run as a background daemon and log to file
  -m, --multi-thread        Enable multi thread
      --dns-server          Enable built-in DNS server (UDP/TCP)
      --dns-port <PORT>     DNS server listen port (default: 53)
      --dns-upstream <ADDR> Upstream DNS server address (default: 8.8.8.8:53)
  -h, --help                Print help"
    );
}

pub fn parse_args() -> Args {
    let mut config: Option<String> = None;
    let mut standalone = false;
    let mut multi_thread = false;
    let mut dns_server = false;
    let mut dns_port: u16 = DNS_SERVER_PORT;
    let mut dns_upstream: String = DNS_UPSTREAM.to_string();

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-c" | "--config" => {
                let val = it.next().unwrap_or_else(|| {
                    eprintln!("error: --config requires a value");
                    std::process::exit(1);
                });
                config = Some(val);
            }
            s if s.starts_with("--config=") => {
                config = Some(s["--config=".len()..].to_string());
            }
            s if s.starts_with("-c=") => {
                config = Some(s["-c=".len()..].to_string());
            }
            "-s" | "--standalone" => standalone = true,
            "-m" | "--multi-thread" => multi_thread = true,
            "--dns-server" => dns_server = true,
            "--dns-port" => {
                let val = it.next().unwrap_or_else(|| {
                    eprintln!("error: --dns-port requires a value");
                    std::process::exit(1);
                });
                dns_port = val.parse().unwrap_or_else(|_| {
                    eprintln!("error: --dns-port must be a valid port number");
                    std::process::exit(1);
                });
            }
            s if s.starts_with("--dns-port=") => {
                dns_port = s["--dns-port=".len()..].parse().unwrap_or_else(|_| {
                    eprintln!("error: --dns-port must be a valid port number");
                    std::process::exit(1);
                });
            }
            "--dns-upstream" => {
                let val = it.next().unwrap_or_else(|| {
                    eprintln!("error: --dns-upstream requires a value");
                    std::process::exit(1);
                });
                dns_upstream = val;
            }
            s if s.starts_with("--dns-upstream=") => {
                dns_upstream = s["--dns-upstream=".len()..].to_string();
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

    let config = config.unwrap_or_else(|| {
        eprintln!("error: the following required argument was not provided: --config <PATH>");
        print_help();
        std::process::exit(1);
    });

    Args {
        config,
        standalone,
        multi_thread,
        dns_server,
        dns_port,
        dns_upstream,
    }
}
