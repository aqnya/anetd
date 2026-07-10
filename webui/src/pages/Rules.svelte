<script lang="ts">
  import { loadRules, reloadRules, type RuleFile } from "../api/anetd";
  import { ksu } from "../api/ksu";

  let entries: RuleFile[] = $state([]);

  const fileCount = $derived(entries.length);

  async function refresh() {
    entries = await loadRules();
  }
  refresh();

  async function handleReload() {
    try {
      const r = await reloadRules();
      await refresh();
      ksu.toast(r.ok ? `Reloaded: ${r.rules_count} files` : "Reload failed");
    } catch (e: any) {
      ksu.toast("Reload failed: " + (e?.message || e));
    }
  }
</script>

<h1 class="page-title">Rules</h1>
<p class="page-subtitle">{fileCount} rule files loaded</p>

<div class="actions">
  <button class="btn btn-primary" onclick={handleReload}>Reload Rules</button>
  <button class="btn" onclick={refresh}>Refresh</button>
</div>

<div class="rule-table-wrap" style="margin-top:16px">
  <table class="rule-table">
    <thead>
      <tr>
        <th>Path</th>
        <th>SHA-256</th>
      </tr>
    </thead>
    <tbody>
      {#if entries.length === 0}
        <tr>
          <td colspan="2" class="empty">No rule files loaded</td>
        </tr>
      {:else}
        {#each entries as entry}
          <tr>
            <td class="rule-text" style="font-family:var(--mono);font-size:var(--font-size-xs)">{entry.path}</td>
            <td style="font-family:var(--mono);font-size:0.7rem;color:var(--text-dim)">{entry.hash}</td>
          </tr>
        {/each}
      {/if}
    </tbody>
  </table>
</div>
