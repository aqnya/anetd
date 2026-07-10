/**
 * KSU WebUI bridge.
 *
 * Provides safe wrappers around `globalThis.ksu.exec()`.
 * In browser dev mode, mock implementations are used.
 */

export interface ExecResult {
  errno: number;
  stdout: string;
  stderr: string;
}

interface KsuBridge {
  exec(cmd: string): ExecResult;
  toast(msg: string): void;
}

declare global {
  var ksu: KsuBridge | undefined;
}

function mockExec(cmd: string): ExecResult {
  console.debug("[ksu.mock] exec:", cmd.slice(0, 80));
  return { errno: 0, stdout: "", stderr: "" };
}

function safeExec(cmd: string): ExecResult {
  try {
    if (!globalThis.ksu) {
      return mockExec(cmd);
    }
    const r = globalThis.ksu.exec(cmd);
    return {
      errno: r.errno ?? -1,
      stdout: r.stdout ?? "",
      stderr: r.stderr ?? "",
    };
  } catch (e: any) {
    return { errno: -1, stdout: "", stderr: e?.message || String(e) };
  }
}

function safeToast(msg: string): void {
  try {
    if (globalThis.ksu) {
      globalThis.ksu.toast(msg);
    } else {
      console.debug("[ksu.mock] toast:", msg);
    }
  } catch (e: any) {
    console.error("[ksu] toast error:", e);
  }
}

export const ksu: KsuBridge = {
  exec: safeExec,
  toast: safeToast,
};
