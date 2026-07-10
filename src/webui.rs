//! Unix domain socket server for the KSU app WebUI.
//!
//! Listens on a Unix socket and speaks a simple JSON-line protocol:
//!   client → `{"method":"get_status"}\n`
//!   server → `{"running":true,...}\n`
//!
//! The KSU WebView bridges to this socket via `ksu.exec("nc -U ...")`.

use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener as StdUnixListener, UnixStream};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use arc_swap::ArcSwap;
use tracing::{error, info};

use crate::rules::RuleSet;
use crate::rules::loader::load_rules;

/// Run the Web UI Unix socket server.
///
/// Handles one connection at a time in a loop (single-threaded, blocking I/O
/// on a dedicated thread — acceptable for the low-volume management socket).
pub fn run(
    socket_path: &str,
    store: &'static ArcSwap<RuleSet>,
    rules_path: String,
    block_count: &'static AtomicU64,
    dns_queries: &'static AtomicU64,
) -> io::Result<()> {
    // Remove stale socket
    let path = Path::new(socket_path);
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let listener = StdUnixListener::bind(socket_path)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o660))?;
    }

    info!("[webui] unix socket listening on {socket_path}");

    for stream in listener.incoming() {
        match stream {
            Ok(mut conn) => {
                let rules_path = rules_path.clone();
                std::thread::spawn(move || {
                    if let Err(e) =
                        handle_conn(&mut conn, store, &rules_path, block_count, dns_queries)
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
    store: &ArcSwap<RuleSet>,
    rules_path: &str,
    block_count: &AtomicU64,
    dns_queries: &AtomicU64,
) -> io::Result<()> {
    let mut line = String::new();
    let mut reader = BufReader::new(&mut *conn);
    reader.read_line(&mut line)?;
    drop(reader);

    let method = parse_method(&line);

    let response = match method {
        "get_status" => {
            let rules = store.load();
            format!(
                "{{\"running\":true,\"blocked\":{},\"dns_queries\":{},\
                 \"rules_count\":{},\"block_rules\":{},\"allow_rules\":{},
                 \"pid\":{},\"uptime\":\"\",\
                 \"dnsFilterEnabled\":{}}}\n",
                block_count.load(std::sync::atomic::Ordering::Relaxed),
                dns_queries.load(std::sync::atomic::Ordering::Relaxed),
                rules.watched_files.len(),
                rules.block_count(),
                rules.allow_count(),
                std::process::id(),
                !rules.block_count() > 0 || rules.allow_count() > 0,
            )
        }

        "load_rules" => {
            let rules = store.load();
            let mut json = String::from("[");
            for (i, (path, hash)) in rules.watched_files.iter().enumerate() {
                if i > 0 {
                    json.push(',');
                }
                json.push_str(&format!("{{\"path\":\"{}\",\"hash\":\"{}\"}}", path, hash));
            }
            json.push_str("]\n");
            json
        }

        "reload_rules" => {
            info!("[webui] reload rules");
            let new_rules = load_rules(rules_path);
            let block = new_rules.block_count();
            let allow = new_rules.allow_count();
            let files = new_rules.watched_files.len();
            store.store(Arc::new(new_rules));
            format!(
                "{{\"ok\":true,\"rules_count\":{files},\
                 \"block_rules\":{block},\"allow_rules\":{allow}}}\n"
            )
        }

        "load_config" => {
            let config_path = "/data/adb/modules/anetd/config.toml";
            match std::fs::read_to_string(config_path) {
                Ok(content) => {
                    let escaped = content
                        .replace('\\', "\\\\")
                        .replace('"', "\\\"")
                        .replace('\n', "\\n");
                    format!("{{\"content\":\"{escaped}\"}}\n")
                }
                Err(_) => "{\"content\":\"\"}\n".to_string(),
            }
        }

        "save_config" => {
            // Extract content from the request JSON
            let content = extract_json_str(&line, "content").unwrap_or("");
            let config_path = "/data/adb/modules/anetd/config.toml";
            match std::fs::write(config_path, content) {
                Ok(()) => {
                    // Also reload rules after config change
                    let new_rules = load_rules(rules_path);
                    store.store(Arc::new(new_rules));
                    "{\"ok\":true}\n".to_string()
                }
                Err(e) => format!("{{\"ok\":false,\"error\":\"{e}\"}}\n"),
            }
        }

        "load_logs" => {
            let count = extract_json_u64(&line, "count").unwrap_or(100);
            let log_path = "/data/adb/modules/anetd/log/anetd.log";
            match std::fs::read_to_string(log_path) {
                Ok(content) => {
                    let lines: Vec<&str> = content.lines().collect();
                    let start = if lines.len() > count as usize {
                        lines.len() - count as usize
                    } else {
                        0
                    };
                    let mut json = String::from("[");
                    for (i, l) in lines[start..].iter().enumerate() {
                        if i > 0 {
                            json.push(',');
                        }
                        json.push('"');
                        json.push_str(&l.replace('\\', "\\\\").replace('"', "\\\""));
                        json.push('"');
                    }
                    json.push_str("]\n");
                    json
                }
                Err(_) => "[]\n".to_string(),
            }
        }

        _ => "{\"error\":\"unknown method\"}\n".to_string(),
    };

    conn.write_all(response.as_bytes())?;
    Ok(())
}

/// Extract `"method"` from a JSON line like `{"method":"get_status"}`.
fn parse_method(line: &str) -> &str {
    line.split("\"method\"")
        .nth(1)
        .and_then(|s| s.split('"').nth(1))
        .unwrap_or("")
}

/// Extract a string field from a simple JSON object line.
fn extract_json_str<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let search = format!("\"{key}\"");
    line.split(&search).nth(1)?.split('"').nth(1)
}

/// Extract a u64 field from a simple JSON object line.
fn extract_json_u64(line: &str, key: &str) -> Option<u64> {
    extract_json_str(line, key)?.parse().ok()
}
