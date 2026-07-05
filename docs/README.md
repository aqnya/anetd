# anetd

> A lightweight DNS proxy and ad-blocking domain filter daemon for Android.

`anetd` intercepts the native `dnsproxyd` Unix domain socket used by the Android framework, inserting an ad-blocking rule engine into the DNS resolution path. It provides **transparent interception and blacklist-based blocking** of DNS requests at the framework level. It can also operate as a standalone DNS server.

---

## Features

- **Socket Hijacking Mode** — Renames `/dev/socket/dnsproxyd`, binds to the original path, and transparently forwards allowed requests to the real netd service.
- **Standalone DNS Server** — Built-in UDP/TCP DNS server with configurable listen port and upstream address.
- **Adblock Rule Engine** — A bespoke, lightweight matcher supporting `||domain.com^` for blocking and `@@||domain.com^` for allowlist exceptions. Cosmetic and element-hiding rules are silently ignored.
- **DNS Wire Protocol** — Parses `getaddrinfo`, `gethostbyname`, and `resnsend` (the three dnsproxyd command types), extracts the hostname, and returns a crafted NXDOMAIN response when a rule matches.
- **Hot Reload** — Watches rule files via inotify and atomically swaps in new rulesets without restarting.
- **Daemon Mode** — Supports daemonization, PID files, and rolling log retention.
- **Flexible Configuration** — CLI arguments > TOML config file > hardcoded defaults, merged by precedence.

---

## Requirements

| Item        | Details                                     |
| ----------- | ------------------------------------------- |
| OS          | Android (root required)                     |
| Build tool  | Rust 1.85+ (edition 2024)                   |
| Runtime dep | `/dev/socket/dnsproxyd` (socket hijack mode) |

---

## Quick Start

### Build

```sh
git clone https://github.com/aqnya/anetd.git
cd anetd
cargo build --release
```

The binary is produced at `target/release/anetd`.

### Configuration

Create `/data/adb/anetd/config.toml` (or provide a custom path):

```toml
rules = "/data/adb/anetd/rules"
standalone = false
multi_thread = true
dns_server = false
dns_port = 53
dns_upstream = "8.8.8.8:53"
```

### Run

```sh
# Socket hijacking mode (default)
anetd --rules /data/adb/anetd/rules

# Standalone DNS server mode
anetd --rules /data/adb/anetd/rules --dns-server --dns-port 5353

# Background daemon
anetd --rules /data/adb/anetd/rules --standalone
```

---

## CLI Reference

```
Usage: anetd --rules <PATH> [OPTIONS]

Options:
  -r, --rules <PATH>         Path to rule file(s) or directory; supports comma-separated values
  -f, --config-file <PATH>   Path to TOML configuration file (default: /data/adb/anetd/config.toml)
  -s, --standalone           Run as a background daemon and log to file
  -m, --multi-thread         Enable multi-thread Tokio runtime
      --dns-server           Enable built-in DNS server (UDP/TCP)
      --dns-port <PORT>      DNS server listen port (default: 53)
      --dns-upstream <ADDR>  Upstream DNS server address (default: 8.8.8.8:53)
  -h, --help                 Print help
```

---

## Rule File Format

`anetd` accepts a subset of the Adblock filter syntax relevant to DNS-level blocking. One rule per line; blank lines, comments (lines starting with `!`), and headers (lines starting with `[`) are ignored.

| Example                          | Meaning                                       |
| -------------------------------- | --------------------------------------------- |
| `\|\|example.com^`               | Block example.com and all sub-domains          |
| `\|\|ads.example.com^`           | Block ads.example.com and its sub-domains      |
| `@@\|\|good.example.com^`        | Allow good.example.com (overrides block rules) |
| `\|\|*.wildcard.domain.xyz^`     | Wildcard `*` is normalized before matching    |

Multiple rule files can be loaded by providing comma-separated paths or a directory to `--rules`. If a file changes on disk, inotify triggers an automatic, atomic reload of the entire ruleset.

---

## Architecture

```
Android App
    │
    ▼
libc (getaddrinfo / gethostbyname)
    │
    ▼
netd (dnsproxyd Unix socket)
    │
    ▼
┌─────────────────────────────────┐
│           anetd                 │
│                                 │
│  ┌──────────┐   ┌────────────┐  │
│  │ Command  │──▶│  RuleSet   │  │
│  │ Parser   │   │ (Adblock)  │  │
│  └──────────┘   └──┬────┬────┘  │
│                    │    │       │
│                 BLOCK ALLOW     │
│                    │    │       │
│              NXDOMAIN  │        │
│                    ▼    ▼       │
│            dnsproxyd_real       │
│            (original netd)       │
└─────────────────────────────────┘
```

The original socket is restored on process exit (`SIGINT` / `SIGTERM`).

---

## Directory Layout

```
src/
├── main.rs         Entry point
├── cli.rs          CLI argument parsing & config merging
├── config.rs       Constants & TOML config loading
├── daemon.rs       Daemonization
├── logging.rs      Logging setup (console / rolling file)
├── server.rs       Main server loop (socket hijack / DNS server dispatch)
├── session.rs      Client session handling & transparent proxy
├── protocol.rs     dnsproxyd wire protocol helpers
├── signal.rs       Signal handling & socket restoration
├── dns_server.rs   Standalone DNS server (UDP/TCP)
├── dns/
│   ├── mod.rs
│   ├── nxdomain.rs  NXDOMAIN response builder
│   ├── status.rs    dnsproxyd status codes
│   ├── wire.rs      DNS wire-format I/O
│   └── response/
│       ├── addrinfo.rs  getaddrinfo response builder
│       ├── hostent.rs   gethostbyname response builder
│       └── raw.rs       resnsend raw response builder
├── handlers/
│   ├── mod.rs           Command handler registry
│   ├── getaddrinfo.rs   getaddrinfo handler
│   ├── gethostbyname.rs gethostbyname handler
│   └── resnsend.rs      resnsend handler
├── rules/
│   ├── mod.rs
│   ├── adblock.rs  Rule matching engine
│   ├── loader.rs   Rule file loading & compilation
│   └── watcher.rs  inotify hot-reload watcher
scripts/
└── probe.py        dnsproxyd response format probe tool
```

---

## License

This project is released under the [GNU General Public License v3.0](LICENSE).

---

## See Also

- [Adblock Plus Filter Syntax](https://help.adblockplus.org/hc/en-us/articles/360062733293-How-to-write-filters)
- [AOSP netd / DnsProxyListener](https://cs.android.com/android/platform/superproject/+/android-latest-release:packages/modules/Connectivity/staticlibs/netd/libnetdutils/include/netdutils/ResponseCode.h)
