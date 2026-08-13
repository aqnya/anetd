<script lang="ts">
  let {
    checked = $bindable(false),
    label = "",
    disabled = false,
    onChange,
  }: {
    checked: boolean;
    label?: string;
    disabled?: boolean;
    onChange?: (value: boolean) => void;
  } = $props();

  function toggle() {
    if (disabled) return;
    checked = !checked;
    onChange?.(checked);
  }
</script>

<button
  class="switch"
  class:on={checked}
  role="switch"
  aria-checked={checked}
  aria-label={label}
  disabled={disabled}
  onclick={toggle}
  type="button"
>
  <span class="switch-thumb"></span>
</button>
