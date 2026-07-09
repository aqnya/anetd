<script lang="ts">
  import Dashboard from "./pages/Dashboard.svelte";
  import Rules from "./pages/Rules.svelte";
  import Settings from "./pages/Settings.svelte";
  import Logs from "./pages/Logs.svelte";
  import ThemeToggle from "./lib/ThemeToggle.svelte";

  // ── DEBUG ──
  const dbg = (window as any).__DEBUG;
  if (dbg) dbg.add("step", "=== App.svelte module loaded ===");

  type Page = "dashboard" | "rules" | "settings" | "logs";

  interface NavItem {
    id: Page;
    label: string;
    icon: "dashboard" | "shield" | "settings" | "logs";
  }

  const navItems: NavItem[] = [
    { id: "dashboard", label: "Dashboard", icon: "dashboard" },
    { id: "rules", label: "Rules", icon: "shield" },
    { id: "settings", label: "Settings", icon: "settings" },
    { id: "logs", label: "Logs", icon: "logs" },
  ];

  let current = $state<Page>("dashboard");
  let theme = $state("dark");
  let sidebarOpen = $state(false);

  // Initialize theme from DOM (set by inline script in index.html)
  $effect(() => {
    theme = document.documentElement.getAttribute("data-theme") ?? "dark";
    if (dbg) dbg.add("info", "App $effect: theme=" + theme);
  });

  // DEBUG: log when App mounts
  if (dbg) dbg.add("step", "=== App.svelte component initialized ===");

  function navigate(page: Page) {
    current = page;
    sidebarOpen = false;
  }

  function handleKeyNav(e: KeyboardEvent, page: Page) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      navigate(page);
    }
  }
</script>

<svelte:window onkeydown={(e) => { if (e.key === 'Escape') sidebarOpen = false; }} />

<div class="app-shell">
  <!-- Mobile backdrop -->
  <div
    class="sidebar-backdrop"
    class:visible={sidebarOpen}
    onclick={() => (sidebarOpen = false)}
    role="presentation"
  ></div>

  <!-- Mobile nav toggle -->
  <button
    class="mobile-nav-toggle"
    onclick={() => (sidebarOpen = !sidebarOpen)}
    aria-label="Toggle navigation"
    aria-expanded={sidebarOpen}
  >
    {#if sidebarOpen}
      <!-- X icon -->
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor"
           stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <line x1="18" y1="6" x2="6" y2="18"/>
        <line x1="6" y1="6" x2="18" y2="18"/>
      </svg>
    {:else}
      <!-- Hamburger icon -->
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor"
           stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <line x1="3" y1="6" x2="21" y2="6"/>
        <line x1="3" y1="12" x2="21" y2="12"/>
        <line x1="3" y1="18" x2="21" y2="18"/>
      </svg>
    {/if}
  </button>

  <!-- Sidebar -->
  <aside class="sidebar" class:open={sidebarOpen}>
    <div class="sidebar-brand">
      <!-- Globe/network icon -->
      <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor"
           stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="12" cy="12" r="10"/>
        <line x1="2" y1="12" x2="22" y2="12"/>
        <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>
      </svg>
      Anetd
    </div>

    <nav class="sidebar-nav" aria-label="Main navigation">
      {#each navItems as item}
        <button
          class="nav-item"
          class:active={current === item.id}
          onclick={() => navigate(item.id)}
          onkeydown={(e) => handleKeyNav(e, item.id)}
          aria-current={current === item.id ? "page" : undefined}
        >
          {#if item.icon === "dashboard"}
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                 stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <rect x="3" y="3" width="7" height="7" rx="1"/>
              <rect x="14" y="3" width="7" height="7" rx="1"/>
              <rect x="14" y="14" width="7" height="7" rx="1"/>
              <rect x="3" y="14" width="7" height="7" rx="1"/>
            </svg>
          {:else if item.icon === "shield"}
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                 stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
            </svg>
          {:else if item.icon === "settings"}
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                 stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <circle cx="12" cy="12" r="3"/>
              <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
            </svg>
          {:else if item.icon === "logs"}
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                 stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
              <polyline points="14 2 14 8 20 8"/>
              <line x1="16" y1="13" x2="8" y2="13"/>
              <line x1="16" y1="17" x2="8" y2="17"/>
              <polyline points="10 9 9 9 8 9"/>
            </svg>
          {/if}
          {item.label}
        </button>
      {/each}
    </nav>

    <div class="sidebar-footer">
      <ThemeToggle bind:theme />
    </div>
  </aside>

  <!-- Main content -->
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
</div>

<style>
  /* Scoped component styles — layout is in style.css */
</style>
