<script lang="ts">
  import { loadLogs, getLogsRaw } from "../api/anetd";
  import { ksu } from "../api/ksu";

  let lines: string[] = $state([]);

  async function refresh() {
    lines = await loadLogs(200);
  }
  refresh();

  function handleClear() {
    ksu.exec(": > /data/adb/modules/anetd/log/anetd.log");
    refresh();
    ksu.toast("Logs cleared");
  }

  async function handleExport() {
    try {
      const text = await getLogsRaw(2000);
      const blob = new Blob([text], { type: "text/plain" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `anetd-log-${Date.now()}.txt`;
      a.click();
      URL.revokeObjectURL(url);
      ksu.toast("Log file exported");
    } catch (e: any) {
      ksu.toast("Export failed: " + (e?.message || e));
    }
  }
</script>

<h1 class="page-title">Logs</h1>
<p class="page-subtitle">Recent log entries from anetd daemon</p>

<div class="log-toolbar">
  <button class="btn" onclick={refresh}>Refresh</button>
  <button class="btn btn-secondary" onclick={handleClear}>Clear Logs</button>
  <button class="btn btn-secondary" onclick={handleExport}>Export Log File</button>
</div>

<div class="log-viewer">
  {#if lines.length === 0}
    <div class="empty">No log entries</div>
  {:else}
    {#each lines as line}
      <div class="log-line">{line}</div>
    {/each}
  {/if}
</div>
