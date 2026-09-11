<script lang="ts">
  import MatrixOrb from "../../components/ui/matrix-orb.svelte";
  import SelectMenu from "../../shared/SelectMenu.svelte";
  import { assistant } from "./assistant.store.svelte";
  import { t } from "../../lib/i18n.svelte";

  const orbLabels = () => ({
    idle: t("orb.idle"),
    listening: t("orb.listening"),
    thinking: t("orb.thinking"),
  });

  let scrollEl: HTMLDivElement | null = null;

  // Follow the conversation while the user stays near the bottom;
  // never yank them away when they scrolled up to read history.
  $effect(() => {
    void assistant.messages.length;
    const el = scrollEl;
    if (!el) return;
    if (el.scrollHeight - el.scrollTop - el.clientHeight < 160) {
      el.scrollTo({ top: el.scrollHeight });
    }
  });

  function submit(e: SubmitEvent) {
    e.preventDefault();
    void assistant.send(assistant.draft);
  }
</script>

<div class="flex h-full min-h-0 flex-1 flex-col gap-4">
  <div class="card-xl flex flex-col items-center" style="padding-top: 1rem; padding-bottom: 1rem;">
    <MatrixOrb
      state={assistant.orb}
      size={180}
      color="#f04e00"
      labels={orbLabels()}
    />
  </div>

  <div class="card-xl flex min-h-0 flex-1 flex-col gap-3">
    <div
      bind:this={scrollEl}
      class="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto"
      aria-live="polite"
    >
      {#each assistant.messages as m, i (i)}
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
        value={assistant.draft}
        oninput={(e) => assistant.setDraft(e.currentTarget.value)}
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
    {#if assistant.voiceError}
      <p class="error-box">{assistant.voiceError}</p>
    {/if}

    <div class="flex flex-wrap items-center gap-2">
      <SelectMenu
        label={t("chat.provider")}
        value={assistant.activeProvider}
        options={assistant.providers.map((p) => ({
          value: p.id,
          label: p.id,
          disabled: !p.available,
          hint: p.available ? undefined : t("providers.offline"),
        }))}
        onChange={(v) => void assistant.selectProvider(v)}
      />
      <SelectMenu
        label={t("chat.model")}
        value={assistant.activeModel}
        options={assistant.activeModels().map((m) => ({ value: m, label: m }))}
        onChange={(v) => void assistant.selectModel(v)}
      />
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
      <button
        class="chip"
        onclick={() => void assistant.loadProviders()}
        aria-label={t("providers.refresh")}
      >
        ↻ {t("providers.refresh")}
      </button>
    </div>
    {#if assistant.selectError}
      <p class="error-box">{assistant.selectError}</p>
    {/if}
    {#if !assistant.activeAvailable() && !assistant.providersLoading}
      <p class="chip" style="border-color: var(--warn); color: var(--warn);">
        {t("providers.needServer")}
      </p>
    {/if}
  </div>
</div>
