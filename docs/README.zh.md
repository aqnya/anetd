# anetd

> 适用于 Android 的轻量级 DNS 代理与广告域名过滤守护进程。

`anetd` 通过劫持 Android 系统原生的 `dnsproxyd` Unix 域套接字，在 DNS 解析路径上植入广告过滤规则引擎，实现对 Android 框架层 DNS 请求的**透明拦截与黑名单阻断**。同时也支持作为独立 DNS 服务器运行。

---

## 特性

- **Socket 劫持模式** — 重命名 `/dev/socket/dnsproxyd` 并接管该路径，透明中转未被拦截的 DNS 请求到原始 netd 服务。
- **独立 DNS 服务器模式** — 内置 UDP/TCP DNS 服务器，可绑定自定义端口与上游服务器。
- **Adblock 规则引擎** — 自研轻量级匹配器，支持 `||domain.com^` 阻断与 `@@||domain.com^` 白名单语法。忽略无 DNS 语义的页面元素隐藏规则。
- **DNS 协议原语** — 解析 `getaddrinfo`、`gethostbyname`、`resnsend` 三种 dnsproxyd 命令，精确提取主机名，命中黑名单时返回构造的 NXDOMAIN 响应。
- **规则热更新** — 基于 inotify 监视规则文件变动，自动重载，无需重启进程。
- **DNS 缓存** — 内建 DNS 服务器的 TTL 感知内存缓存，减少冗余上游查询，降低功耗。
- **网络变化检测** — 基于 netlink 监控默认路由变化（WiFi ↔ 移动数据切换），自动清空 DNS 缓存以淘汰上一网络的 CDN IP。每次 DNS 查询和 netd 请求均创建全新套接字，无需维护长连接。
- **后台守护进程** — 支持 daemonize、PID 文件、日志滚动归档。
- **省电模式** — 可选模式，缩小 DNS 缓存以降低功耗。
- **灵活的配置层级** — CLI 参数 > TOML 配置文件 > 硬编码默认值，按优先级合并。

---

## 系统要求

| 项目       | 说明                                |
| ---------- | ----------------------------------- |
| 操作系统   | Android（需 root 权限）             |
| 构建工具   | Rust 1.85+（edition 2024）          |
| 运行依赖   | `/dev/socket/dnsproxyd`（socket 劫持模式） |

---

## 快速开始

### 构建

```sh
git clone https://github.com/aqnya/anetd.git
cd anetd
cargo build --release
```

产物位于 `target/release/anetd`。

### 配置文件

创建 `/data/adb/anetd/config.toml`（亦可自行指定路径）：

```toml
rules = "/data/adb/anetd/rules"
standalone = false
multi_thread = true
dns_server = false
dns_port = 53
dns_upstream = "8.8.8.8:53"
battery_saver = false
```

### 运行

```sh
# Socket 劫持模式（默认）
anetd --rules /data/adb/anetd/rules

# 独立 DNS 服务器模式
anetd --rules /data/adb/anetd/rules --dns-server --dns-port 5353

# 后台守护进程
anetd --rules /data/adb/anetd/rules --standalone
```

---

## 命令行参考

```
Usage: anetd --rules <PATH> [OPTIONS]

Options:
  -r, --rules <PATH>         规则文件或目录路径，多个路径以逗号分隔
  -f, --config-file <PATH>   TOML 配置文件路径（默认：/data/adb/anetd/config.toml）
  -s, --standalone           以守护进程模式运行，日志写入文件
  -m, --multi-thread         启用多线程 Tokio 运行时
      --dns-server           启用内建 DNS 服务器（UDP/TCP）
      --dns-port <PORT>      DNS 服务器监听端口（默认：53）
      --dns-upstream <ADDR>  上游 DNS 服务器地址（默认：8.8.8.8:53）
      --battery-saver        启用省电模式（缩小缓存）
  -h, --help                 打印帮助信息
```

---

## 规则文件格式

支持 Adblock 标准语法中与 DNS 层面相关的子集。每行一条规则，空行、`!` 开头（注释）、`[` 开头（头部标记）的行将被忽略。

| 示例                             | 含义                                     |
| -------------------------------- | ---------------------------------------- |
| `\|\|example.com^`               | 阻断 example.com 及其所有子域名           |
| `\|\|ads.example.com^`           | 阻断 ads.example.com 及其子域名           |
| `@@\|\|good.example.com^`        | 将 good.example.com 从黑名单中排除        |
| `\|\|*.wildcard.domain.xyz^`     | 通配符 `*` 会被规范化后匹配              |

规则文件支持从多个文件中收集，仅需在 `--rules` 中以逗号分隔，或指向一个目录。运行时若文件内容发生变更，inotify 将自动触发规则重载。

---

## 架构简述

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
│            (原始 netd socket)    │
└─────────────────────────────────┘
```

进程退出时自动恢复原始 socket（`SIGINT` / `SIGTERM` 处理）。

---

## 项目结构

```
src/
├── main.rs         入口点
├── cli.rs          命令行参数解析与配置合并
├── config.rs       常量定义、TOML 配置文件加载
├── daemon.rs       守护进程化
├── logging.rs      日志初始化（控制台 / 滚动文件）
├── network.rs     netlink 路由监控，网络变化检测
├── server.rs       服务器主循环（socket 劫持 / DNS 服务器分发）
├── session.rs      客户端会话处理与透明代理
├── protocol.rs     dnsproxyd 线路协议辅助工具
├── signal.rs       系统信号处理与 socket 恢复
├── dns_server.rs   独立 DNS 服务器（UDP/TCP）
├── dns/
│   ├── mod.rs
│   ├── cache.rs     DNS 响应缓存（TTL 感知）
│   ├── nxdomain.rs 构造 NXDOMAIN 响应
│   ├── status.rs   dnsproxyd 状态码枚举
│   ├── wire.rs     DNS 线路协议读写
│   └── response/
│       ├── mod.rs
│       ├── addrinfo.rs  getaddrinfo 响应构造
│       ├── hostent.rs   gethostbyname 响应构造
│       └── raw.rs       resnsend 原始响应构造
├── handlers/
│   ├── mod.rs          命令处理器注册表
│   ├── getaddrinfo.rs  getaddrinfo 处理器
│   ├── gethostbyname.rs gethostbyname 处理器
│   └── resnsend.rs     resnsend 处理器
├── rules/
│   ├── mod.rs
│   ├── adblock.rs  规则匹配引擎
│   ├── loader.rs   规则文件加载与编译
│   └── watcher.rs  inotify 热重载监视器
scripts/
└── probe.py        dnsproxyd 响应格式探测工具
```

---

## 许可证

本软件以 [GNU General Public License v3.0](LICENSE) 发布。

---

## 相关项目

- [Adblock Plus Filter Syntax](https://help.adblockplus.org/hc/en-us/articles/360062733293-How-to-write-filters)
- [AOSP netd / DnsProxyListener](https://cs.android.com/android/platform/superproject/+/android-latest-release:packages/modules/Connectivity/staticlibs/netd/libnetdutils/include/netdutils/ResponseCode.h)
