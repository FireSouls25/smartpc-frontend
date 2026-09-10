<script lang="ts">
  import { assistant } from "./assistant.store.svelte";
  import { t } from "../../lib/i18n.svelte";

  function color(status: string): string {
    if (status === "running") return "var(--accent)";
    if (status === "failed") return "var(--danger)";
    return "var(--fg-faint)";
  }

  function box(status: string): string {
    if (status === "running")
      return "border-color: var(--accent); background: color-mix(in srgb, var(--accent) 7%, var(--surface));";
    if (status === "failed")
      return "border-color: var(--danger); background: color-mix(in srgb, var(--danger) 7%, var(--surface));";
    return "border-color: var(--border); background: var(--bg); opacity: 0.72;";
  }

  function label(status: string): string {
    if (status === "running") return t("events.running");
    if (status === "failed") return t("events.failed");
    return "✓ " + t("events.done");
  }
</script>

<aside class="card-xl flex h-full min-h-[240px] min-h-0 flex-col gap-3">
  <h2 class="px-1 text-sm font-bold">{t("events.title")}</h2>
  {#if assistant.events.length === 0}
    <p class="faint px-1 text-xs leading-relaxed">{t("events.empty")}</p>
  {/if}
  <ul class="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto">
    {#each assistant.events as ev (ev.id)}
      <li class="rounded-2xl border p-3 transition" style={box(ev.status)}>
        <div class="flex items-center gap-2">
          <span
            class="dot {ev.status === 'running' ? 'animate-pulse' : ''}"
            style="background: {color(ev.status)};"
          ></span>
          <p class="flex-1 text-[13px] font-semibold leading-snug">
            {ev.title}
          </p>
        </div>
        <p
          class="mt-1.5 pl-4 text-[11px] font-medium"
          style="color: {color(ev.status)};"
        >
          {label(ev.status)}
        </p>
      </li>
    {/each}
  </ul>
</aside>
