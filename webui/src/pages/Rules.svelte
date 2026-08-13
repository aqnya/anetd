<script lang="ts">
  import { api, type RuleFile } from "../api/anetd";
  import Icon from "../lib/Icon.svelte";
  import { t } from "../lib/i18n";
  import { appState } from "../lib/store";
  import { toast } from "../lib/toast";
  import { copyText, formatHash, formatNumber } from "../lib/ui";

  let files = $state<RuleFile[]>([]);
  let query = $state("");
  let loading = $state(true);
  let reloading = $state(false);

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return files;
    return files.filter((f) => f.path.toLowerCase().includes(q));
  });

  async function refresh(silent = false) {
    if (!silent) loading = true;
    try {
      const r = await api.loadRules();
      files = r.files ?? [];
    } catch {
      toast(t("toast.reload_failed"), "error");
    } finally {
      loading = false;
    }
  }

  async function handleReload() {
    reloading = true;
    try {
      const r = await api.reloadRules();
      await refresh(true);
      toast(
        r.ok
          ? `${t("toast.reloaded")}（${r.rules_count} 文件 / ${r.block_rules} 阻断 / ${r.allow_rules} 放行）`
          : t("toast.reload_failed"),
        r.ok ? "success" : "error",
      );
    } catch {
      toast(t("toast.reload_failed"), "error");
    } finally {
      reloading = false;
    }
  }

  async function handleCopy(text: string) {
    toast((await copyText(text)) ? t("common.copied") : t("common.copy"), "info", 1600);
  }

  $effect(() => {
    void refresh(true);
  });

  const s = $derived(appState.status);
</script>

<header class="page-header">
  <div>
    <h1 class="page-title">{t("rules.title")}</h1>
    <p class="page-subtitle">{t("rules.subtitle", { count: formatNumber(files.length) })}</p>
  </div>
</header>

<div class="summary-chips">
  <div class="chip"><Icon name="file" size={13} />{t("dash.rules_files")} {formatNumber(files.length)}</div>
  <div class="chip chip-red"><Icon name="shield" size={13} />{t("dash.block_rules")} {formatNumber(s?.block_rules ?? 0)}</div>
  <div class="chip chip-green"><Icon name="check" size={13} />{t("dash.allow_rules")} {formatNumber(s?.allow_rules ?? 0)}</div>
</div>

<div class="toolbar">
  <div class="search-box">
    <Icon name="search" size={15} />
    <input
      type="text"
      placeholder={t("rules.search")}
      bind:value={query}
      aria-label={t("rules.search")}
    />
  </div>
  <button class="btn btn-primary" onclick={handleReload} disabled={reloading || !appState.connected}>
    {#if reloading}<span class="spinner"></span>{:else}<Icon name="refresh" size={15} />{/if}
    {t("rules.reload")}
  </button>
</div>

<div class="rule-table-wrap">
  <table class="rule-table">
    <thead>
      <tr>
        <th>{t("rules.file")}</th>
        <th class="hash-col">{t("rules.hash")}</th>
        <th class="copy-col"></th>
      </tr>
    </thead>
    <tbody>
      {#if loading}
        <tr><td colspan="3" class="empty"><span class="spinner"></span> {t("common.loading")}</td></tr>
      {:else if filtered.length === 0}
        <tr>
          <td colspan="3" class="empty">
            {query.trim() ? t("rules.no_match") : t("rules.empty")}
          </td>
        </tr>
      {:else}
        {#each filtered as entry (entry.path)}
          <tr>
            <td class="rule-path" title={entry.path}>{entry.path}</td>
            <td class="rule-hash" title={entry.hash}>
              <code>{formatHash(entry.hash)}</code>
            </td>
            <td class="copy-col">
              <button
                class="mini-btn"
                onclick={() => void handleCopy(entry.path)}
                aria-label={t("common.copy")}
                title={t("common.copy")}
              >
                <Icon name="copy" size={13} />
              </button>
            </td>
          </tr>
        {/each}
      {/if}
    </tbody>
  </table>
</div>
