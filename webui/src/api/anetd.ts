/**
 * anetd daemon API via Unix socket.
 *
 * Communicates with the anetd daemon through its Unix domain socket
 * using `ksu.exec("nc -U <socket>")`.  The protocol is simple JSON-line:
 *   → `{"method":"get_status"}\n`
 *   ← `{"running":true,...}\n`
 */

import { ksu } from "./ksu";

// ── Socket path ────────────────────────────────────

const SOCKET = "/data/adb/modules/anetd/webui.sock";

function send(method: string, extra?: Record<string, unknown>): string {
  const req: Record<string, unknown> = { method, ...extra };
  const json = JSON.stringify(req);
  // Escape single quotes for shell: replace ' → '\''
  const esc = json.replace(/'/g, "'\\''");
  const cmd = `echo '${esc}' | nc -U '${SOCKET}' -w 3 2>/dev/null`;
  const r = ksu.exec(cmd);
  if (r.stderr) {
    console.error("[anetd:unix]", r.stderr);
  }
  return r.stdout || "";
}

function sendJson<T>(method: string, extra?: Record<string, unknown>): T {
  const raw = send(method, extra);
  try {
    return JSON.parse(raw);
  } catch {
    return {} as T;
  }
}

export interface AnetdStatus {
  running: boolean;
  pid: number | null;
  dnsFilterEnabled: boolean;
  uptime: string;
}

export interface StatusResponse {
  running: boolean;
  blocked: number;
  dns_queries: number;
  rules_count: number;
  block_rules: number;
  allow_rules: number;
  pid: number | null;
  uptime: string;
  dnsFilterEnabled: boolean;
}

export interface RuleFile {
  path: string;
  hash: string;
}

export interface ReloadResponse {
  ok: boolean;
  rules_count: number;
  block_rules: number;
  allow_rules: number;
}

export async function getStatus(): Promise<AnetdStatus> {
  const s = sendJson<StatusResponse>("get_status");
  if (s.running === undefined) s.running = true; // assume running if responded
  return {
    running: s.running ?? true,
    pid: s.pid ?? null,
    dnsFilterEnabled: s.dnsFilterEnabled ?? true,
    uptime: s.uptime || `${(s.blocked ?? 0)} blocked`,
  };
}

export async function getStatusDebug(): Promise<Record<string, unknown>> {
  return sendJson("get_status_debug");
}

export async function loadRules(): Promise<RuleFile[]> {
  const raw = send("load_rules");
  try {
    return JSON.parse(raw);
  } catch {
    return [];
  }
}

export async function reloadRules(): Promise<ReloadResponse> {
  return sendJson<ReloadResponse>("reload_rules");
}

export async function toggleFilter(): Promise<boolean> {
  const r = ksu.exec(`sh '/data/adb/modules/anetd/toggle.sh'`);
  return r.errno === 0;
}

export async function loadConfig(): Promise<string> {
  const r = sendJson<{ content: string }>("load_config");
  return (r.content || "").replace(/\\n/g, "\n").replace(/\\"/g, '"').replace(/\\\\/g, "\\");
}

export async function saveConfig(content: string): Promise<boolean> {
  return sendJson<{ ok: boolean }>("save_config", { content }).ok;
}

export async function loadLogs(lines: number = 100): Promise<string[]> {
  const raw = send("load_logs", { count: lines });
  try {
    return JSON.parse(raw);
  } catch {
    return [];
  }
}

export async function getLogsRaw(lines: number = 500): Promise<string> {
  const arr = await loadLogs(lines);
  return arr.join("\n");
}
