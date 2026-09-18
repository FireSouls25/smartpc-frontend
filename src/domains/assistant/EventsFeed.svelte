<script lang="ts">
  import { chatStore as chat } from "./chat.store.svelte";
  import { providerStore as providers } from "./providers.store.svelte";
  import { contextPct, fmtK } from "../../lib/format";
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

  function ctxPct(): number {
    return contextPct(chat.contextUsed, providers.contextWindow);
  }

  function ctxColor(): string {
    const p = ctxPct();
    if (p >= 90) return "var(--danger)";
    if (p >= 70) return "var(--warn)";
    return "var(--accent)";
  }
</script>

<aside class="card-xl flex h-full min-h-[240px] min-h-0 flex-col gap-3">
  <h2 class="px-1 text-sm font-bold">{t("events.title")}</h2>
  <div
    class="shrink-0 rounded-2xl border px-3 py-2"
    style="border-color: var(--border); background: var(--bg);"
  >
    <div class="flex items-center justify-between gap-2">
      <p class="text-[11px] font-bold">{t("context.label")}</p>
      <p class="font-mono text-[11px]">
        {fmtK(chat.contextUsed)} / {providers.contextWindow == null
          ? "—"
          : fmtK(providers.contextWindow)}
      </p>
    </div>
    {#if providers.contextWindow != null}
      <div
        class="mt-1.5 h-1 overflow-hidden rounded-full"
        style="background: var(--border);"
      >
        <div
          class="h-full rounded-full transition-all"
          style="width: {ctxPct()}%; background: {ctxColor()};"
        ></div>
      </div>
    {/if}
  </div>
  {#if chat.events.length === 0}
    <p class="faint px-1 text-xs leading-relaxed">{t("events.empty")}</p>
  {/if}
  <ul class="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto">
    {#each chat.events as ev (ev.id)}
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
