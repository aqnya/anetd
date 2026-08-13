<script lang="ts">
  import { api, restartDaemon, type ConfigValue } from "../api/anetd";
  import Icon from "../lib/Icon.svelte";
  import Switch from "../lib/Switch.svelte";
  import { confirmDialog } from "../lib/confirm";
  import { lang, setLang, t } from "../lib/i18n";
  import { toast } from "../lib/toast";
  import { patchToml, MODULE } from "../lib/ui";

  type Tab = "basic" | "advanced";

  const KNOWN_KEYS = [
    "rules",
    "standalone",
    "multi_thread",
    "dns_server",
    "dns_port",
    "dns_upstream",
    "battery_saver",
    "webui_socket",
  ] as const;

  const textFields = [
    { key: "rules", label: "settings.rules_path", hint: "settings.rules_path.hint" },
    { key: "dns_upstream", label: "settings.dns_upstream" },
    { key: "webui_socket", label: "settings.webui_socket" },
    { key: "dns_port", label: "settings.dns_port", type: "number" },
  ];

  const boolFields = [
    { key: "standalone", label: "settings.standalone" },
    { key: "multi_thread", label: "settings.multi_thread" },
    { key: "dns_server", label: "settings.dns_server" },
    { key: "battery_saver", label: "settings.battery_saver" },
  ];

  let tab = $state<Tab>("basic");
  let configPath = $state(`${MODULE}/config.toml`);
  let content = $state("");
  let parsedContent = $state("");
  let values = $state<Record<string, ConfigValue>>({});
  let dirty = $state(false);
  let loaded = $state(false);
  let loadFailed = $state(false);
  let saving = $state(false);
  let restarting = $state(false);

  const form = $state<Record<string, string | boolean | number>>({
    rules: "",
    dns_port: 53,
    dns_upstream: "8.8.8.8:53",
    webui_socket: "/data/adb/modules/anetd/webui.sock",
  });

  const bools = $state<Record<string, boolean>>({
    standalone: false,
    multi_thread: true,
    dns_server: false,
    battery_saver: false,
  });

  const manualEdit = $derived(content !== parsedContent);

  function applyValuesToForm() {
    for (const key of KNOWN_KEYS) {
      const v = values[key];
      if (v === undefined || v === null) continue;
      if (key in bools) {
        bools[key] = Boolean(v);
      } else if (key === "dns_port") {
        const n = Number(v);
        form[key] = Number.isFinite(n) ? n : 53;
      } else {
        form[key] = v;
      }
    }
  }

  async function load(silent = false) {
    if (!silent) {
      loaded = false;
      loadFailed = false;
    }
    try {
      const r = await api.loadConfig();
      content = r.content ?? "";
      values = r.values ?? {};
      parsedContent = content;
      applyValuesToForm();
      dirty = false;
      loaded = true;
    } catch {
      loadFailed = true;
      loaded = false;
    }
  }

  function markDirty() {
    dirty = true;
  }

  function toPatch(): Record<string, string | boolean | number> {
    const patch: Record<string, string | boolean | number> = {};
    for (const key of KNOWN_KEYS) {
      patch[key] = key in bools ? bools[key] : form[key];
    }
    return patch;
  }

  async function handleSave() {
    saving = true;
    try {
      const next = tab === "advanced" ? content : patchToml(content, toPatch());
      const r = await api.saveConfig(next);
      if (r.ok) {
        toast(t("toast.saved"), "success");
        toast(t("toast.config_restart_hint"), "info", 4000);
        await load(true);
      } else {
        toast(`${t("toast.save_failed")}：${r.error ?? ""}`, "error", 5000);
      }
    } catch (e) {
      toast(t("toast.save_failed"), "error");
      console.error(e);
    } finally {
      saving = false;
    }
  }

  async function handleReset() {
    if (dirty && !(await confirmDialog({
      title: t("confirm.reset_config.title"),
      message: t("confirm.reset_config.message"),
      danger: true,
    }))) {
      return;
    }
    await load(true);
    toast(t("common.refresh"), "info", 1500);
  }

  async function handleRestart() {
    const confirmed = await confirmDialog({
      title: t("confirm.restart.title"),
      message: t("confirm.restart.message"),
      confirmText: t("settings.restart_to_apply"),
      danger: true,
    });
    if (!confirmed) return;
    restarting = true;
    const ok = await restartDaemon();
    restarting = false;
    toast(ok ? t("toast.restarted") : t("toast.restart_failed"), ok ? "success" : "error");
  }
</script>

<header class="page-header">
  <div>
    <h1 class="page-title">{t("settings.title")}</h1>
    <p class="page-subtitle">{t("settings.subtitle", { path: configPath })}</p>
  </div>
</header>

{#if loadFailed}
  <div class="card">
    <div class="card-body center-col">
      <Icon name="alert" size={22} />
      <p class="field-hint">{t("banner.offline")}</p>
      <button class="btn" onclick={() => void load()}>{t("common.retry")}</button>
    </div>
  </div>
{:else if !loaded}
  <div class="card">
    <div class="card-body center-col">
      <span class="spinner"></span>
      <p class="field-hint">{t("common.loading")}</p>
    </div>
  </div>
{:else}
  <div class="segmented">
    <button class="seg-btn" class:active={tab === "basic"} onclick={() => (tab = "basic")}>
      {t("settings.basic")}
    </button>
    <button class="seg-btn" class:active={tab === "advanced"} onclick={() => (tab = "advanced")}>
      {t("settings.advanced")}
    </button>
  </div>

  {#if tab === "basic"}
    <section class="card">
      <div class="card-header">
        <Icon name="settings" size={14} />
        {t("settings.basic")}
      </div>
      <div class="card-body">
        <p class="field-hint">{t("settings.basic_hint")}</p>

        {#each textFields as field (field.key)}
          <label class="field">
            <span class="field-label">{t(field.label)}</span>
            <input
              type={field.type === "number" ? "number" : "text"}
              bind:value={form[field.key]}
              oninput={markDirty}
              spellcheck="false"
            />
            {#if field.hint}
              <span class="field-hint">{t(field.hint)}</span>
            {/if}
          </label>
        {/each}

        <div class="field-list">
          {#each boolFields as field (field.key)}
            <label class="field-row">
              <span>
                <span class="row-title">{t(field.label)}</span>
              </span>
              <Switch bind:checked={bools[field.key]} onChange={markDirty} label={t(field.label)} />
            </label>
          {/each}
        </div>

        <label class="field-row">
          <span>
            <span class="row-title">{t("settings.language")}</span>
          </span>
          <select
            class="select"
            value={lang.value}
            onchange={(e) => setLang((e.target as HTMLSelectElement).value as "zh" | "en")}
          >
            <option value="zh">中文</option>
            <option value="en">English</option>
          </select>
        </label>
      </div>
    </section>
  {:else}
    <section class="card">
      <div class="card-header">
        <Icon name="terminal" size={14} />
        {t("settings.editor")}
      </div>
      <div class="card-body">
        <p class="field-hint">{t("settings.advanced_hint")}</p>
        <textarea
          class="code-editor"
          rows="18"
          spellcheck="false"
          bind:value={content}
          oninput={markDirty}
          aria-label={t("settings.editor")}
        ></textarea>
      </div>
    </section>
  {/if}

  <div class="save-bar">
    <div class="save-status">
      {#if dirty}
        <span class="unsaved"><span class="unsaved-dot"></span>{t("settings.dirty")}</span>
      {/if}
      {#if manualEdit && tab === "basic"}
        <span class="field-hint">{t("settings.manual_edit_hint")}</span>
      {/if}
    </div>
    <div class="save-actions">
      <button class="btn" onclick={handleReset}>{t("settings.reset")}</button>
      <button class="btn btn-primary" onclick={handleSave} disabled={saving || !dirty}>
        {#if saving}<span class="spinner"></span>{/if}
        {t("settings.save_apply")}
      </button>
      <button class="btn btn-danger" onclick={handleRestart} disabled={restarting}>
        {#if restarting}<span class="spinner"></span>{/if}
        <Icon name="power" size={14} />
        {t("settings.restart_to_apply")}
      </button>
    </div>
  </div>

  <details class="card ref-card">
    <summary class="card-header ref-summary">
      <Icon name="info" size={14} />
      {t("settings.config_ref")}
    </summary>
    <div class="card-body">
      <pre class="ref-block"># anetd config.toml
rules = "/data/adb/modules/anetd/rules"
standalone = false      # 后台守护进程模式
multi_thread = true     # 多线程 Tokio 运行时
dns_server = false      # 内置 DNS 服务器
dns_port = 53
dns_upstream = "8.8.8.8:53"
battery_saver = false   # 省电模式
webui_socket = "/data/adb/modules/anetd/webui.sock"</pre>
    </div>
  </details>
{/if}
