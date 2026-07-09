<script lang="ts">
  import { loadConfig, saveConfig } from "../api/anetd";
  import { ksu } from "../api/ksu";

  let config: string = $state(loadConfig());
  let dirty: boolean = $state(false);

  function handleSave() {
    const ok = saveConfig(config);
    ksu.toast(ok ? "Config saved & reloaded" : "Save failed");
    if (ok) dirty = false;
  }

  function handleReset() {
    config = loadConfig();
    dirty = false;
  }

  function onInput(e: Event) {
    dirty = true;
    config = (e.target as HTMLTextAreaElement).value;
  }
</script>

<div class="page">
  <h2>Settings</h2>
  <p class="subtitle">
    Edit <code>/data/adb/anetd/config.toml</code>
  </p>

  <div class="card">
    <div class="card-title">Configuration</div>
    <div class="card-body">
      <textarea
        id="config-editor"
        rows="12"
        spellcheck="false"
        value={config}
        oninput={onInput}
      ></textarea>
      <div class="actions" style="margin-top:12px">
        <button class="btn" onclick={handleSave}>Save &amp; Reload</button>
        <button class="btn btn-secondary" onclick={handleReset}>Reset</button>
        {#if dirty}
          <span style="color: var(--orange); font-size: 0.85rem; align-self: center;">Unsaved changes</span>
        {/if}
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
