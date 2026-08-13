<script lang="ts">
  import { confirmState, settleConfirm } from "./confirm";
  import { t } from "./i18n";
  import Icon from "./Icon.svelte";
</script>

{#if confirmState}
  <div
    class="dialog-backdrop"
    role="presentation"
    onclick={(e) => {
      if (e.target === e.currentTarget) settleConfirm(false);
    }}
  >
    <div class="dialog" role="alertdialog" aria-modal="true" aria-labelledby="dialog-title">
      <div class="dialog-header">
        <Icon name={confirmState.opts.danger ? "alert" : "info"} size={18} />
        <h2 id="dialog-title">{confirmState.opts.title}</h2>
      </div>
      {#if confirmState.opts.message}
        <p class="dialog-message">{confirmState.opts.message}</p>
      {/if}
      <div class="dialog-actions">
        <button class="btn" onclick={() => settleConfirm(false)}>
          {confirmState.opts.cancelText ?? t("common.cancel")}
        </button>
        <button
          class="btn {confirmState.opts.danger ? "btn-danger" : "btn-primary"}"
          onclick={() => settleConfirm(true)}
        >
          {confirmState.opts.confirmText ?? t("common.confirm")}
        </button>
      </div>
    </div>
  </div>
{/if}
