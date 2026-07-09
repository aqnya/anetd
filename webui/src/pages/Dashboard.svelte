<script lang="ts">
  import { getStatus, toggleFilter, reloadRules } from "../api/anetd";
  import { ksu } from "../api/ksu";
  import type { AnetdStatus } from "../api/anetd";

  let status: AnetdStatus = $state(getStatus());

  function refresh() {
    status = getStatus();
  }

  function handleToggleFilter() {
    const ok = toggleFilter();
    ksu.toast(ok ? "Filter toggled" : "Toggle failed");
    refresh();
  }

  function handleReloadRules() {
    const ok = reloadRules();
    ksu.toast(ok ? "Rules reloaded" : "Reload failed");
    refresh();
  }

  function handleRestart() {
    const result = ksu.exec(
      "kill -TERM $(cat /data/adb/modules/anetd/log/anetd.pid) 2>/dev/null; sleep 1; sh /data/adb/modules/anetd/post-fs-data.sh",
    );
    ksu.toast(result.errno === 0 ? "Restarted" : "Restart failed");
    setTimeout(refresh, 1500);
  }
</script>

<div class="page">
  <h2>Dashboard</h2>

  <div class="card">
    <div class="card-title">Daemon Status</div>
    <div class="card-body">
      <div class="stat-row">
        <span class="label">Status</span>
        {#if status.running}
          <span class="badge on">RUNNING</span>
        {:else}
          <span class="badge off">STOPPED</span>
        {/if}
      </div>
      <div class="stat-row">
        <span class="label">PID</span>
        <span>{status.pid ?? "—"}</span>
      </div>
      <div class="stat-row">
        <span class="label">Uptime</span>
        <span>{status.uptime}</span>
      </div>
    </div>
  </div>

  <div class="card">
    <div class="card-title">DNS Filter</div>
    <div class="card-body">
      <div class="stat-row">
        <span class="label">Adblock Filter</span>
        {#if status.dnsFilterEnabled}
          <span class="badge on">ACTIVE</span>
        {:else}
          <span class="badge off">PAUSED</span>
        {/if}
      </div>
    </div>
  </div>

  <div class="card">
    <div class="card-title">Quick Actions</div>
    <div class="card-body actions">
      <button class="btn" onclick={handleToggleFilter}>Toggle Filter</button>
      <button class="btn" onclick={handleReloadRules}>Reload Rules</button>
      <button class="btn btn-danger" onclick={handleRestart}>Restart Daemon</button>
    </div>
  </div>
</div>
