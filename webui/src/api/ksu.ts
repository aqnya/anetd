/**
 * KSU / MMRL WebUI bridge API.
 * These functions are injected by the manager's WebView at runtime.
 * When running in a browser during development, mock implementations are used.
 */

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

export const ksu: KsuBridge = globalThis.ksu ?? {
  exec: mockExec,
  toast: mockToast,
  moduleInfo: mockModuleInfo,
  fullScreen: () => {},
};
