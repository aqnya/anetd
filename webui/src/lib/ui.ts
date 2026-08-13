import { api, toast as ksuToast, writeFileBase64 } from "../api/anetd";
import { MODULE_DIR } from "../api/ksu";

export function formatNumber(n: number): string {
  return (n ?? 0).toLocaleString("en-US");
}

export function formatHash(hash: string, keep = 12): string {
  if (!hash) return "—";
  if (hash.length <= keep) return hash;
  return `${hash.slice(0, keep)}…`;
}

/** Copy text to the clipboard with a WebView-safe fallback. */
export async function copyText(text: string): Promise<boolean> {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch {
    /* fall through */
  }
  try {
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.focus();
    ta.select();
    const ok = document.execCommand("copy");
    document.body.removeChild(ta);
    return ok;
  } catch {
    return false;
  }
}

/** Patch known `key = value` lines in a TOML document, preserving comments. */
export function patchToml(content: string, values: Record<string, string | boolean | number>): string {
  let out = content;
  for (const [key, value] of Object.entries(values)) {
    const line = tomlLine(key, value);
    const re = new RegExp(`^[ \\t]*${escapeRegExp(key)}[ \\t]*=.*$`, "m");
    if (re.test(out)) {
      out = out.replace(re, line);
    } else {
      out = out.replace(/\n?\s*$/, "\n" + line + "\n");
    }
  }
  return out;
}

function escapeRegExp(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function tomlLine(key: string, value: string | boolean | number): string {
  const v =
    typeof value === "string"
      ? `"${value.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`
      : String(value);
  return `${key} = ${v}`;
}

/** Export a JSON/text file to /sdcard/Download via the root shell. */
export async function exportFile(filename: string, content: string): Promise<boolean> {
  const path = `/sdcard/Download/${filename}`;
  const ok = await writeFileBase64(path, content);
  if (ok) ksuToast(`Saved to ${path}`);
  return ok;
}

/** Export the daemon status as a debug JSON file. */
export async function exportDebugInfo(): Promise<void> {
  try {
    const status = await api.getStatus();
    const name = `anetd-debug-${Date.now()}.json`;
    await exportFile(name, JSON.stringify(status, null, 2));
  } catch (e) {
    ksuToast(`Export failed: ${e instanceof Error ? e.message : e}`);
  }
}

/** The module directory, re-exported for pages that need shell paths. */
export const MODULE = MODULE_DIR;
