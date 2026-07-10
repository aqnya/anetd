<script lang="ts">
  import { loadConfig, saveConfig } from "../api/anetd";
  import { ksu } from "../api/ksu";

  let config: string = $state("");
  let dirty: boolean = $state(false);

  async function initConfig() {
    config = await loadConfig();
  }
  initConfig();

  async function handleSave() {
    const ok = await saveConfig(config);
    ksu.toast(ok ? "Config saved & reloaded" : "Save failed");
    if (ok) dirty = false;
  }

  async function handleReset() {
    config = await loadConfig();
    dirty = false;
  }

  function onInput(e: Event) {
    dirty = true;
    config = (e.target as HTMLTextAreaElement).value;
  }
</script>

<h1 class="page-title">Settings</h1>
<p class="page-subtitle">
  Edit <code>/data/adb/modules/anetd/config.toml</code>
</p>

<div class="card">
  <div class="card-header">
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
         stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
      <polyline points="14 2 14 8 20 8"/>
      <line x1="16" y1="13" x2="8" y2="13"/>
      <line x1="16" y1="17" x2="8" y2="17"/>
    </svg>
    Configuration
  </div>
  <div class="card-body">
    <textarea
      id="config-editor"
      rows="14"
      spellcheck="false"
      value={config}
      oninput={onInput}
    ></textarea>
    <div class="actions">
      <button class="btn btn-primary" onclick={handleSave}>Save &amp; Reload</button>
      <button class="btn btn-secondary" onclick={handleReset}>Reset</button>
      {#if dirty}
        <span class="unsaved">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor">
            <circle cx="12" cy="12" r="8"/>
          </svg>
          Unsaved changes
        </span>
      {/if}
    </div>
  </div>
</div>

<div class="card">
  <div class="card-header">
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
         stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="12" cy="12" r="10"/>
      <line x1="12" y1="16" x2="12" y2="12"/>
      <line x1="12" y1="8" x2="12.01" y2="8"/>
    </svg>
    Config Reference
  </div>
  <div class="card-body">
    <pre class="ref-block"># anetd config.toml
rules = "/data/adb/modules/anetd/rules"
standalone = false      # daemon mode
multi_thread = true     # tokio multi-thread
dns_server = false      # built-in DNS server
dns_port = 53
dns_upstream = "8.8.8.8:53"
battery_saver = false
# Unix socket for KSU WebUI
webui_socket = "/data/adb/modules/anetd/webui.sock"</pre>
  </div>
</div>
