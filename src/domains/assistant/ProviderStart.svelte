<script lang="ts">
  /*
   * Start control for a startable-but-offline provider (today: ollama).
   * Renders nothing when the provider is available or not startable.
   * The availability watch picks the server up once it answers.
   */
  import { providerStore as providers } from "./providers.store.svelte";
  import { t } from "../../lib/i18n.svelte";

  let { providerId }: { providerId: string } = $props();

  const info = () => providers.providers.find((p) => p.id === providerId);
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
      disabled={providers.startingProvider !== null}
      onclick={() => void providers.startProvider(providerId)}
    >
      {providers.startingProvider === providerId
        ? t("providers.starting")
        : `${t("providers.start")} ${providerId}`}
    </button>
  {/if}
  {#if providers.startError}
    <p class="error-box">{providers.startError}</p>
  {/if}
{/if}
