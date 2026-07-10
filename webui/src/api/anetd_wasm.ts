/**
 * anetd WASM API wrapper.
 *
 * Provides a clean, typed JavaScript interface to the WASM module.
 * This replaces webui/src/api/anetd.ts.
 */

import AnetdWasmFactory from "./anetd_wasm_core.js";

export interface AnetdStatus {
  running: boolean;
  pid: number | null;
  dnsFilterEnabled: boolean;
  uptime: string;
}

export interface RuleEntry {
  raw: string;
  type: "block" | "allow" | "comment" | "header" | "blank" | "inline-comment";
  original: string;
}

interface AnetdWasmRaw {
  _get_status(): number;
  _get_status_debug(): number;
  _load_rules(): number;
  _reload_rules(): number;
  _toggle_filter(): number;
  _load_config(): number;
  _save_config(ptr: number): number;
  _load_logs(lines: number): number;
  _check_url(url_ptr: number, rules_ptr: number): number;
  _malloc(size: number): number;
  _free(ptr: number): void;
  UTF8ToString(ptr: number): string;
  stringToUTF8(str: string, ptr: number, maxBytes: number): void;
  lengthBytesUTF8(str: string): number;
}

let _mod: Promise<AnetdWasmRaw>;
let _ready = false;

async function getMod(): Promise<AnetdWasmRaw> {
  if (_ready) return _mod as unknown as AnetdWasmRaw;
  if (!_mod) {
    _mod = AnetdWasmFactory() as Promise<AnetdWasmRaw>;
  }
  await _mod;
  _ready = true;
  return _mod;
}

/** Allocate a C string on the WASM heap, return its pointer. */
async function ptrFromStr(s: string): Promise<number> {
  const m = await getMod();
  const len = m.lengthBytesUTF8(s) + 1;
  const ptr = m._malloc(len);
  m.stringToUTF8(s, ptr, len);
  return ptr;
}

/** Read a C string from the WASM heap, then free it. */
async function strFromPtr(ptr: number): Promise<string> {
  const m = await getMod();
  const s = m.UTF8ToString(ptr);
  m._free(ptr);
  return s;
}

/** Call a function that returns a JSON string, parse and free it. */
async function callJson(fn: () => Promise<number>): Promise<any> {
  const ptr = await fn();
  const json = await strFromPtr(ptr);
  return JSON.parse(json);
}

/** Call a function that returns an int. */
async function callInt(fn: () => Promise<number>): Promise<number> {
  return fn();
}

export async function getStatus(): Promise<AnetdStatus> {
  const m = await getMod();
  return callJson(() => Promise.resolve(m._get_status()));
}

/**
 * Get debug status info — raw intermediate values from each shell call.
 * Useful for diagnosing why the status display is broken.
 */
export async function getStatusDebug(): Promise<Record<string, unknown>> {
  const m = await getMod();
  return callJson(() => Promise.resolve(m._get_status_debug()));
}

/**
 * Get raw log text (not JSON-parsed) for file export.
 */
export async function getLogsRaw(lines: number = 500): Promise<string> {
  const m = await getMod();
  const ptr = m._load_logs(lines);
  const json = await strFromPtr(ptr);
  // Re-parse to extract the raw lines as a text blob
  const arr: string[] = JSON.parse(json);
  return arr.join("\n");
}

export async function loadRules(): Promise<RuleEntry[]> {
  const m = await getMod();
  return callJson(() => Promise.resolve(m._load_rules()));
}

export async function reloadRules(): Promise<boolean> {
  const m = await getMod();
  return (await callInt(() => Promise.resolve(m._reload_rules()))) === 1;
}

export async function toggleFilter(): Promise<boolean> {
  const m = await getMod();
  return (await callInt(() => Promise.resolve(m._toggle_filter()))) === 1;
}

export async function loadConfig(): Promise<string> {
  const m = await getMod();
  return strFromPtr(m._load_config());
}

export async function saveConfig(content: string): Promise<boolean> {
  const m = await getMod();
  const ptr = await ptrFromStr(content);
  const ok = m._save_config(ptr) === 1;
  // Note: save_config calls reload_rules internally, which may free ptr.
  // We free it here just in case.
  m._free(ptr);
  return ok;
}

export async function loadLogs(lines: number = 100): Promise<string[]> {
  const m = await getMod();
  return callJson(() => Promise.resolve(m._load_logs(lines)));
}

export async function checkUrl(url: string, rulesJson: string = "[]"): Promise<boolean> {
  const m = await getMod();
  const urlPtr = await ptrFromStr(url);
  const rulesPtr = await ptrFromStr(rulesJson);
  const blocked = m._check_url(urlPtr, rulesPtr) === 1;
  m._free(urlPtr);
  m._free(rulesPtr);
  return blocked;
}
