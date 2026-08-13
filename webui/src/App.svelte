<script lang="ts">
  import ConfirmDialog from "./lib/ConfirmDialog.svelte";
  import Icon from "./lib/Icon.svelte";
  import { dismiss, toasts } from "./lib/toast";
  import { t } from "./lib/i18n";
  import { startDaemon } from "./api/anetd";
  import { appState, refreshStatus, startPolling } from "./lib/store";
  import { toast } from "./lib/toast";
  import Dashboard from "./pages/Dashboard.svelte";
  import Logs from "./pages/Logs.svelte";
  import Rules from "./pages/Rules.svelte";
  import Settings from "./pages/Settings.svelte";

  type Page = "dashboard" | "rules" | "settings" | "logs";
  const PAGES: Page[] = ["dashboard", "rules", "settings", "logs"];

  const navItems: { id: Page; label: string; icon: string }[] = [
    { id: "dashboard", label: "nav.dashboard", icon: "dashboard" },
    { id: "rules", label: "nav.rules", icon: "shield" },
    { id: "settings", label: "nav.settings", icon: "settings" },
    { id: "logs", label: "nav.logs", icon: "logs" },
  ];

  function pageFromHash(): Page {
    const h = window.location.hash.replace(/^#\/?/, "");
    return (PAGES as string[]).includes(h) ? (h as Page) : "dashboard";
  }

  let current = $state<Page>(pageFromHash());
  let theme = $state(getInitialTheme());

  function getInitialTheme(): "dark" | "light" {
    try {
      const stored = localStorage.getItem("anetd-theme");
      if (stored === "dark" || stored === "light") return stored;
    } catch {
      /* ignore */
    }
    return window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
  }

  function navigate(page: Page) {
    current = page;
    window.location.hash = `/${page}`;
  }

  function toggleTheme() {
    theme = theme === "dark" ? "light" : "dark";
    document.documentElement.setAttribute("data-theme", theme);
    try {
      localStorage.setItem("anetd-theme", theme);
    } catch {
      /* ignore */
    }
  }

  async function handleStart() {
    const ok = await startDaemon();
    toast(ok ? t("toast.restarted") : t("toast.restart_failed"), ok ? "success" : "error");
    for (let i = 0; i < 10; i++) {
      await new Promise((r) => setTimeout(r, 1500));
      if (await refreshStatus()) return;
    }
  }

  $effect(() => {
    document.documentElement.setAttribute("data-theme", theme);
  });

  $effect(() => {
    startPolling();
    const onHash = () => {
      current = pageFromHash();
    };
    window.addEventListener("hashchange", onHash);
    return () => window.removeEventListener("hashchange", onHash);
  });
</script>

<div class="app-shell">
  <header class="topbar">
    <div class="topbar-brand" role="banner">
      <Icon name="globe" size={22} />
      <span>Anetd</span>
    </div>

    <nav class="topbar-nav" aria-label="Main navigation">
      {#each navItems as item (item.id)}
        <button
          class="tab-item"
          class:active={current === item.id}
          onclick={() => navigate(item.id)}
          aria-current={current === item.id ? "page" : undefined}
        >
          <Icon name={item.icon} size={17} />
          <span class="tab-label">{t(item.label)}</span>
        </button>
      {/each}
    </nav>

    <div class="topbar-actions">
      <button
        class="icon-btn"
        onclick={() => void refreshStatus()}
        aria-label={t("common.refresh")}
        title={t("common.refresh")}
      >
        <Icon name="refresh" size={17} />
      </button>
      <button
        class="icon-btn"
        onclick={toggleTheme}
        aria-label={theme === "dark" ? "Switch to light mode" : "Switch to dark mode"}
      >
        <Icon name={theme === "dark" ? "sun" : "moon"} size={17} />
      </button>
    </div>
  </header>

  {#if !appState.connected}
    <div class="conn-banner" class:checking={appState.checking}>
      <Icon name={appState.checking ? "refresh" : "alert"} size={15} />
      <span class="conn-text">
        {appState.checking ? t("banner.checking") : t("banner.offline")}
      </span>
      {#if !appState.checking}
        <button class="conn-retry" onclick={() => void handleStart()}>
          {t("banner.start")}
        </button>
      {/if}
    </div>
  {/if}

  <main class="main-content">
    {#if current === "dashboard"}
      <Dashboard />
    {:else if current === "rules"}
      <Rules />
    {:else if current === "settings"}
      <Settings />
    {:else if current === "logs"}
      <Logs />
    {/if}
  </main>

  <div class="toast-container" aria-live="polite">
    {#each toasts as item (item.id)}
      <div class="toast toast-{item.type}">
        <Icon
          name={item.type === "error" ? "alert" : item.type === "success" ? "check" : "info"}
          size={14}
        />
        <span class="toast-msg">{item.msg}</span>
        <button class="toast-close" onclick={() => dismiss(item.id)} aria-label={t("common.close")}>
          <Icon name="x" size={12} />
        </button>
      </div>
    {/each}
  </div>

  <ConfirmDialog />
</div>
