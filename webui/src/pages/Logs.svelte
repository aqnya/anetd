<script lang="ts">
  import { api, clearLogFile } from "../api/anetd";
  import Icon from "../lib/Icon.svelte";
  import Switch from "../lib/Switch.svelte";
  import { confirmDialog } from "../lib/confirm";
  import { t } from "../lib/i18n";
  import { appState } from "../lib/store";
  import { toast } from "../lib/toast";
  import { exportFile, formatNumber, MODULE } from "../lib/ui";

  let lines = $state<string[]>([]);
  let query = $state("");
  let limit = $state(200);
  let autoRefresh = $state(true);
  let loading = $state(false);
  let clearing = $state(false);
  let viewer: HTMLDivElement | undefined = $state();
  let stickToBottom = $state(true);

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return lines;
    return lines.filter((l) => l.toLowerCase().includes(q));
  });

  const logPath = $derived(appState.status?.log_path ?? `${MODULE}/log/anetd.log`);

  async function refresh(silent = false) {
    if (!silent) loading = true;
    try {
      const r = await api.loadLogs(limit);
      lines = r.lines ?? [];
    } catch {
      if (!silent) toast(t("toast.reload_failed"), "error");
    } finally {
      loading = false;
    }
  }

  function onScroll() {
    if (!viewer) return;
    const el = viewer;
    stickToBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
  }

  async function scrollToBottom() {
    await Promise.resolve();
    if (viewer) viewer.scrollTop = viewer.scrollHeight;
  }

  $effect(() => {
    void refresh();
  });

  $effect(() => {
    if (lines.length === 0 || !stickToBottom) return;
    if (viewer) viewer.scrollTop = viewer.scrollHeight;
  });

  $effect(() => {
    if (!autoRefresh) return;
    const id = window.setInterval(() => void refresh(true), 2000);
    return () => window.clearInterval(id);
  });

  async function handleClear() {
    const confirmed = await confirmDialog({
      title: t("confirm.clear_logs.title"),
      message: t("confirm.clear_logs.message"),
      confirmText: t("logs.clear"),
      danger: true,
    });
    if (!confirmed) return;
    clearing = true;
    const ok = await clearLogFile(logPath);
    clearing = false;
    if (ok) {
      lines = [];
      toast(t("toast.logs_cleared"), "success");
    } else {
      toast(t("toast.clear_failed"), "error");
    }
  }

  async function handleExport() {
    try {
      const name = `anetd-log-${Date.now()}.txt`;
      const ok = await exportFile(name, lines.join("\n"));
      if (ok) toast(t("toast.exported", { path: `/sdcard/Download/${name}` }), "success");
      else toast(t("toast.export_failed"), "error");
    } catch {
      toast(t("toast.export_failed"), "error");
    }
  }
</script>

<header class="page-header">
  <div>
    <h1 class="page-title">{t("logs.title")}</h1>
    <p class="page-subtitle">
      {t("logs.subtitle")} · {formatNumber(filtered.length)}/{formatNumber(lines.length)}
    </p>
  </div>
</header>

<div class="log-toolbar">
  <div class="search-box grow">
    <Icon name="search" size={15} />
    <input
      type="text"
      placeholder={t("logs.search")}
      bind:value={query}
      aria-label={t("logs.search")}
    />
  </div>
  <button class="btn" onclick={() => void refresh()} disabled={loading || !appState.connected}>
    {#if loading}<span class="spinner"></span>{:else}<Icon name="refresh" size={15} />{/if}
    {t("common.refresh")}
  </button>
  <button class="btn btn-danger" onclick={handleClear} disabled={clearing || !appState.connected}>
    {#if clearing}<span class="spinner"></span>{:else}<Icon name="trash" size={15} />{/if}
    {t("logs.clear")}
  </button>
  <button class="btn" onclick={handleExport} disabled={lines.length === 0}>
    <Icon name="download" size={15} />
    {t("logs.export")}
  </button>
</div>

<div class="log-options">
  <label class="option">
    <span class="option-label">{t("logs.auto_refresh")}</span>
    <Switch checked={autoRefresh} label={t("logs.auto_refresh")} />
  </label>
  <label class="option">
    <span class="option-label">{t("logs.lines")}</span>
    <select
      class="select"
      value={limit}
      onchange={(e) => {
        limit = Number((e.target as HTMLSelectElement).value);
      }}
    >
      <option value={100}>100</option>
      <option value={200}>200</option>
      <option value={500}>500</option>
      <option value={1000}>1000</option>
      <option value={2000}>2000</option>
    </select>
  </label>
</div>

<div class="log-viewer" bind:this={viewer} onscroll={onScroll}>
  {#if lines.length === 0}
    <div class="empty">{query.trim() ? t("logs.no_match") : t("logs.empty")}</div>
  {:else if filtered.length === 0}
    <div class="empty">{t("logs.no_match")}</div>
  {:else}
    {#each filtered as line, i (i)}
      <div class="log-line">{line}</div>
    {/each}
  {/if}
</div>
