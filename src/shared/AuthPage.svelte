<script lang="ts">
  import { auth } from "../domains/auth/auth.store.svelte";
  import { t } from "../lib/i18n.svelte";
  import { navigate } from "../app/router.svelte";
  import Logo from "./Logo.svelte";
  import LangTheme from "./LangTheme.svelte";

  let { mode }: { mode: "login" | "register" } = $props();

  let email = $state("");
  let password = $state("");
  let error = $state("");
  let busy = $state(false);

  const isLogin = () => mode === "login";

  $effect(() => {
    if (auth.user) navigate("");
  });

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (busy) return;
    busy = true;
    error = "";
    try {
      if (isLogin()) await auth.login(email, password);
      else await auth.register(email, password);
      navigate("");
    } catch (err) {
      error = err instanceof Error ? err.message : "Error";
    } finally {
      busy = false;
    }
  }

  function switchMode() {
    error = "";
    navigate(isLogin() ? "register" : "login");
  }
</script>

<div class="dot-bg grid min-h-screen place-items-center p-4">
  <div class="absolute right-4 top-4"><LangTheme /></div>
  <div class="card-xl msg-in w-full max-w-md" style="padding: 2.25rem;">
    <div class="mb-6 flex items-center gap-3">
      <Logo />
      <div>
        <p class="text-lg font-bold">Smart PC</p>
        <p class="faint text-xs">{t("brand.tagline")}</p>
      </div>
    </div>
    <h1 class="mb-1 text-2xl font-bold">
      {isLogin() ? t("auth.loginTitle") : t("auth.registerTitle")}
    </h1>
    <p class="muted mb-5 text-sm">
      {isLogin() ? t("auth.toRegister") : t("auth.toLogin")}
    </p>
    <form onsubmit={submit} class="flex flex-col gap-3">
      <div>
        <label class="label" for="auth-email">{t("auth.email")}</label>
        <input
          id="auth-email"
          class="field"
          style="border-radius: 1rem; padding: 0.75rem 1rem;"
          type="email"
          required
          autocomplete="email"
          bind:value={email}
        />
      </div>
      <div>
        <label class="label" for="auth-password">{t("auth.password")}</label>
        <input
          id="auth-password"
          class="field"
          style="border-radius: 1rem; padding: 0.75rem 1rem;"
          type="password"
          required
          minlength={isLogin() ? undefined : 8}
          autocomplete={isLogin() ? "current-password" : "new-password"}
          bind:value={password}
        />
      </div>
      {#if error}<p class="error-box">{error}</p>{/if}
      <button
        class="btn btn-primary mt-1"
        style="border-radius: 1rem; padding: 0.8rem 1.25rem;"
        type="submit"
        disabled={busy}
      >
        {busy
          ? t("common.loading")
          : isLogin()
            ? t("auth.loginBtn")
            : t("auth.registerBtn")}
      </button>
    </form>
    <div class="mt-5 flex items-center gap-3">
      <span class="h-px flex-1" style="background: var(--border);"></span>
      <span class="faint text-xs">{t("auth.or")}</span>
      <span class="h-px flex-1" style="background: var(--border);"></span>
    </div>
    <button
      class="mx-auto mt-3 block text-sm font-semibold transition hover:brightness-110"
      style="color: var(--accent);"
      onclick={switchMode}
    >
      {isLogin() ? t("auth.registerBtn") : t("auth.loginBtn")} →
    </button>
  </div>
</div>
