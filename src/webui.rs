//! Unix domain socket server for the KSU app WebUI.
//!
//! Listens on a Unix socket and speaks a simple JSON-line protocol:
//!   client → `{"method":"get_status"}\n`
//!   server → `{"ok":true,"running":true,...}\n`
//!
//! The KSU WebView bridges to this socket via `ksu.exec("echo ... | nc -U ...")`.
//!
//! Responses are valid JSON. Requests are a single JSON line with the
//! `method` field; string payloads (e.g. `content` for `save_config`) are
//! JSON-escaped and decoded here, so shell quoting and file contents can
//! never corrupt each other.

use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener as StdUnixListener, UnixStream};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::Instant;

use arc_swap::ArcSwap;
use tracing::{error, info};

use crate::config::{DEFAULT_CONFIG_FILE, LOG_DIR};
use crate::rules::RuleSet;
use crate::rules::loader::load_rules;

/// Static information about the daemon, provided to the WebUI server.
#[derive(Clone)]
pub struct WebUiInfo {
    pub version: &'static str,
    pub started: Instant,
    pub socket_path: String,
    pub rules_path: String,
    pub config_path: String,
    pub log_path: String,
    pub dns_server: bool,
    pub dns_port: u16,
    pub dns_upstream: String,
    pub battery_saver: bool,
    pub multi_thread: bool,
    pub standalone: bool,
}

impl WebUiInfo {
    fn from_args(args: &crate::cli::Args) -> Self {
        WebUiInfo {
            version: env!("CARGO_PKG_VERSION"),
            started: Instant::now(),
            socket_path: args.webui_socket.clone(),
            rules_path: args.rules.clone(),
            config_path: DEFAULT_CONFIG_FILE.to_string(),
            log_path: format!("{LOG_DIR}/anetd.log"),
            dns_server: args.dns_server,
            dns_port: args.dns_port,
            dns_upstream: args.dns_upstream.clone(),
            battery_saver: args.battery_saver,
            multi_thread: args.multi_thread,
            standalone: args.standalone,
        }
    }
}

/// Run the Web UI Unix socket server.
///
/// Handles one connection at a time in a loop (single-threaded, blocking I/O
/// on a dedicated thread — acceptable for the low-volume management socket).
pub fn run(
    args: &crate::cli::Args,
    store: &'static ArcSwap<RuleSet>,
    block_count: &'static AtomicU64,
    dns_queries: &'static AtomicU64,
) -> io::Result<()> {
    let info = WebUiInfo::from_args(args);
    let socket_path = info.socket_path.clone();

    // Remove stale socket
    let path = Path::new(&socket_path);
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let listener = StdUnixListener::bind(&socket_path)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o660))?;
    }

    info!("[webui] unix socket listening on {socket_path}");

    for stream in listener.incoming() {
        match stream {
            Ok(mut conn) => {
                let info = info.clone();
                std::thread::spawn(move || {
                    if let Err(e) =
                        handle_conn(&mut conn, &info, store, block_count, dns_queries)
                    {
                        error!("[webui] connection error: {e}");
                    }
                });
            }
            Err(e) => error!("[webui] accept error: {e}"),
        }
    }
    Ok(())
}

/// Handle a single client connection: read one JSON-line request, respond.
fn handle_conn(
    conn: &mut UnixStream,
    info: &WebUiInfo,
    store: &ArcSwap<RuleSet>,
    block_count: &AtomicU64,
    dns_queries: &AtomicU64,
) -> io::Result<()> {
    let mut line = String::new();
    let mut reader = BufReader::new(&mut *conn);
    reader.read_line(&mut line)?;
    drop(reader);

    let method = parse_method(&line);

    let response = match method {
        "get_status" => status_json(info, store, block_count, dns_queries),
        "load_rules" => rules_json(store),
        "reload_rules" => reload_rules(store, &info.rules_path),
        "load_config" => load_config(info),
        "save_config" => save_config(info, store, &line),
        "load_logs" => logs_json(&line, info),
        _ => "{\"ok\":false,\"error\":\"unknown method\"}\n".to_string(),
    };

    conn.write_all(response.as_bytes())?;
    Ok(())
}

// ── method handlers ─────────────────────────────────────────────────────────

fn status_json(
    info: &WebUiInfo,
    store: &ArcSwap<RuleSet>,
    block_count: &AtomicU64,
    dns_queries: &AtomicU64,
) -> String {
    let rules = store.load();
    let uptime_sec = info.started.elapsed().as_secs();
    // The daemon answers this socket, so "running" is true; the frontend
    // treats a failed/empty response as "daemon down".
    format!(
        concat!(
            "{{",
            "\"ok\":true,",
            "\"running\":true,",
            "\"version\":{},",
            "\"pid\":{},",
            "\"uptime_sec\":{},",
            "\"uptime\":{},",
            "\"dns_queries\":{},",
            "\"blocked\":{},",
            "\"rules_count\":{},",
            "\"block_rules\":{},",
            "\"allow_rules\":{},",
            "\"dns_filter_enabled\":{},",
            "\"mode\":{},",
            "\"battery_saver\":{},",
            "\"multi_thread\":{},",
            "\"standalone\":{},",
            "\"dns_port\":{},",
            "\"dns_upstream\":{},",
            "\"rules_path\":{},",
            "\"socket_path\":{},",
            "\"config_path\":{},",
            "\"log_path\":{}",
            "}}\n"
        ),
        json_str(info.version),
        std::process::id(),
        uptime_sec,
        json_str(&format_uptime(uptime_sec)),
        dns_queries.load(std::sync::atomic::Ordering::Relaxed),
        block_count.load(std::sync::atomic::Ordering::Relaxed),
        rules.watched_files.len(),
        rules.block_count(),
        rules.allow_count(),
        filter_enabled(),
        json_str(if info.dns_server { "dns-server" } else { "hijack" }),
        info.battery_saver,
        info.multi_thread,
        info.standalone,
        info.dns_port,
        json_str(&info.dns_upstream),
        json_str(&info.rules_path),
        json_str(&info.socket_path),
        json_str(&info.config_path),
        json_str(&info.log_path),
    )
}

fn rules_json(store: &ArcSwap<RuleSet>) -> String {
    let rules = store.load();
    let mut json = String::from("{\"ok\":true,\"files\":[");
    for (i, (path, hash)) in rules.watched_files.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        json.push_str(&format!(
            "{{\"path\":{},\"hash\":{}}}",
            json_str(path),
            json_str(hash)
        ));
    }
    json.push_str("]}\n");
    json
}

fn reload_rules(store: &ArcSwap<RuleSet>, rules_path: &str) -> String {
    info!("[webui] reload rules");
    let new_rules = load_rules(rules_path);
    let block = new_rules.block_count();
    let allow = new_rules.allow_count();
    let files = new_rules.watched_files.len();
    store.store(Arc::new(new_rules));
    format!(
        "{{\"ok\":true,\"rules_count\":{files},\"block_rules\":{block},\"allow_rules\":{allow}}}\n"
    )
}

fn load_config(info: &WebUiInfo) -> String {
    match std::fs::read_to_string(&info.config_path) {
        Ok(content) => format!(
            "{{\"ok\":true,\"content\":{},\"values\":{}}}\n",
            json_str(&content),
            config_values_json(&content)
        ),
        Err(_) => "{\"ok\":true,\"content\":\"\",\"values\":{}}\n".to_string(),
    }
}

fn save_config(info: &WebUiInfo, store: &ArcSwap<RuleSet>, line: &str) -> String {
    let Some(content) = extract_json_field(line, "content") else {
        return "{\"ok\":false,\"error\":\"missing content\"}\n".to_string();
    };

    // Validate before touching the file so a typo cannot brick the config.
    if content.parse::<toml::Table>().is_err() {
        return "{\"ok\":false,\"error\":\"invalid TOML\"}\n".to_string();
    }

    match std::fs::write(&info.config_path, content.as_bytes()) {
        Ok(()) => {
            info!("[webui] config saved, reloading rules");
            let new_rules = load_rules(&info.rules_path);
            store.store(Arc::new(new_rules));
            "{\"ok\":true}\n".to_string()
        }
        Err(e) => format!("{{\"ok\":false,\"error\":{}}}\n", json_str(&e.to_string())),
    }
}

fn logs_json(line: &str, info: &WebUiInfo) -> String {
    let count = extract_json_number(line, "count").unwrap_or(100) as usize;
    match std::fs::read_to_string(&info.log_path) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            let start = lines.len().saturating_sub(count);
            let mut json = String::from("{\"ok\":true,\"lines\":[");
            for (i, l) in lines[start..].iter().enumerate() {
                if i > 0 {
                    json.push(',');
                }
                json.push_str(&json_str(l));
            }
            json.push_str("]}\n");
            json
        }
        Err(_) => "{\"ok\":true,\"lines\":[]}\n".to_string(),
    }
}

// ── helpers ─────────────────────────────────────────────────────────────────

/// Whether the adblock filter is currently active (not paused via toggle.sh).
fn filter_enabled() -> bool {
    !Path::new(&format!("{LOG_DIR}/dns_off")).exists()
}

fn format_uptime(secs: u64) -> String {
    let d = secs / 86_400;
    let h = (secs % 86_400) / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m {s}s")
    } else {
        format!("{s}s")
    }
}

/// Encode a string as a JSON string literal (UTF-8 safe).
fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Extract `"method"` from a JSON line like `{"method":"get_status"}`.
fn parse_method(line: &str) -> &str {
    line.split("\"method\"")
        .nth(1)
        .and_then(|s| s.split('"').nth(1))
        .unwrap_or("")
}

/// Extract and JSON-decode a string field (e.g. `"content"`) from a simple
/// JSON object line. Handles `\"`, `\\`, and control escapes.
fn extract_json_field(line: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let after_key = line.find(&needle)? + needle.len();
    let rest = line[after_key..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;

    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                '/' => out.push('/'),
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                'b' => out.push('\u{0008}'),
                'f' => out.push('\u{000c}'),
                'u' => {
                    let hex: String = chars.by_ref().take(4).collect();
                    if let Ok(code) = u32::from_str_radix(&hex, 16)
                        && let Some(ch) = char::from_u32(code)
                    {
                        out.push(ch);
                    }
                }
                _ => {}
            },
            c => out.push(c),
        }
    }
    None
}

/// Extract a JSON number field (e.g. `"count": 200`) from a simple object.
fn extract_json_number(line: &str, key: &str) -> Option<u64> {
    let needle = format!("\"{key}\"");
    let after_key = line.find(&needle)? + needle.len();
    let rest = line[after_key..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// Build a JSON object of the config keys the WebUI knows about, parsed from
/// the raw TOML content. Unknown keys and comments are preserved in the raw
/// content and left untouched.
fn config_values_json(content: &str) -> String {
    let Ok(table) = content.parse::<toml::Table>() else {
        return "{}".to_string();
    };

    const KNOWN_KEYS: [&str; 8] = [
        "rules",
        "standalone",
        "multi_thread",
        "dns_server",
        "dns_port",
        "dns_upstream",
        "battery_saver",
        "webui_socket",
    ];

    let mut out = String::from("{");
    let mut first = true;
    for key in KNOWN_KEYS {
        let Some(value) = table.get(key) else { continue };
        let piece = match value {
            toml::Value::String(s) => json_str(s),
            toml::Value::Boolean(b) => b.to_string(),
            toml::Value::Integer(i) => i.to_string(),
            _ => continue,
        };
        if !first {
            out.push(',');
        }
        first = false;
        out.push_str(&format!("\"{key}\":{piece}"));
    }
    out.push('}');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_str_escapes() {
        assert_eq!(json_str("a\"b\\c\nd"), r#""a\"b\\c\nd""#);
        assert_eq!(json_str("中文"), "\"中文\"");
    }

    #[test]
    fn extract_field_decodes_escapes() {
        let line = r#"{"method":"save_config","content":"rules = \"/x\"\nstandalone = false"}"#;
        assert_eq!(
            extract_json_field(line, "content").as_deref(),
            Some("rules = \"/x\"\nstandalone = false")
        );
        assert_eq!(extract_json_field(line, "nope"), None);
    }

    #[test]
    fn extract_number() {
        assert_eq!(extract_json_number(r#"{"count":200}"#, "count"), Some(200));
        assert_eq!(extract_json_number(r#"{"count": 12}"#, "count"), Some(12));
    }

    #[test]
    fn config_values() {
        let content = "rules = \"/data/adb/modules/anetd/rules\"\nstandalone = false\n";
        let json = config_values_json(content);
        assert!(json.contains("\"rules\":\"/data/adb/modules/anetd/rules\""));
        assert!(json.contains("\"standalone\":false"));
    }

    #[test]
    fn uptime() {
        assert_eq!(format_uptime(5), "5s");
        assert_eq!(format_uptime(65), "1m 5s");
        assert_eq!(format_uptime(3661), "1h 1m");
        assert_eq!(format_uptime(90_000), "1d 1h");
    }

    fn test_info(tag: &str) -> WebUiInfo {
        let dir = std::env::temp_dir();
        WebUiInfo {
            version: "test",
            started: Instant::now(),
            socket_path: dir.join(format!("anetd-test-{tag}.sock")).to_string_lossy().into(),
            rules_path: dir.join(format!("anetd-test-{tag}-rules")).to_string_lossy().into(),
            config_path: dir
                .join(format!("anetd-test-{tag}-config.toml"))
                .to_string_lossy()
                .into(),
            log_path: dir.join(format!("anetd-test-{tag}.log")).to_string_lossy().into(),
            dns_server: false,
            dns_port: 53,
            dns_upstream: "1.1.1.1:53".to_string(),
            battery_saver: false,
            multi_thread: true,
            standalone: true,
        }
    }

    /// Send one request line over a socket pair and return the response line.
    fn leaked_store() -> &'static ArcSwap<RuleSet> {
        Box::leak(Box::new(ArcSwap::new(Arc::new(RuleSet::new()))))
    }

    fn roundtrip(req: &str, info: &WebUiInfo, store: &'static ArcSwap<RuleSet>) -> String {
        static BLOCKED: AtomicU64 = AtomicU64::new(42);
        static QUERIES: AtomicU64 = AtomicU64::new(7);
        let (mut client, mut server) = UnixStream::pair().unwrap();
        let info = info.clone();
        std::thread::spawn(move || {
            let _ = handle_conn(&mut server, &info, &store, &BLOCKED, &QUERIES);
        });
        client.write_all(format!("{req}\n").as_bytes()).unwrap();
        let mut resp = String::new();
        let mut reader = BufReader::new(&mut client);
        reader.read_line(&mut resp).unwrap();
        resp
    }

    #[test]
    fn status_response_contains_all_fields() {
        let store = leaked_store();
        let resp = roundtrip(r#"{"method":"get_status"}"#, &test_info("status"), &store);
        assert!(resp.contains("\"ok\":true"), "{resp}");
        assert!(resp.contains("\"running\":true"), "{resp}");
        assert!(resp.contains("\"version\":\"test\""), "{resp}");
        assert!(resp.contains("\"blocked\":42"), "{resp}");
        assert!(resp.contains("\"dns_queries\":7"), "{resp}");
        assert!(resp.contains("\"mode\":\"hijack\""), "{resp}");
        assert!(resp.contains("\"rules_count\":0"), "{resp}");
        assert!(resp.contains("\"dns_filter_enabled\":true"), "{resp}");
    }

    #[test]
    fn save_config_roundtrips_quotes_and_newlines() {
        let store = leaked_store();
        let info = test_info("save");
        let req = r#"{"method":"save_config","content":"rules = \"/data/adb/modules/anetd/rules\"\nstandalone = false\nbattery_saver = true\n"}"#;
        let resp = roundtrip(req, &info, &store);
        assert!(resp.contains("\"ok\":true"), "{resp}");
        let saved = std::fs::read_to_string(&info.config_path).unwrap();
        assert_eq!(
            saved,
            "rules = \"/data/adb/modules/anetd/rules\"\nstandalone = false\nbattery_saver = true\n"
        );
        let _ = std::fs::remove_file(&info.config_path);
    }

    #[test]
    fn save_config_rejects_invalid_toml() {
        let store = leaked_store();
        let info = test_info("invalid");
        std::fs::write(&info.config_path, "rules = \"keep\"\n").unwrap();
        let req = r#"{"method":"save_config","content":"this is not toml"}"#;
        let resp = roundtrip(req, &info, &store);
        assert!(resp.contains("\"ok\":false"), "{resp}");
        assert!(resp.contains("invalid TOML"), "{resp}");
        // File must be untouched.
        let saved = std::fs::read_to_string(&info.config_path).unwrap();
        assert_eq!(saved, "rules = \"keep\"\n");
        let _ = std::fs::remove_file(&info.config_path);
    }

    #[test]
    fn logs_escape_special_characters() {
        let store = leaked_store();
        let info = test_info("logs");
        std::fs::write(&info.log_path, "line one\nwith \"quotes\" and \\ backslash\n").unwrap();
        let resp = roundtrip(r#"{"method":"load_logs","count":10}"#, &info, &store);
        assert!(resp.contains(r#""line one""#), "{resp}");
        assert!(resp.contains(r#""with \"quotes\" and \\ backslash""#), "{resp}");
        let _ = std::fs::remove_file(&info.log_path);
    }
}
