<script lang="ts">
  import { onMount } from "svelte";
  import { auth } from "../domains/auth/auth.store.svelte";
  import { navigate } from "./router.svelte";
  import TopBar from "./TopBar.svelte";
  import EventsFeed from "../domains/assistant/EventsFeed.svelte";
  import CenterPanel from "../domains/assistant/CenterPanel.svelte";
  import SettingsPage from "../domains/settings/SettingsPage.svelte";

  let view = $state<"main" | "settings">("main");

  onMount(() => {
    if (!auth.user) navigate("login");
  });
</script>

<div class="dot-bg min-h-screen">
  <div class="mx-auto flex min-h-screen max-w-6xl flex-col px-4 md:px-6">
    <TopBar onSettings={() => (view = "settings")} />
    {#if view === "main"}
      <div class="grid flex-1 gap-4 pb-8 lg:grid-cols-[300px_minmax(0,1fr)]">
        <EventsFeed />
        <CenterPanel />
      </div>
    {:else}
      <div class="pb-8">
        <SettingsPage onBack={() => (view = "main")} />
      </div>
    {/if}
  </div>
</div>
