/**
 * KSU / MMRL WebUI bridge API.
 * These functions are injected by the manager's WebView at runtime.
 * When running in a browser during development, mock implementations are used.
 */

// ── DEBUG: log bridge availability ──
const dbg = (window as any).__DEBUG;
function dlog(cls: string, msg: string) {
  if (dbg) dbg.add(cls, msg);
  console.log("[anetd:debug:ksu]", cls, msg);
}
dlog("step", "=== ksu.ts module loaded ===");
dlog("info", "globalThis.ksu present: " + (typeof (globalThis as any).ksu !== "undefined"));

export interface ExecResult {
  errno: number;
  stdout: string;
  stderr: string;
}

interface KsuBridge {
  exec(cmd: string): ExecResult;
  toast(msg: string): void;
  moduleInfo(): { id: string; name: string; version: string; versionCode: number };
  fullScreen(enable: boolean): void;
}

declare global {
  // eslint-disable-next-line no-var
  var ksu: KsuBridge | undefined;
}

function mockExec(cmd: string): ExecResult {
  console.debug("[ksu.mock] exec:", cmd);
  return { errno: 0, stdout: "", stderr: "" };
}

function mockToast(msg: string): void {
  console.debug("[ksu.mock] toast:", msg);
}

function mockModuleInfo() {
  return { id: "anetd", name: "Anetd", version: "v0.1.0", versionCode: 1 };
}

/**
 * Safe exec wrapper: catches any errors and returns a fallback ExecResult
 * so that a bad ksu.exec call never crashes the app.
 */
function safeExec(cmd: string): ExecResult {
  try {
    if (!globalThis.ksu) {
      dlog("info", "ksu.mock exec: " + cmd.slice(0, 80));
      return mockExec(cmd);
    }
    dlog("info", "ksu.real exec: " + cmd.slice(0, 80));
    const r = globalThis.ksu.exec(cmd);
    dlog("info", "ksu.real exec OK: errno=" + r.errno + " stdout_len=" + (r.stdout?.length || 0));
    return r;
  } catch (e: any) {
    dlog("err", "ksu.exec CRASHED: " + (e?.message || e) + " | cmd=" + cmd.slice(0, 60));
    return { errno: -1, stdout: "", stderr: e?.message || String(e) };
  }
}

function safeToast(msg: string): void {
  try {
    if (globalThis.ksu) {
      globalThis.ksu.toast(msg);
    } else {
      mockToast(msg);
    }
  } catch (e: any) {
    dlog("err", "ksu.toast CRASHED: " + (e?.message || e));
  }
}

function safeModuleInfo() {
  try {
    if (globalThis.ksu) {
      return globalThis.ksu.moduleInfo();
    }
    return mockModuleInfo();
  } catch (e: any) {
    dlog("err", "ksu.moduleInfo CRASHED: " + (e?.message || e));
    return mockModuleInfo();
  }
}

function safeFullScreen(enable: boolean): void {
  try {
    if (globalThis.ksu) {
      globalThis.ksu.fullScreen(enable);
    }
  } catch (e: any) {
    dlog("err", "ksu.fullScreen CRASHED: " + (e?.message || e));
  }
}

export const ksu: KsuBridge = {
  exec: safeExec,
  toast: safeToast,
  moduleInfo: safeModuleInfo,
  fullScreen: safeFullScreen,
};
