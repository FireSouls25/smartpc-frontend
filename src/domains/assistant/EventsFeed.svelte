<script lang="ts">
  import { assistant } from "./assistant.store.svelte";
  import { t } from "../../lib/i18n.svelte";
</script>

<aside class="card-xl flex min-h-[240px] min-h-0 flex-col gap-3">
  <h2 class="px-1 text-sm font-bold">{t("events.title")}</h2>
  <ul class="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto">
    {#each assistant.events as ev (ev.id)}
      <li
        class="rounded-2xl border p-3 transition"
        style={ev.status === "running"
          ? "border-color: var(--accent); background: color-mix(in srgb, var(--accent) 7%, var(--surface));"
          : "border-color: var(--border); background: var(--bg); opacity: 0.72;"}
      >
        <div class="flex items-center gap-2">
          <span
            class="dot {ev.status === 'running' ? 'animate-pulse' : ''}"
            style="background: {ev.status === 'running'
              ? 'var(--accent)'
              : 'var(--fg-faint)'};"
          ></span>
          <p class="flex-1 text-[13px] font-semibold leading-snug">
            {t(ev.titleKey)}
          </p>
        </div>
        <p
          class="mt-1.5 pl-4 text-[11px] font-medium"
          style="color: {ev.status === 'running'
            ? 'var(--accent)'
            : 'var(--fg-faint)'};"
        >
          {ev.status === "running"
            ? t("events.running")
            : "✓ " + t("events.done")}
        </p>
      </li>
    {/each}
  </ul>
</aside>
