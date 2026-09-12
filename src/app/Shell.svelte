<script lang="ts">
  import { onMount } from "svelte";
  import { auth } from "../domains/auth/auth.store.svelte";
  import { assistant } from "../domains/assistant/assistant.store.svelte";
  import { navigate } from "./router.svelte";
  import TopBar from "./TopBar.svelte";
  import EventsFeed from "../domains/assistant/EventsFeed.svelte";
  import CenterPanel from "../domains/assistant/CenterPanel.svelte";
  import SessionsPane from "../domains/assistant/SessionsPane.svelte";
  import SettingsPage from "../domains/settings/SettingsPage.svelte";
  import ApiKeyModal from "../shared/ApiKeyModal.svelte";
  import { fetchHealth, SIDECAR_PROTOCOL } from "../lib/api";
  import { t } from "../lib/i18n.svelte";

  let view = $state<"main" | "settings">("main");
  let protoOk = $state(true);

  onMount(() => {
    fetchHealth()
      .then((h) => {
        protoOk = h.protocol === SIDECAR_PROTOCOL;
      })
      .catch(() => {});
  });

  onMount(() => {
    if (!auth.user) navigate("login");
    else {
      void assistant.loadProviders().then(() => assistant.refreshSessions());
    }
  });
</script>

<div class="dot-bg min-h-dvh lg:h-dvh lg:overflow-hidden">
  <div
    class="mx-auto flex min-h-dvh max-w-7xl flex-col px-4 md:px-6 lg:h-full lg:min-h-0"
  >
    {#if !protoOk}
      <div
        class="mb-3 rounded-2xl border p-3 text-center text-sm font-semibold"
        style="border-color: var(--danger); color: var(--danger); background: color-mix(in srgb, var(--danger) 8%, transparent);"
      >
        {t("protocol.mismatch")}
      </div>
    {/if}
    <TopBar onSettings={() => (view = "settings")} />
    {#if view === "main"}
      <div
        class="grid flex-1 gap-4 pb-4 md:pb-8 lg:min-h-0 lg:grid-cols-[280px_minmax(0,1fr)] xl:grid-cols-[280px_minmax(0,1fr)_300px]"
      >
        <div class="pane-a flex min-h-0 flex-col">
          <EventsFeed />
        </div>
        <div class="pane-b flex min-h-0 flex-col">
          <CenterPanel />
        </div>
        <div class="pane-c hidden min-h-0 flex-col xl:flex">
          <SessionsPane />
        </div>
      </div>
    {:else}
      <div class="min-h-0 flex-1 pb-8 lg:overflow-y-auto">
        <SettingsPage onBack={() => (view = "main")} />
      </div>
    {/if}
    <ApiKeyModal />
  </div>
</div>
