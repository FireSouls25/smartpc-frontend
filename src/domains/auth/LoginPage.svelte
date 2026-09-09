<script lang="ts">
  import { auth } from "./auth.store.svelte";
  import { t } from "../../lib/i18n.svelte";
  import { navigate } from "../../app/router.svelte";
  import Logo from "../../shared/Logo.svelte";
  import LangTheme from "../../shared/LangTheme.svelte";

  let email = $state("");
  let password = $state("");
  let error = $state("");
  let busy = $state(false);

  $effect(() => {
    if (auth.user) navigate("");
  });

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (busy) return;
    busy = true;
    error = "";
    try {
      await auth.login(email, password);
      navigate("");
    } catch (err) {
      error = err instanceof Error ? err.message : "Error";
    } finally {
      busy = false;
    }
  }
</script>

<div class="grid min-h-screen place-items-center p-4">
  <div class="absolute right-4 top-4"><LangTheme /></div>
  <div class="card w-full max-w-sm">
    <div class="mb-5 flex items-center gap-3">
      <Logo />
      <div>
        <p class="font-bold">Smart PC</p>
        <p class="faint text-xs">{t("brand.tagline")}</p>
      </div>
    </div>
    <h1 class="mb-4 text-xl font-bold">{t("auth.loginTitle")}</h1>
    <form onsubmit={submit} class="flex flex-col gap-3">
      <div>
        <label class="label" for="login-email">{t("auth.email")}</label>
        <input
          id="login-email"
          class="field"
          type="email"
          required
          autocomplete="email"
          bind:value={email}
        />
      </div>
      <div>
        <label class="label" for="login-password">{t("auth.password")}</label>
        <input
          id="login-password"
          class="field"
          type="password"
          required
          autocomplete="current-password"
          bind:value={password}
        />
      </div>
      {#if error}<p class="error-box">{error}</p>{/if}
      <button class="btn btn-primary mt-1" type="submit" disabled={busy}>
        {busy ? t("common.loading") : t("auth.loginBtn")}
      </button>
    </form>
    <button
      class="muted mt-4 text-sm underline"
      onclick={() => navigate("register")}
    >
      {t("auth.toRegister")}
    </button>
  </div>
</div>
