/**
 * anetd WebUI — Svelte 5 entry point.
 */
import { mount } from "svelte";
import App from "./App.svelte";

function fatalOverlay(message: string, stack?: string): void {
  const target = document.getElementById("app");
  if (!target) return;
  target.innerHTML =
    '<div style="position:fixed;inset:0;z-index:99999;background:#0b1120;color:#f1f5f9;' +
    'font-family:monospace;font-size:12px;padding:16px;overflow-y:auto;white-space:pre-wrap;' +
    'word-break:break-all;line-height:1.6;">' +
    '<span style="color:#ef4444">FATAL MOUNT ERROR:</span><br>' +
    String(message) +
    (stack ? "<br>" + String(stack) : "") +
    "</div>";
}

try {
  const target = document.getElementById("app");
  if (!target) {
    throw new Error("#app element not found");
  }
  mount(App, { target });
} catch (e) {
  console.error("[anetd] mount failed:", e);
  fatalOverlay(e instanceof Error ? e.message : String(e), e instanceof Error ? e.stack : "");
}
