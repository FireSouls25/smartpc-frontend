<script lang="ts">
  import { assistant } from "../domains/assistant/assistant.store.svelte";
  import { t } from "../lib/i18n.svelte";

  let key = $state("");

  const modal = () => assistant.keyModal;

  function close() {
    key = "";
    assistant.closeKeyModal();
  }

  async function save(e: SubmitEvent) {
    e.preventDefault();
    const m = modal();
    if (!m || assistant.keyBusy) return;
    await assistant.saveKey(m.provider, key);
    key = "";
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") close();
  }
</script>

<svelte:window onkeydown={onkeydown} />

{#if modal()}
  {@const m = modal()!}
  <div
    class="fixed inset-0 z-50 grid place-items-center p-4"
    style="background: rgb(17 17 20 / 0.45); backdrop-filter: blur(2px);"
    onclick={(e) => {
      if (e.target === e.currentTarget) close();
    }}
    role="presentation"
  >
    <div
      class="card-xl msg-in w-full max-w-sm"
      role="dialog"
      aria-modal="true"
      aria-label={t("aikey.title")}
    >
      <h2 class="mb-1 text-lg font-bold">{t("aikey.title")}</h2>
      <p class="muted mb-4 text-sm">{t("aikey.desc")}</p>
      <form onsubmit={save} class="flex flex-col gap-3">
        <div>
          <label class="label" for="aikey-input">{m?.provider}</label>
          <input
            id="aikey-input"
            class="field"
            style="border-radius: 1rem;"
            type="password"
            required
            minlength={8}
            autocomplete="new-password"
            placeholder={t("aikey.placeholder")}
            bind:value={key}
          />
        </div>
        {#if assistant.keyError}
          <p class="error-box">{assistant.keyError}</p>
        {/if}
        <div class="flex justify-end gap-2">
          {#if m?.hasKey}
            <button
              type="button"
              class="btn btn-danger mr-auto"
              disabled={assistant.keyBusy}
              onclick={() => void assistant.deleteKey(m.provider)}
            >
              {t("aikey.remove")}
            </button>
          {/if}
          <button
            type="button"
            class="btn btn-ghost"
            disabled={assistant.keyBusy}
            onclick={close}
          >
            {t("aikey.cancel")}
          </button>
          <button type="submit" class="btn btn-primary" disabled={assistant.keyBusy}>
            {assistant.keyBusy ? t("common.loading") : t("aikey.save")}
          </button>
        </div>
      </form>
    </div>
  </div>
{/if}
