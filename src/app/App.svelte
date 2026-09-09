<script lang="ts">
  import { onMount } from "svelte";
  import { route } from "./router.svelte";
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
    ready = true;
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
