<script lang="ts">
  import { getStatus, getStatusDebug, toggleFilter, reloadRules, type AnetdStatus } from "../api/anetd";
  import { ksu } from "../api/ksu";

  let status: AnetdStatus = $state({ running: false, pid: null, dnsFilterEnabled: true, uptime: "\u2014" });

  async function refresh() {
    status = await getStatus();
  }
  refresh();

  async function handleToggleFilter() {
    const ok = await toggleFilter();
    ksu.toast(ok ? "Filter toggled" : "Toggle failed");
    await refresh();
  }

  async function handleReloadRules() {
    try {
      const r = await reloadRules();
      ksu.toast(r.ok
        ? `Reloaded: ${r.rules_count} files (${r.block_rules}B/${r.allow_rules}A)`
        : "Reload failed");
    } catch (e: any) {
      ksu.toast("Reload failed: " + (e?.message || e));
    }
    await refresh();
  }

  async function handleRestart() {
    const result = ksu.exec(
      "kill -TERM $(cat /data/adb/modules/anetd/log/anetd.pid) 2>/dev/null; sleep 1; sh /data/adb/modules/anetd/post-fs-data.sh",
    );
    ksu.toast(result.errno === 0 ? "Restarted" : "Restart failed");
    setTimeout(() => refresh(), 1500);
  }

 async function handleExportDebug() {
  const debug = await getStatusDebug();
  const json = JSON.stringify(debug, null, 2);
  const path = `/sdcard/Download/anetd-debug-${Date.now()}.json`;
  ksu.exec(`echo '${json}' > ${path}`);
  ksu.toast(`Saved to ${path}`);
}
</script>

<h1 class="page-title">Dashboard</h1>
<p class="page-subtitle">Daemon status and quick controls</p>

<div class="card">
  <div class="card-header">
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
         stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/>
    </svg>
    Daemon Status
  </div>
  <div class="card-body">
    <div class="stat-row">
      <span class="stat-label">Status</span>
      <span class="stat-value">
        {#if status.running}
          <span class="status-dot running"></span>
          <span class="badge on">RUNNING</span>
        {:else}
          <span class="status-dot stopped"></span>
          <span class="badge off">STOPPED</span>
        {/if}
      </span>
    </div>
    <div class="stat-row">
      <span class="stat-label">PID</span>
      <span class="stat-value">{status.pid ?? "\u2014"}</span>
    </div>
    <div class="stat-row">
      <span class="stat-label">Uptime</span>
      <span class="stat-value">{status.uptime}</span>
    </div>
  </div>
</div>

<div class="card">
  <div class="card-header">
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
         stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
    </svg>
    DNS Filter
  </div>
  <div class="card-body">
    <div class="stat-row">
      <span class="stat-label">Adblock Filter</span>
      <span class="stat-value">
        {#if status.dnsFilterEnabled}
          <span class="badge on">ACTIVE</span>
        {:else}
          <span class="badge off">PAUSED</span>
        {/if}
      </span>
    </div>
  </div>
</div>

<div class="card">
  <div class="card-header">
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
         stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/>
    </svg>
    Quick Actions
  </div>
  <div class="card-body">
    <div class="actions">
      <button class="btn" onclick={handleToggleFilter}>Toggle Filter</button>
      <button class="btn" onclick={handleReloadRules}>Reload Rules</button>
      <button class="btn btn-danger" onclick={handleRestart}>Restart Daemon</button>
      <button class="btn btn-secondary" onclick={handleExportDebug}>Export Debug Info</button>
    </div>
  </div>
</div>
