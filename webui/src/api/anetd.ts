/**
 * anetd daemon API.
 *
 * Speaks the JSON-line protocol over the daemon's Unix domain socket. The
 * socket itself is only reachable from the KSU WebView, so requests are
 * bridged through `ksu.exec("echo '...' | nc -U <socket>")`.
 */

import { execAsync, MODULE_DIR, toast as ksuToast } from "./ksu";

const SOCKET_PATH = `${MODULE_DIR}/webui.sock`;

export class ApiError extends Error {
  readonly kind: "offline" | "bad-response";
  constructor(kind: ApiError["kind"], message: string) {
    super(message);
    this.kind = kind;
  }
}

export interface DaemonStatus {
  ok: boolean;
  running: boolean;
  version: string;
  pid: number | null;
  uptime_sec: number;
  uptime: string;
  dns_queries: number;
  blocked: number;
  rules_count: number;
  block_rules: number;
  allow_rules: number;
  dns_filter_enabled: boolean;
  mode: "hijack" | "dns-server";
  battery_saver: boolean;
  multi_thread: boolean;
  standalone: boolean;
  dns_port: number;
  dns_upstream: string;
  rules_path: string;
  socket_path: string;
  config_path: string;
  log_path: string;
}

export interface RuleFile {
  path: string;
  hash: string;
}

export interface RulesResult {
  ok: boolean;
  files: RuleFile[];
}

export interface ReloadResult {
  ok: boolean;
  rules_count: number;
  block_rules: number;
  allow_rules: number;
  error?: string;
}

export type ConfigValue = string | boolean | number;

export interface ConfigResult {
  ok: boolean;
  content: string;
  values: Record<string, ConfigValue>;
}

export interface LogsResult {
  ok: boolean;
  lines: string[];
}

/** Shell single-quote escaping (safe for any JSON payload). */
function shellEscape(s: string): string {
  return `'${s.replace(/'/g, `'\\''`)}'`;
}

/** Cache the first `nc` invocation that works on this device. */
let ncCommand: string | null = null;

async function runSocket(json: string): Promise<string> {
  const request = shellEscape(json);
  const candidates =
    ncCommand !== null
      ? [ncCommand]
      : [
          `echo ${request} | nc -U ${shellEscape(SOCKET_PATH)} -w 3 2>/dev/null`,
          `echo ${request} | ncat -U ${shellEscape(SOCKET_PATH)} -w 3 2>/dev/null`,
          `echo ${request} | toybox nc -U ${shellEscape(SOCKET_PATH)} -w 3 2>/dev/null`,
          `echo ${request} | nc -U ${shellEscape(SOCKET_PATH)} 2>/dev/null`,
        ];

  let lastErr = "";
  for (const cmd of candidates) {
    const r = await execAsync(cmd, 8000);
    if (r.stdout && r.stdout.trim()) {
      ncCommand = cmd;
      return r.stdout;
    }
    lastErr = r.stderr || `exit ${r.errno}`;
  }
  throw new ApiError(
    "offline",
    lastErr ? `守护进程不可用 (${lastErr})` : "守护进程不可用",
  );
}

async function request<T>(method: string, extra?: Record<string, unknown>): Promise<T> {
  const line = JSON.stringify({ method, ...extra });
  const raw = await runSocket(line);
  const text = raw.trim();
  if (!text) throw new ApiError("offline", "空响应");
  try {
    const obj = JSON.parse(text);
    if (obj && typeof obj === "object") return obj as T;
  } catch {
    /* fall through */
  }
  throw new ApiError("bad-response", `无法解析守护进程响应: ${text.slice(0, 80)}`);
}

export const api = {
  getStatus: () => request<DaemonStatus>("get_status"),
  loadRules: () => request<RulesResult>("load_rules"),
  reloadRules: () => request<ReloadResult>("reload_rules"),
  loadConfig: () => request<ConfigResult>("load_config"),
  saveConfig: (content: string) =>
    request<{ ok: boolean; error?: string }>("save_config", { content }),
  loadLogs: (count = 200) => request<LogsResult>("load_logs", { count }),
};

/** Toggle the adblock filter via the module's toggle script. */
export async function toggleFilter(): Promise<boolean> {
  const r = await execAsync(`sh ${shellEscape(`${MODULE_DIR}/toggle.sh`)}`, 10000);
  return r.errno === 0;
}

/** Build the command that ensures the daemon is running (restarts if alive). */
function ensureDaemonCommand(): string {
  const pidFile = shellEscape(`${MODULE_DIR}/log/anetd.pid`);
  const postFs = shellEscape(`${MODULE_DIR}/post-fs-data.sh`);
  return (
    `PID=$(cat ${pidFile} 2>/dev/null); ` +
    `if [ -n "$PID" ] && [ -d "/proc/$PID" ]; then ` +
    `kill -TERM "$PID" 2>/dev/null; ` +
    `for i in 1 2 3 4 5 6 7 8 9 10; do [ ! -d "/proc/$PID" ] && break; sleep 1; done; ` +
    `fi; ` +
    `sh ${postFs}`
  );
}

/** Stop and start the daemon via the module lifecycle scripts. */
export async function restartDaemon(): Promise<boolean> {
  const r = await execAsync(ensureDaemonCommand(), 20000);
  return r.errno === 0;
}

/** Start the daemon (safe to run when it is currently stopped). */
export async function startDaemon(): Promise<boolean> {
  const r = await execAsync(ensureDaemonCommand(), 20000);
  return r.errno === 0;
}

/** Truncate a file (used to clear the log). */
export async function clearLogFile(path: string): Promise<boolean> {
  const r = await execAsync(`: > ${shellEscape(path)}`, 5000);
  return r.errno === 0;
}

/** Write a UTF-8 string to a file via base64 (avoids shell escaping issues). */
export async function writeFileBase64(path: string, content: string): Promise<boolean> {
  const bytes = new TextEncoder().encode(content);
  let binary = "";
  for (const b of bytes) binary += String.fromCharCode(b);
  let b64 = "";
  try {
    b64 = btoa(binary);
  } catch {
    return false;
  }
  const r = await execAsync(`echo '${b64}' | base64 -d > ${shellEscape(path)}`, 10000);
  return r.errno === 0;
}

export function toast(msg: string): void {
  ksuToast(msg);
}
