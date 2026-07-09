/**
 * anetd daemon API.
 *
 * Communicates with the running anetd process via shell commands and
 * signal files. In the future this could be replaced by a proper HTTP API.
 */

import { ksu, type ExecResult } from "./ksu";

const MODDIR = "/data/adb/modules/anetd";
const PID_FILE = `${MODDIR}/log/anetd.pid`;
const RULES_DIR = `${MODDIR}/rules`;
const CONFIG_FILE = "/data/adb/anetd/config.toml";
const STATE_FILE = `${MODDIR}/log/dns_off`;

export interface AnetdStatus {
  running: boolean;
  pid: number | null;
  dnsFilterEnabled: boolean;
  uptime: string;
}

export interface RuleEntry {
  raw: string;
  type: "block" | "allow" | "comment" | "header" | "blank" | "inline-comment";
  original: string; // for comments etc, the original line
}

/** Run a shell command as root and return the result. */
function sh(cmd: string): ExecResult {
  return ksu.exec(cmd);
}

/** Check if the anetd daemon is running. */
export function getStatus(): AnetdStatus {
  const pidRaw = sh(`cat "${PID_FILE}" 2>/dev/null`).stdout.trim();
  const pid = pidRaw ? parseInt(pidRaw, 10) : null;

  let running = false;
  let uptime = "—";
  if (pid && !isNaN(pid)) {
    const ps = sh(`ps -p ${pid} -o pid= 2>/dev/null`).stdout.trim();
    running = ps === String(pid);
    if (running) {
      const etime = sh(`ps -p ${pid} -o etime= 2>/dev/null`).stdout.trim();
      uptime = etime || "running";
    }
  }

  const dnsOff = sh(`test -f "${STATE_FILE}" && echo 1 || echo 0`).stdout.trim();

  return {
    running,
    pid: running ? pid : null,
    dnsFilterEnabled: dnsOff !== "1",
    uptime,
  };
}

/** Load rule files from the rules directory. */
export function loadRules(): RuleEntry[] {
  const listing = sh(`ls "${RULES_DIR}" 2>/dev/null`).stdout.trim();
  if (!listing) return [];

  const files = listing.split("\n").filter(Boolean);
  const entries: RuleEntry[] = [];

  for (const file of files) {
    const filePath = `${RULES_DIR}/${file}`;
    const content = sh(`cat "${filePath}" 2>/dev/null`).stdout;
    const lines = content.split("\n");

    for (const raw of lines) {
      const trimmed = raw.trim();
      if (!trimmed) {
        entries.push({ raw, type: "blank", original: raw });
        continue;
      }
      if (trimmed.startsWith("!")) {
        entries.push({ raw, type: "comment", original: raw });
        continue;
      }
      if (trimmed.startsWith("[")) {
        entries.push({ raw, type: "header", original: raw });
        continue;
      }
      // inline comment: rule followed by comment
      const idx = raw.indexOf("!");
      if (idx > 0 && raw[idx - 1] === " ") {
        entries.push({ raw, type: "inline-comment", original: raw });
        continue;
      }
      if (trimmed.startsWith("@@||")) {
        entries.push({ raw, type: "allow", original: raw });
        continue;
      }
      if (trimmed.startsWith("||")) {
        entries.push({ raw, type: "block", original: raw });
        continue;
      }
      // catch-all as comment
      entries.push({ raw, type: "comment", original: raw });
    }
  }

  return entries;
}

/** Reload rules by sending SIGHUP to the daemon. */
export function reloadRules(): boolean {
  const pidRaw = sh(`cat "${PID_FILE}" 2>/dev/null`).stdout.trim();
  const pid = parseInt(pidRaw, 10);
  if (!pid || isNaN(pid)) return false;
  const r = sh(`kill -HUP ${pid} 2>/dev/null`);
  return r.errno === 0;
}

/** Toggle DNS filtering on/off (action.sh equivalent). */
export function toggleFilter(): boolean {
  const r = sh(`sh "${MODDIR}/toggle.sh"`);
  return r.errno === 0;
}

/** Read the current config.toml (raw text). */
export function loadConfig(): string {
  return sh(`cat "${CONFIG_FILE}" 2>/dev/null`).stdout;
}

/** Write config.toml and reload. */
export function saveConfig(content: string): boolean {
  // Write to temp, then move (atomic-ish)
  const tmp = `${CONFIG_FILE}.tmp`;
  const encoded = content.replace(/\\/g, "\\\\").replace(/'/g, "'\\''");
  const r = sh(`printf '%s' '${encoded}' > "${tmp}" && mv "${tmp}" "${CONFIG_FILE}"`);
  if (r.errno !== 0) return false;
  return reloadRules();
}

/** Get recent log lines. */
export function loadLogs(lines: number = 100): string[] {
  const logFile = `${MODDIR}/log/anetd.log`;
  const out = sh(`tail -n ${lines} "${logFile}" 2>/dev/null`).stdout;
  if (!out) return [];
  return out.split("\n");
}
