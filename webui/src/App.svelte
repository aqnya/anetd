<script lang="ts">
  import Dashboard from "./pages/Dashboard.svelte";
  import Rules from "./pages/Rules.svelte";
  import Settings from "./pages/Settings.svelte";
  import Logs from "./pages/Logs.svelte";

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

  // Init theme from DOM
  $effect(() => {
    theme = document.documentElement.getAttribute("data-theme") ?? "dark";
  });

  function navigate(page: Page) {
    current = page;
  }

  function handleKeyNav(e: KeyboardEvent, page: Page) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      navigate(page);
    }
  }

  function toggleTheme() {
    const next = theme === "dark" ? "light" : "dark";
    theme = next;
    document.documentElement.setAttribute("data-theme", next);
    try { localStorage.setItem("anetd-theme", next); } catch {}
  }
</script>

<div class="app-shell">
  <!-- Top Navigation Bar -->
  <header class="topbar">
    <!-- Brand -->
    <div class="topbar-brand" role="banner">
      <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor"
           stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="12" cy="12" r="10"/>
        <line x1="2" y1="12" x2="22" y2="12"/>
        <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>
      </svg>
      Anetd
    </div>

    <!-- Navigation Tabs -->
    <nav class="topbar-nav" aria-label="Main navigation">
      {#each navItems as item}
        <button
          class="tab-item"
          class:active={current === item.id}
          onclick={() => navigate(item.id)}
          onkeydown={(e) => handleKeyNav(e, item.id)}
          aria-current={current === item.id ? "page" : undefined}
        >
          {#if item.icon === "dashboard"}
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                 stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <rect x="3" y="3" width="7" height="7" rx="1"/>
              <rect x="14" y="3" width="7" height="7" rx="1"/>
              <rect x="14" y="14" width="7" height="7" rx="1"/>
              <rect x="3" y="14" width="7" height="7" rx="1"/>
            </svg>
          {:else if item.icon === "shield"}
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                 stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
            </svg>
          {:else if item.icon === "settings"}
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                 stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <circle cx="12" cy="12" r="3"/>
              <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
            </svg>
          {:else if item.icon === "logs"}
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                 stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
              <polyline points="14 2 14 8 20 8"/>
              <line x1="16" y1="13" x2="8" y2="13"/>
              <line x1="16" y1="17" x2="8" y2="17"/>
              <polyline points="10 9 9 9 8 9"/>
            </svg>
          {/if}
          <span class="tab-label">{item.label}</span>
        </button>
      {/each}
    </nav>

    <!-- Theme Toggle -->
    <div class="topbar-actions">
      <button
        class="theme-btn"
        onclick={toggleTheme}
        aria-label={theme === "dark" ? "Switch to light mode" : "Switch to dark mode"}
      >
        {#if theme === "dark"}
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor"
               stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="5"/>
            <line x1="12" y1="1" x2="12" y2="3"/>
            <line x1="12" y1="21" x2="12" y2="23"/>
            <line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/>
            <line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/>
            <line x1="1" y1="12" x2="3" y2="12"/>
            <line x1="21" y1="12" x2="23" y2="12"/>
            <line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/>
            <line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/>
          </svg>
        {:else}
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor"
               stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/>
          </svg>
        {/if}
      </button>
    </div>
  </header>

  <!-- Main Content -->
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
