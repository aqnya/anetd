/// Base directory for anetd runtime data (logs, pid).
pub const BASE_DIR: &str = "/data/adb/anetd";

/// Log directory (subdirectory of BASE_DIR).
pub const LOG_DIR: &str = "/data/adb/anetd/log";

/// Stdout log file path.
pub const PATH_OUT: &str = "/data/adb/anetd/log/anetd.out";

/// Stderr log file path.
pub const PATH_ERR: &str = "/data/adb/anetd/log/anetd.err";

/// PID file path.
pub const PATH_PID: &str = "/data/adb/anetd/log/anetd.pid";

/// The proxy socket path that clients connect to.
pub const PROXY_SOCKET: &str = "/dev/socket/dnsproxyd";

/// The real netd socket path (original dnsproxyd renamed).
pub const REAL_SOCKET: &str = "/dev/socket/dnsproxyd_real";

/// Default DNS server listen port.
pub const DNS_SERVER_PORT: u16 = 53;

/// Default upstream DNS server address.
pub const DNS_UPSTREAM: &str = "8.8.8.8:53";
