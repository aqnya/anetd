use serde::Deserialize;

/// Base directory for anetd runtime data (logs, pid, config).
pub const BASE_DIR: &str = "/data/adb/modules/anetd";

/// Log directory (subdirectory of BASE_DIR).
pub const LOG_DIR: &str = "/data/adb/modules/anetd/log";

/// Default config file path.
pub const DEFAULT_CONFIG_FILE: &str = "/data/adb/modules/anetd/config.toml";

/// Stdout log file path.
pub const PATH_OUT: &str = "/data/adb/modules/anetd/log/anetd.out";

/// Stderr log file path.
pub const PATH_ERR: &str = "/data/adb/modules/anetd/log/anetd.err";

/// PID file path.
pub const PATH_PID: &str = "/data/adb/modules/anetd/log/anetd.pid";

/// The proxy socket path that clients connect to.
pub const PROXY_SOCKET: &str = "/dev/socket/dnsproxyd";

/// The real netd socket path (original dnsproxyd renamed).
pub const REAL_SOCKET: &str = "/dev/socket/dnsproxyd_real";

/// Default DNS server listen port.
pub const DNS_SERVER_PORT: u16 = 53;

/// Default upstream DNS server address.
pub const DNS_UPSTREAM: &str = "8.8.8.8:53";

/// Default path for the web UI Unix socket.
pub const WEBUI_SOCKET: &str = "/data/adb/modules/anetd/webui.sock";

/// Settings loadable from a TOML configuration file.
///
/// All fields are optional; CLI arguments override file values.
#[derive(Debug, Deserialize)]
pub struct ConfigFile {
    /// Path to rule file(s) or directory (same as `--rules` CLI arg).
    pub rules: Option<String>,
    /// Run as a background daemon.
    #[serde(default)]
    pub standalone: bool,
    /// Enable multi-thread Tokio runtime.
    #[serde(default)]
    pub multi_thread: bool,
    /// Enable built-in DNS server.
    #[serde(default)]
    pub dns_server: bool,
    /// DNS server listen port.
    #[serde(default = "default_dns_port")]
    pub dns_port: u16,
    /// Upstream DNS server address.
    #[serde(default = "default_dns_upstream")]
    pub dns_upstream: String,

    /// Enable battery saver mode: smaller DNS cache, single netd connection,
    /// reduced worker threads.
    #[serde(default)]
    pub battery_saver: bool,

    /// Path for the web UI Unix socket.
    #[serde(default = "default_webui_socket")]
    pub webui_socket: String,
}

fn default_dns_port() -> u16 {
    DNS_SERVER_PORT
}

fn default_dns_upstream() -> String {
    DNS_UPSTREAM.to_string()
}

fn default_webui_socket() -> String {
    WEBUI_SOCKET.to_string()
}

/// Load configuration from a TOML file.
pub fn load_config_file(path: &str) -> Result<ConfigFile, Box<dyn std::error::Error>> {
    let contents = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&contents)?)
}
