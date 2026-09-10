<script lang="ts">
  /*
   * Themed dropdown menu. Opens upward by default (chat blocks sit low on
   * the screen); set align="down" for top-anchored contexts.
   */
  export interface MenuOption {
    value: string;
    label: string;
    disabled?: boolean;
    hint?: string;
  }

  let {
    options,
    value,
    onChange,
    label,
    align = "up",
  }: {
    options: MenuOption[];
    value: string;
    onChange: (v: string) => void;
    label: string;
    align?: "up" | "down";
  } = $props();

  let open = $state(false);
  const current = () => options.find((o) => o.value === value);

  function pick(v: string, disabled?: boolean): void {
    if (disabled) return;
    open = false;
    onChange(v);
  }
</script>

<svelte:window
  onclick={(e) => {
    const el = e.target as HTMLElement | null;
    if (el?.closest?.("[data-selectmenu]")) return;
    open = false;
  }}
  onkeydown={(e) => {
    if (e.key === "Escape") open = false;
  }}
/>

<div class="relative" data-selectmenu>
  <button
    type="button"
    class="chip"
    style={open ? "border-color: var(--accent);" : ""}
    onclick={() => (open = !open)}
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-label={label}
  >
    <span class="faint">{label}</span>
    <span class="font-semibold" style="color: var(--fg);">
      {current()?.label ?? value}
    </span>
    <svg
      viewBox="0 0 24 24"
      class="h-3.5 w-3.5 transition-transform {open ? 'rotate-180' : ''}"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
    >
      <path d="M6 9l6 6 6-6" />
    </svg>
  </button>

  {#if open}
    <div
      role="listbox"
      aria-label={label}
      class="dropdown msg-in {align === 'up' ? 'dropdown-up' : ''}"
    >
      {#each options as o (o.value)}
        <button
          type="button"
          role="option"
          aria-selected={o.value === value}
          aria-checked={o.value === value}
          disabled={o.disabled}
          onclick={() => pick(o.value, o.disabled)}
        >
          <span
            class="dot"
            style="background: {o.value === value
              ? 'var(--accent)'
              : 'var(--border-strong)'};"
          ></span>
          <span class="flex-1 text-left">{o.label}</span>
          {#if o.hint}
            <span class="faint text-[11px]">{o.hint}</span>
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</div>
