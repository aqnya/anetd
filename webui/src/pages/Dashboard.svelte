<script lang="ts">
  import { api, restartDaemon, toggleFilter } from "../api/anetd";
  import Icon from "../lib/Icon.svelte";
  import Switch from "../lib/Switch.svelte";
  import { confirmDialog } from "../lib/confirm";
  import { t } from "../lib/i18n";
  import { appState, refreshStatus } from "../lib/store";
  import { toast } from "../lib/toast";
  import { exportDebugInfo, formatNumber } from "../lib/ui";

  const busy = $state<Record<string, boolean>>({});

  async function run(key: string, fn: () => Promise<void>): Promise<void> {
    if (busy[key]) return;
    busy[key] = true;
    try {
      await fn();
    } finally {
      busy[key] = false;
    }
  }

  async function handleToggleFilter() {
    await run("filter", async () => {
      const wasEnabled = appState.status?.dns_filter_enabled ?? true;
      const ok = await toggleFilter();
      await refreshStatus();
      toast(
        ok
          ? wasEnabled
            ? t("toast.filter_off")
            : t("toast.filter_on")
          : t("toast.filter_failed"),
        ok ? "success" : "error",
      );
    });
  }

  async function handleReloadRules() {
    await run("reload", async () => {
      try {
        const r = await api.reloadRules();
        await refreshStatus();
        toast(
          r.ok
            ? `${t("toast.reloaded")}（${r.rules_count} 文件 / ${r.block_rules} 阻断 / ${r.allow_rules} 放行）`
            : t("toast.reload_failed"),
          r.ok ? "success" : "error",
        );
      } catch (e) {
        toast(t("toast.reload_failed"), "error");
        console.error(e);
      }
    });
  }

  async function handleRestart() {
    const confirmed = await confirmDialog({
      title: t("confirm.restart.title"),
      message: t("confirm.restart.message"),
      confirmText: t("dash.restart"),
      danger: true,
    });
    if (!confirmed) return;

    await run("restart", async () => {
      const ok = await restartDaemon();
      toast(ok ? t("toast.restarted") : t("toast.restart_failed"), ok ? "success" : "error");
      // The daemon briefly goes away during restart; wait for it to come back.
      for (let i = 0; i < 10; i++) {
        await new Promise((r) => setTimeout(r, 1500));
        if (await refreshStatus()) return;
      }
    });
  }

  const s = $derived(appState.status);
  const running = $derived(s?.running ?? false);
  const stats = $derived([
    { label: t("dash.dns_queries"), value: formatNumber(s?.dns_queries ?? 0), icon: "activity" },
    { label: t("dash.blocked"), value: formatNumber(s?.blocked ?? 0), icon: "shield" },
    { label: t("dash.rules_files"), value: formatNumber(s?.rules_count ?? 0), icon: "file" },
    { label: t("dash.rules_entries"), value: formatNumber((s?.block_rules ?? 0) + (s?.allow_rules ?? 0)), icon: "zap" },
  ]);
</script>

<header class="page-header">
  <div>
    <h1 class="page-title">{t("dash.title")}</h1>
    <p class="page-subtitle">{t("dash.subtitle")}</p>
  </div>
  <div class="live-indicator" class:live={appState.connected} title={t("dash.auto_refresh")}>
    <span class="live-dot"></span>
    <span>{appState.connected ? t("banner.connected") : t("common.loading")}</span>
  </div>
</header>

<!-- Status hero -->
<section class="card hero-card">
  <div class="hero-main">
    <div class="hero-icon" class:down={!running}>
      <Icon name="shield" size={26} />
    </div>
    <div>
      <div class="hero-status">
        {#if appState.connected}
          <span class="badge on"><span class="status-dot"></span>{t(running ? "dash.running" : "dash.stopped")}</span>
        {:else}
          <span class="badge off"><span class="status-dot"></span>{t("dash.stopped")}</span>
        {/if}
      </div>
      <div class="hero-meta">
        {#if s}
          <span>{t("dash.version")} {s.version}</span>
          <span class="meta-sep">·</span>
          <span>{t("dash.mode")}: {t(s.mode === "dns-server" ? "dash.mode.dns-server" : "dash.mode.hijack")}</span>
        {/if}
      </div>
    </div>
  </div>
  <div class="hero-stats">
    <div class="hero-stat">
      <span class="hero-stat-label">{t("dash.uptime")}</span>
      <span class="hero-stat-value">{s?.uptime ?? "—"}</span>
    </div>
    <div class="hero-stat">
      <span class="hero-stat-label">{t("dash.pid")}</span>
      <span class="hero-stat-value">{s?.pid ?? "—"}</span>
    </div>
    <div class="hero-stat">
      <span class="hero-stat-label">{t("dash.multi_thread")}</span>
      <span class="hero-stat-value">{s ? (s.multi_thread ? t("common.on") : t("common.off")) : "—"}</span>
    </div>
    <div class="hero-stat">
      <span class="hero-stat-label">{t("dash.battery_saver")}</span>
      <span class="hero-stat-value">{s ? (s.battery_saver ? t("common.on") : t("common.off")) : "—"}</span>
    </div>
  </div>
</section>

<!-- Stats -->
<section class="stat-grid">
  {#each stats as stat (stat.label)}
    <div class="card stat-card">
      <Icon name={stat.icon} size={18} />
      <span class="stat-card-value">{stat.value}</span>
      <span class="stat-card-label">{stat.label}</span>
    </div>
  {/each}
</section>

<!-- Filter -->
<section class="card">
  <div class="card-header">
    <Icon name="toggle" size={14} />
    {t("dash.filter")}
    <span class="card-header-right">
      <span class="badge {appState.connected && (s?.dns_filter_enabled ?? false) ? "on" : "off"}">
        {appState.connected && (s?.dns_filter_enabled ?? false) ? t("dash.filter.active") : t("dash.filter.paused")}
      </span>
    </span>
  </div>
  <div class="card-body row-between">
    <div>
      <div class="row-title">
        {t("dash.filter")}
        <span class="chip">{t("dash.block_rules")} {formatNumber(s?.block_rules ?? 0)} / {t("dash.allow_rules")} {formatNumber(s?.allow_rules ?? 0)}</span>
      </div>
      {#if appState.connected && (s?.rules_count ?? 0) === 0}
        <p class="field-hint warn-text">
          <Icon name="alert" size={12} />
          {t("dash.filter.no_rules")}
        </p>
      {/if}
    </div>
    <Switch
      checked={appState.connected && (s?.dns_filter_enabled ?? false)}
      label={t("dash.filter")}
      disabled={!appState.connected || busy["filter"]}
      onChange={handleToggleFilter}
    />
  </div>
</section>

<!-- Actions -->
<section class="card">
  <div class="card-header">
    <Icon name="zap" size={14} />
    {t("dash.actions")}
  </div>
  <div class="card-body">
    <div class="action-list">
      <button class="action-row" onclick={handleReloadRules} disabled={!appState.connected || busy["reload"]}>
        <span class="action-icon primary-icon"><Icon name="refresh" size={16} /></span>
        <span class="action-text">
          <span class="action-title">{t("dash.reload_rules")}</span>
          <span class="action-desc">{t("rules.subtitle", { count: s?.rules_count ?? 0 })}</span>
        </span>
        {#if busy["reload"]}<span class="spinner"></span>{/if}
      </button>
      <button class="action-row" onclick={() => void exportDebugInfo()} disabled={!appState.connected}>
        <span class="action-icon"><Icon name="download" size={16} /></span>
        <span class="action-text">
          <span class="action-title">{t("dash.export_debug")}</span>
          <span class="action-desc">JSON → /sdcard/Download</span>
        </span>
      </button>
      <button class="action-row danger-row" onclick={handleRestart} disabled={busy["restart"]}>
        <span class="action-icon danger-icon"><Icon name="power" size={16} /></span>
        <span class="action-text">
          <span class="action-title">{t("dash.restart")}</span>
          <span class="action-desc">{t("confirm.restart.message")}</span>
        </span>
        {#if busy["restart"]}<span class="spinner"></span>{/if}
      </button>
    </div>
  </div>
</section>
