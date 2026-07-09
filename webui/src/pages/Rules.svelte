<script lang="ts">
  import { loadRules, reloadRules } from "../api/anetd";
  import type { RuleEntry } from "../api/anetd";
  import { ksu } from "../api/ksu";

  let entries: RuleEntry[] = $state([]);

  const blockCount = $derived(entries.filter((e) => e.type === "block").length);
  const allowCount = $derived(entries.filter((e) => e.type === "allow").length);

  function refresh() {
    entries = loadRules();
  }
  refresh();

  function ruleLabel(t: RuleEntry["type"]): string {
    switch (t) {
      case "block": return "BLOCK";
      case "allow": return "ALLOW";
      case "comment": return "#";
      case "header": return "HDR";
      case "inline-comment": return "+#";
      case "blank": return "";
    }
  }

  function handleRefresh() {
    reloadRules();
    refresh();
    ksu.toast("Rules reloaded");
  }
</script>

<div class="page">
  <h2>Rules</h2>
  <p class="subtitle">{blockCount} block rules, {allowCount} allow rules loaded</p>
  <button class="btn" onclick={handleRefresh}>Refresh</button>

  <div class="rule-table-wrap">
    <table class="rule-table">
      <thead>
        <tr>
          <th></th>
          <th>Rule</th>
        </tr>
      </thead>
      <tbody>
        {#if entries.length === 0}
          <tr>
            <td colspan="2" class="empty">No rules loaded</td>
          </tr>
        {:else}
          {#each entries as entry}
            <tr class="rule-{entry.type}">
              <td class="rule-badge">
                {#if ruleLabel(entry.type)}
                  <span class="mini-badge rule-{entry.type}">{ruleLabel(entry.type)}</span>
                {/if}
              </td>
              <td class="rule-text">{entry.raw || " "}</td>
            </tr>
          {/each}
        {/if}
      </tbody>
    </table>
  </div>
</div>
