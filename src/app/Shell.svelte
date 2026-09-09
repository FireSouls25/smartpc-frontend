<script lang="ts">
  import { onMount } from "svelte";
  import { auth } from "../domains/auth/auth.store.svelte";
  import { t, type I18nKey } from "../lib/i18n.svelte";
  import { navigate } from "./router.svelte";
  import Logo from "../shared/Logo.svelte";
  import LangTheme from "../shared/LangTheme.svelte";
  import HomePage from "../domains/home/HomePage.svelte";
  import Placeholder from "../domains/home/Placeholder.svelte";

  const tabs = ["assistant", "activity", "gestures", "actions", "settings"] as const;
  type Tab = (typeof tabs)[number];
  let tab = $state<Tab>("assistant");

  onMount(() => {
    if (!auth.user) navigate("login");
  });

  async function logout() {
    await auth.logout();
    navigate("login");
  }

  async function remove() {
    if (!window.confirm(t("auth.deleteAsk"))) return;
    await auth.deleteAccount();
    navigate("register");
  }
</script>

<div class="flex min-h-screen">
  <aside
    class="flex w-56 shrink-0 flex-col gap-1 border-r p-4"
    style="border-color: var(--border); background: var(--surface);"
  >
    <div class="mb-4 flex items-center gap-2.5 px-1">
      <Logo />
      <p class="font-bold">Smart PC</p>
    </div>
    {#each tabs as id (id)}
      <button
        class="rounded-lg px-3 py-2 text-left text-sm font-medium"
        style={tab === id
          ? "background: var(--surface-2); color: var(--fg);"
          : "color: var(--fg-muted);"}
        onclick={() => (tab = id)}
      >
        {t(`nav.${id}` as I18nKey)}
      </button>
    {/each}
    <div
      class="mt-auto flex flex-col gap-2 border-t pt-3"
      style="border-color: var(--border);"
    >
      <p class="faint truncate px-1 text-xs">{auth.user?.email}</p>
      <button class="btn btn-ghost" onclick={logout}>{t("auth.logout")}</button>
      <button class="btn btn-danger" onclick={remove}>{t("auth.delete")}</button>
    </div>
  </aside>

  <div class="min-w-0 flex-1">
    <header
      class="flex items-center justify-between gap-3 border-b px-5 py-3"
      style="border-color: var(--border);"
    >
      <span class="chip">
        <span class="dot" style="background: var(--success);"></span>
        {t("home.provider")}
      </span>
      <LangTheme />
    </header>
    <main class="mx-auto max-w-3xl p-5">
      {#if tab === "assistant"}
        <HomePage />
      {:else}
        <Placeholder domain={t(`nav.${tab}` as I18nKey)} />
      {/if}
    </main>
  </div>
</div>
