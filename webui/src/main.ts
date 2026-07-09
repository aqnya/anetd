/**
 * anetd WebUI — Svelte 5 entry point.
 * Mounts the App component into #app.
 */
import { mount } from "svelte";
import App from "./App.svelte";

// DEBUG helpers
const dbg = (window as any).__DEBUG;
function dlog(cls: string, msg: string) {
  if (dbg) dbg.add(cls, msg);
  console.log("[anetd:debug]", cls, msg);
}

dlog("step", "=== main.ts module loaded ===");

try {
  const target = document.getElementById("app");
  if (!target) {
    dlog("err", "FATAL: #app element not found!");
  } else {
    dlog("info", "#app found, mounting App...");
    mount(App, { target });
    dlog("step", "=== mount(App) returned OK ===");
    // Hide debug banner if app mounted successfully
    setTimeout(() => {
      const banner = document.getElementById("debug-banner");
      if (banner) banner.style.display = "none";
      dlog("step", "=== app rendering complete (debug hidden) ===");
    }, 500);
  }
} catch (e: any) {
  dlog("err", "FATAL: mount() threw: " + (e?.message || e));
  if (e?.stack) dlog("err", e.stack);
  // Force debug banner to render
  const target = document.getElementById("app");
  if (target) {
    target.innerHTML =
      '<div id="debug-banner" style="position:fixed;inset:0;z-index:99999;background:#0f172a;color:#f1f5f9;font-family:monospace;font-size:12px;padding:16px;overflow-y:auto;white-space:pre-wrap;word-break:break-all;line-height:1.6;">' +
      '<span style="color:#ef4444">FATAL MOUNT ERROR:</span><br>' +
      (e?.message || String(e)) + "<br>" +
      (e?.stack || "") +
      "</div>";
  }
}
