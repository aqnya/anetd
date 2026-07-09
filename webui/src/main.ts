/**
 * anetd WebUI — Main entry point.
 *
 * Single-page app using hash-free routing via tab buttons.
 * Pages are rendered server-side style (innerHTML) from typed modules.
 */

import { renderDashboard } from "./ui/dashboard";
import { renderRules } from "./ui/rules";
import { renderSettings } from "./ui/settings";
import { renderLogs } from "./ui/logs";
import { toggleFilter, reloadRules, saveConfig, loadConfig } from "./api/anetd";
import { ksu } from "./api/ksu";

type PageName = "dashboard" | "rules" | "settings" | "logs";

const renderers: Record<PageName, () => string> = {
  dashboard: renderDashboard,
  rules: renderRules,
  settings: renderSettings,
  logs: renderLogs,
};

let currentPage: PageName = "dashboard";

const view = document.getElementById("view")!;
const tabs = document.getElementById("tabs")!;

/** Navigate to a page and re-render. */
function navigate(page: PageName): void {
  currentPage = page;
  view.innerHTML = renderers[page]();
  bindActions();
  updateTabActive(page);
}

/** Highlight the active tab button. */
function updateTabActive(page: PageName): void {
  tabs.querySelectorAll(".tab").forEach((el) => {
    const btn = el as HTMLButtonElement;
    btn.classList.toggle("active", btn.dataset.page === page);
  });
}

/** Attach event handlers to buttons in the current view. */
function bindActions(): void {
  // Dashboard buttons
  document.getElementById("btn-toggle-filter")?.addEventListener("click", () => {
    const ok = toggleFilter();
    ksu.toast(ok ? "Filter toggled" : "Toggle failed");
    navigate("dashboard");
  });

  document.getElementById("btn-reload-rules")?.addEventListener("click", () => {
    const ok = reloadRules();
    ksu.toast(ok ? "Rules reloaded" : "Reload failed");
    navigate("dashboard");
  });

  document.getElementById("btn-restart")?.addEventListener("click", () => {
    const result = ksu.exec("kill -TERM $(cat /data/adb/modules/anetd/log/anetd.pid) 2>/dev/null; sleep 1; sh /data/adb/modules/anetd/post-fs-data.sh");
    ksu.toast(result.errno === 0 ? "Restarted" : "Restart failed");
    setTimeout(() => navigate("dashboard"), 1500);
  });

  // Rules refresh
  document.getElementById("btn-reload-rules2")?.addEventListener("click", () => {
    reloadRules();
    navigate("rules");
  });

  // Settings
  document.getElementById("btn-save-config")?.addEventListener("click", () => {
    const textarea = document.getElementById("config-editor") as HTMLTextAreaElement;
    if (!textarea) return;
    const ok = saveConfig(textarea.value);
    ksu.toast(ok ? "Config saved & reloaded" : "Save failed");
  });

  document.getElementById("btn-reset-config")?.addEventListener("click", () => {
    const textarea = document.getElementById("config-editor") as HTMLTextAreaElement;
    if (textarea) textarea.value = loadConfig();
  });

  // Logs
  document.getElementById("btn-refresh-logs")?.addEventListener("click", () => {
    navigate("logs");
  });

  document.getElementById("btn-clear-logs")?.addEventListener("click", () => {
    ksu.exec(": > /data/adb/modules/anetd/log/anetd.log");
    navigate("logs");
  });
}

// Tab click handler
tabs.addEventListener("click", (e) => {
  const btn = (e.target as HTMLElement).closest<HTMLButtonElement>(".tab");
  if (!btn) return;
  const page = btn.dataset.page as PageName;
  if (page && page in renderers) {
    navigate(page);
  }
});

// Initial render
navigate("dashboard");
