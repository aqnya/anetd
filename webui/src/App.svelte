<script lang="ts">
  import Dashboard from "./pages/Dashboard.svelte";
  import Settings from "./pages/Settings.svelte";
  import Logs from "./pages/Logs.svelte";

  type Page = "dashboard" | "settings" | "logs";

  const tabs: { id: Page; label: string }[] = [
    { id: "dashboard", label: "Dashboard" },
    { id: "settings", label: "Settings" },
    { id: "logs", label: "Logs" },
  ];

  let current = $state<Page>("dashboard");
</script>

<header>
  <span class="brand">⚡ Anetd</span>
  <nav>
    {#each tabs as tab}
      <button
        class="tab"
        class:active={current === tab.id}
        onclick={() => (current = tab.id)}
      >
        {tab.label}
      </button>
    {/each}
  </nav>
</header>

<main>
  {#if current === "dashboard"}
    <Dashboard />
  {:else if current === "settings"}
    <Settings />
  {:else if current === "logs"}
    <Logs />
  {/if}
</main>

<style>
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    background: var(--bg-card);
    border-bottom: 1px solid var(--border);
    position: sticky;
    top: 0;
    z-index: 10;
    flex-wrap: wrap;
    gap: 8px;
  }

  .brand {
    font-size: 1.1rem;
    font-weight: 700;
    letter-spacing: -0.02em;
  }

  nav {
    display: flex;
    gap: 4px;
  }

  .tab {
    background: none;
    border: none;
    color: var(--text-dim);
    padding: 6px 14px;
    border-radius: 8px;
    font-size: 0.9rem;
    cursor: pointer;
    transition: color 0.15s, background 0.15s;
    font-family: var(--font);
  }

  .tab:hover {
    color: var(--text);
    background: var(--bg);
  }

  .tab.active {
    color: var(--text);
    background: var(--accent-dim);
  }

  main {
    max-width: 720px;
    margin: 0 auto;
    padding: 20px 16px 40px;
  }
</style>
