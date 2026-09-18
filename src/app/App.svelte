<script lang="ts">
  import { onMount } from "svelte";
  import { route, navigate, syncAuthRoute } from "./router.svelte";
  import { auth } from "../domains/auth/auth.store.svelte";
  import { initLang, t } from "../lib/i18n.svelte";
  import { initTheme } from "../lib/theme.svelte";
  import LoginPage from "../domains/auth/LoginPage.svelte";
  import RegisterPage from "../domains/auth/RegisterPage.svelte";
  import Shell from "./Shell.svelte";

  initLang();
  initTheme();

  let ready = $state(false);
  onMount(async () => {
    await auth.restore();
    syncAuthRoute(auth.user);
    ready = true;
  });

  // Unknown hashes fall back to home (parse flags them; redirect once).
  $effect(() => {
    if (ready && route.unknown) navigate("");
  });
</script>

{#if !ready}
  <div class="grid min-h-screen place-items-center">
    <p class="muted text-sm">{t("common.loading")}</p>
  </div>
{:else if route.name === "login"}
  <LoginPage />
{:else if route.name === "register"}
  <RegisterPage />
{:else}
  <Shell />
{/if}
