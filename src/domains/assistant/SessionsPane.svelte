<script lang="ts">
  import { assistant } from "./assistant.store.svelte";
  import { t } from "../../lib/i18n.svelte";

  function timeOf(iso: string): string {
    const d = new Date(iso);
    if (Number.isNaN(d.getTime())) return "";
    const now = new Date();
    if (d.toDateString() === now.toDateString()) {
      return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
    }
    return d.toLocaleDateString([], { day: "2-digit", month: "short" });
  }
</script>

<aside class="card-xl flex h-full min-h-[240px] min-h-0 flex-col gap-3">
  <div class="flex items-center justify-between px-1">
    <h2 class="text-sm font-bold">{t("sessions.title")}</h2>
    <button
      class="icon-btn"
      style="height: 2rem; width: 2rem;"
      onclick={() => assistant.newChat()}
      aria-label={t("sessions.new")}
      title={t("sessions.new")}
    >
      <svg
        viewBox="0 0 24 24"
        class="h-4 w-4"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
      >
        <path d="M12 5v14M5 12h14" />
      </svg>
      <span class="tip">{t("sessions.new")}</span>
    </button>
  </div>

  {#if assistant.sessions.length === 0}
    <p class="faint px-1 text-xs leading-relaxed">{t("sessions.empty")}</p>
  {/if}

  <ul class="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto">
    {#each assistant.sessions as s (s.id)}
      {@const active = assistant.activeSessionId === s.id}
      <li
        class="flex items-stretch gap-1 rounded-2xl border transition"
        style={active
          ? "border-color: var(--accent); background: color-mix(in srgb, var(--accent) 7%, var(--bg));"
          : "border-color: var(--border); background: var(--bg);"}
      >
        <button
          class="min-w-0 flex-1 p-3 text-left"
          onclick={() => void assistant.openSession(s.id)}
        >
          <p class="truncate text-[13px] font-semibold leading-snug">
            {s.title}
          </p>
          {#if s.preview}
            <p class="faint mt-0.5 truncate text-[11px]">{s.preview}</p>
          {/if}
          <p class="faint mt-1 text-[10px]">
            {timeOf(s.updated_at)} · {s.message_count}
          </p>
        </button>
        <button
          class="faint self-start p-2 transition hover:text-[var(--danger)]"
          title={t("sessions.delete")}
          aria-label={t("sessions.delete")}
          onclick={() => void assistant.deleteSession(s.id)}
        >
          <svg
            viewBox="0 0 24 24"
            class="h-4 w-4"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path d="M4 7h16M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2M6 7l1 13h10l1-13" />
          </svg>
        </button>
      </li>
    {/each}
  </ul>
</aside>
