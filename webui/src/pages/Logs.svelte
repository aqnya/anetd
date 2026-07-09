<script lang="ts">
  import { loadLogs } from "../api/anetd";
  import { ksu } from "../api/ksu";

  let lines: string[] = $state([]);

  function refresh() {
    lines = loadLogs(200);
  }
  refresh();

  function handleClear() {
    ksu.exec(": > /data/adb/modules/anetd/log/anetd.log");
    refresh();
    ksu.toast("Logs cleared");
  }
</script>

<div class="page">
  <h2>Logs</h2>
  <p class="subtitle">Recent log entries from anetd daemon</p>
  <button class="btn" onclick={refresh}>Refresh</button>
  <button class="btn btn-secondary" style="margin-left:8px" onclick={handleClear}>
    Clear Logs
  </button>

  <div class="log-viewer">
    {#if lines.length === 0}
      <div class="empty">No log entries</div>
    {:else}
      {#each lines as line}
        <div class="log-line">{line}</div>
      {/each}
    {/if}
  </div>
</div>
