<script lang="ts">
  import MatrixOrb from "../../components/ui/matrix-orb.svelte";
  import { assistant, PROVIDERS } from "./assistant.store.svelte";
  import { t } from "../../lib/i18n.svelte";

  let draft = $state("");

  const orbLabels = () => ({
    idle: t("orb.idle"),
    listening: t("orb.listening"),
    thinking: t("orb.thinking"),
  });

  const models: () => readonly string[] = () =>
    PROVIDERS.find((p) => p.id === assistant.provider)?.models ?? [];

  function submit(e: SubmitEvent) {
    e.preventDefault();
    assistant.send(draft);
    draft = "";
  }
</script>

<div class="flex min-h-0 flex-1 flex-col gap-4">
  <div class="card-xl flex flex-col items-center !py-4">
    <MatrixOrb
      state={assistant.orb}
      size={180}
      color="#8839ef"
      labels={orbLabels()}
    />
  </div>

  <div class="card-xl flex min-h-[320px] min-h-0 flex-1 flex-col gap-3">
    <div
      class="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto"
      aria-live="polite"
    >
      {#each assistant.messages as m (m.id)}
        {#if m.role === "user"}
          <div class="msg-in flex justify-end">
            <p class="bubble-user">{m.text}</p>
          </div>
        {:else}
          <div class="msg-in flex justify-start">
            <p class="bubble-assistant">
              {m.textKey ? t(m.textKey) : m.text}
            </p>
          </div>
        {/if}
      {/each}
    </div>

    <form
      onsubmit={submit}
      class="flex items-center gap-2 rounded-2xl border p-2"
      style="border-color: var(--border-strong); background: var(--bg);"
    >
      <button
        type="button"
        class="icon-btn shrink-0"
        style={assistant.listening
          ? "border-color: var(--danger); color: var(--danger);"
          : ""}
        onclick={() => assistant.toggleListening()}
        aria-label={t("orb.listening")}
        aria-pressed={assistant.listening}
      >
        <svg
          viewBox="0 0 24 24"
          class="h-5 w-5"
          fill="none"
          stroke="currentColor"
          stroke-width="1.8"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <rect x="9" y="3" width="6" height="11" rx="3" />
          <path d="M5 11a7 7 0 0 0 14 0M12 18v3" />
        </svg>
      </button>
      <input
        class="min-w-0 flex-1 bg-transparent text-sm outline-none"
        style="color: var(--fg);"
        placeholder={t("chat.placeholder")}
        bind:value={draft}
      />
      <button class="btn btn-primary shrink-0" type="submit">
        <svg
          viewBox="0 0 24 24"
          class="h-4 w-4"
          fill="none"
          stroke="currentColor"
          stroke-width="1.8"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <path d="M22 2 11 13M22 2l-7 20-4-9-9-4z" />
        </svg>
        {t("chat.send")}
      </button>
    </form>

    <div class="flex flex-wrap items-center gap-2">
      <label class="chip">
        <span class="faint">{t("chat.provider")}</span>
        <select
          class="bg-transparent text-xs font-semibold outline-none"
          style="color: var(--fg);"
          value={assistant.provider}
          onchange={(e) => assistant.setProvider(e.currentTarget.value)}
        >
          {#each PROVIDERS as p (p.id)}
            <option value={p.id}>{p.id}</option>
          {/each}
        </select>
      </label>
      <label class="chip">
        <span class="faint">{t("chat.model")}</span>
        <select
          class="bg-transparent text-xs font-semibold outline-none"
          style="color: var(--fg);"
          value={assistant.model}
          onchange={(e) => assistant.setModel(e.currentTarget.value)}
        >
          {#each models() as m (m)}
            <option value={m}>{m}</option>
          {/each}
        </select>
      </label>
      <button
        class="chip"
        onclick={() => assistant.toggleGestures()}
        aria-pressed={assistant.gesturesOn}
      >
        <span
          class="dot"
          style="background: {assistant.gesturesOn
            ? 'var(--success)'
            : 'var(--border-strong)'};"
        ></span>
        {t("chat.gestures")}
      </button>
    </div>
    <p class="faint text-[11px]">{t("chat.mockNote")}</p>
  </div>
</div>
