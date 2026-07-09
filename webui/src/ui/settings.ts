import { loadConfig, saveConfig } from "../api/anetd";

export function renderSettings(): string {
  const raw = loadConfig();

  return `
    <div class="page" id="page-settings">
      <h2>Settings</h2>
      <p class="subtitle">Edit <code>/data/adb/anetd/config.toml</code></p>

      <div class="card">
        <div class="card-title">Configuration</div>
        <div class="card-body">
          <textarea id="config-editor" rows="12" spellcheck="false">${escHtml(raw)}</textarea>
          <div class="actions" style="margin-top:12px">
            <button class="btn" id="btn-save-config">Save & Reload</button>
            <button class="btn btn-secondary" id="btn-reset-config">Reset</button>
          </div>
        </div>
      </div>

      <div class="card">
        <div class="card-title">Config Reference</div>
        <div class="card-body">
          <pre class="ref-block"># anetd config.toml
rules = "/data/adb/anetd/rules"
standalone = false      # daemon mode
multi_thread = true     # tokio multi-thread
dns_server = false      # built-in DNS server
dns_port = 53
dns_upstream = "8.8.8.8:53"
battery_saver = false</pre>
        </div>
      </div>
    </div>
  `;
}

function escHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}
