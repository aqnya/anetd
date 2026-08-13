/**
 * KSU / MMRL WebUI bridge.
 *
 * The KernelSU manager exposes `window.ksu` with:
 *   - synchronous  `exec(cmd)` returning stdout
 *   - asynchronous `exec(cmd, optionsJson, callbackName)` where `callbackName`
 *     names a function registered on `window`; the native side invokes it with
 *     positional `(errno, stdout, stderr)` arguments.
 * We prefer the async form so the UI never freezes, probe the bridge once at
 * startup, and fall back to older / standalone bridges (`exec(cmd, fn)` or
 * plain synchronous `exec`) when needed.
 */

export interface ExecResult {
  errno: number;
  stdout: string;
  stderr: string;
}

interface ModuleInfo {
  moduleDir?: string;
  moduleId?: string;
  id?: string;
  name?: string;
}

interface KsuNative {
  exec(command: string): ExecResult | string | void;
  exec(command: string, callbackOrName: unknown, maybeName?: string): ExecResult | string | void;
  toast(message: string): void;
  moduleInfo?(): string | ModuleInfo | undefined;
}

declare global {
  interface Window {
    ksu?: KsuNative;
  }
}

function normalize(result: unknown): ExecResult {
  if (result && typeof result === "object") {
    const r = result as Record<string, unknown>;
    const errno =
      typeof r.errno === "number" ? r.errno :
      typeof r.code === "number" ? r.code :
      typeof r.exitCode === "number" ? r.exitCode : -1;
    return {
      errno,
      stdout: typeof r.stdout === "string" ? r.stdout : "",
      stderr: typeof r.stderr === "string" ? r.stderr : "",
    };
  }
  if (typeof result === "string") {
    // Some bridges return stdout directly for sync exec.
    return { errno: 0, stdout: result, stderr: "" };
  }
  return { errno: -1, stdout: "", stderr: "" };
}

function mockResult(command: string): ExecResult {
  console.debug("[ksu.mock] exec:", command.slice(0, 120));
  return { errno: 0, stdout: "", stderr: "" };
}

type ExecMode = "async3" | "async2" | "asyncFn" | "sync" | "mock";

let modePromise: Promise<ExecMode> | null = null;
let nameCounter = 0;

function genName(): string {
  nameCounter += 1;
  return `anetd_exec_${Date.now()}_${nameCounter}`;
}

function setGlobal(name: string, value: unknown): void {
  (window as unknown as Record<string, unknown>)[name] = value;
}

function delGlobal(name: string): void {
  delete (window as unknown as Record<string, unknown>)[name];
}

/** Normalize the async callback's positional `(errno, stdout, stderr)` args. */
function normalizeArgs(errno: unknown, stdout: unknown, stderr: unknown): ExecResult {
  if (errno && typeof errno === "object") {
    // Some bridges pass a single result object instead.
    return normalize(errno);
  }
  return {
    errno: typeof errno === "number" ? errno : -1,
    stdout: typeof stdout === "string" ? stdout : "",
    stderr: typeof stderr === "string" ? stderr : "",
  };
}

/** Probe one exec style; resolves within `timeoutMs` whether or not it fires. */
function probe(
  timeoutMs: number,
  start: (done: (ok: boolean, value?: unknown) => void) => void,
): Promise<{ ok: boolean; value?: unknown }> {
  return new Promise((resolve) => {
    let settled = false;
    const done = (ok: boolean, value?: unknown) => {
      if (!settled) {
        settled = true;
        resolve({ ok, value });
      }
    };
    const timer = window.setTimeout(() => done(false), timeoutMs);
    try {
      start((ok, value) => {
        window.clearTimeout(timer);
        done(ok, value);
      });
    } catch {
      window.clearTimeout(timer);
      done(false);
    }
  });
}

async function detectMode(): Promise<ExecMode> {
  if (modePromise) return modePromise;
  modePromise = detectModeOnce();
  return modePromise;
}

async function detectModeOnce(): Promise<ExecMode> {
  const ksu = window.ksu;
  if (!ksu || typeof ksu.exec !== "function") return "mock";

  // 1) Official async protocol: exec(cmd, optionsJson, callbackName)
  const m3 = await probe(900, (done) => {
    const name = genName();
    setGlobal(name, (errno: unknown, stdout: unknown, stderr: unknown) => {
      delGlobal(name);
      done(true, normalizeArgs(errno, stdout, stderr));
    });
    const ret = ksu.exec("echo ok", "{}", name);
    if (typeof ret === "string" && ret.trim()) {
      delGlobal(name);
      done(true, { errno: 0, stdout: ret, stderr: "" });
    }
  });
  if (m3.ok) return "async3";

  // 2) Two-arg official variant: exec(cmd, callbackName)
  const m2 = await probe(900, (done) => {
    const name = genName();
    setGlobal(name, (errno: unknown, stdout: unknown, stderr: unknown) => {
      delGlobal(name);
      done(true, normalizeArgs(errno, stdout, stderr));
    });
    ksu.exec("echo ok", name);
  });
  if (m2.ok) return "async2";

  // 3) Standalone-style bridges: exec(cmd, callbackFn)
  const mf = await probe(900, (done) => {
    ksu.exec("echo ok", (raw: unknown) => done(true, normalize(raw)));
  });
  if (mf.ok) return "asyncFn";

  // 4) Synchronous bridge: exec(cmd) -> stdout
  try {
    const ret = ksu.exec("echo ok");
    if (typeof ret === "string" && ret.trim()) return "sync";
  } catch {
    /* fall through */
  }
  return "mock";
}

/**
 * Run a shell command asynchronously. Never throws; errors are reported
 * through the result's `errno`/`stderr` (or a timeout pseudo-result).
 */
export function execAsync(command: string, timeoutMs = 8000): Promise<ExecResult> {
  return (async () => {
    const ksu = window.ksu;
    if (!ksu || typeof ksu.exec !== "function") return mockResult(command);

    const mode = await detectMode();
    if (mode === "mock") return mockResult(command);

    if (mode === "sync") {
      try {
        return normalize(ksu.exec(command));
      } catch (e) {
        return { errno: -1, stdout: "", stderr: e instanceof Error ? e.message : String(e) };
      }
    }

    return new Promise<ExecResult>((resolve) => {
      let settled = false;
      const finish = (r: ExecResult) => {
        if (!settled) {
          settled = true;
          resolve(r);
        }
      };
      const name = genName();
      const timer = window.setTimeout(() => {
        delGlobal(name);
        finish({ errno: -1, stdout: "", stderr: "timeout" });
      }, timeoutMs);

      const onResult = (errno: unknown, stdout?: unknown, stderr?: unknown) => {
        window.clearTimeout(timer);
        delGlobal(name);
        finish(normalizeArgs(errno, stdout, stderr));
      };

      setGlobal(name, onResult);
      try {
        const ret =
          mode === "async3"
            ? ksu.exec(command, JSON.stringify({}), name)
            : mode === "async2"
              ? ksu.exec(command, name)
              : ksu.exec(command, (raw: unknown) => onResult(raw));
        // Some bridges execute synchronously and return stdout directly.
        if (typeof ret === "string" && ret) {
          window.clearTimeout(timer);
          delGlobal(name);
          finish({ errno: 0, stdout: ret, stderr: "" });
        }
      } catch (e) {
        window.clearTimeout(timer);
        delGlobal(name);
        finish({ errno: -1, stdout: "", stderr: e instanceof Error ? e.message : String(e) });
      }
    });
  })();
}

/** Resolve the module directory (KSU exposes it via `moduleInfo()`). */
function resolveModuleDir(): string {
  try {
    const info = window.ksu?.moduleInfo?.();
    if (typeof info === "string" && info) {
      if (info.startsWith("{")) {
        try {
          const parsed = JSON.parse(info) as ModuleInfo;
          if (typeof parsed.moduleDir === "string" && parsed.moduleDir) {
            return parsed.moduleDir;
          }
          const id = parsed.moduleId ?? parsed.id;
          if (typeof id === "string" && id) return `/data/adb/modules/${id}`;
        } catch {
          /* fall through */
        }
      }
      return `/data/adb/modules/${info}`;
    }
    if (info && typeof info === "object") {
      if (typeof info.moduleDir === "string" && info.moduleDir) return info.moduleDir;
      const id = info.moduleId ?? info.id;
      if (typeof id === "string" && id) return `/data/adb/modules/${id}`;
    }
  } catch {
    /* fall through */
  }
  return "/data/adb/modules/anetd";
}

/** Module install directory (cached). */
export const MODULE_DIR: string = resolveModuleDir();

/** Show a toast through the KSU bridge (no-op in plain browsers). */
export function toast(msg: string): void {
  try {
    if (window.ksu?.toast) {
      window.ksu.toast(msg);
    } else {
      console.info("[ksu.mock] toast:", msg);
    }
  } catch {
    /* ignore */
  }
}
