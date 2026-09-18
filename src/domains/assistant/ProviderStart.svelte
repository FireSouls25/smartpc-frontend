<script lang="ts">
  /*
   * Start control for a startable-but-offline provider (today: ollama).
   * Renders nothing when the provider is available or not startable.
   * The availability watch picks the server up once it answers.
   */
  import { assistant } from "./assistant.store.svelte";
  import { t } from "../../lib/i18n.svelte";

  let { providerId }: { providerId: string } = $props();

  const info = () => assistant.providers.find((p) => p.id === providerId);
</script>

{#if info() && !info()!.available && info()!.startable}
  {#if info()!.installed === false}
    <p class="chip" style="border-color: var(--warn); color: var(--warn);">
      {t("providers.notInstalled")}
    </p>
  {:else}
    <button
      class="btn btn-ghost"
      style="padding: 0.375rem 0.75rem; font-size: 0.75rem;"
      disabled={assistant.startingProvider !== null}
      onclick={() => void assistant.startProvider(providerId)}
    >
      {assistant.startingProvider === providerId
        ? t("providers.starting")
        : `${t("providers.start")} ${providerId}`}
    </button>
  {/if}
  {#if assistant.startError}
    <p class="error-box">{assistant.startError}</p>
  {/if}
{/if}
