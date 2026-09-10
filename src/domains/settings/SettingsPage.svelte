<script lang="ts">
  import ReactIsland from "../../shared/ReactIsland.svelte";
  import { BounceSidebar } from "../../components/ui/bounce-sidebar";
  import { auth } from "../auth/auth.store.svelte";
  import { navigate } from "../../app/router.svelte";
  import { t, getLang, setLang, type Lang, type I18nKey } from "../../lib/i18n.svelte";
  import { getTheme, setTheme, type Theme } from "../../lib/theme.svelte";
  import { assistant } from "../assistant/assistant.store.svelte";
  import SelectMenu from "../../shared/SelectMenu.svelte";

  let { onBack }: { onBack: () => void } = $props();

  const sections = ["general", "gestures", "ai", "account"] as const;
  let section = $state(0);

  const items = (): string[] =>
    sections.map((s) => t(`settings.${s}` as I18nKey));

  async function logout(): Promise<void> {
    await auth.logout();
    navigate("login");
  }

  async function remove(): Promise<void> {
    if (!window.confirm(t("auth.deleteAsk"))) return;
    await auth.deleteAccount();
    navigate("register");
  }
</script>

<div class="flex flex-col gap-4">
  <div class="flex items-center gap-3">
    <button class="icon-btn" onclick={onBack} aria-label={t("settings.back")}>
      <svg
        viewBox="0 0 24 24"
        class="h-5 w-5"
        fill="none"
        stroke="currentColor"
        stroke-width="1.8"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <path d="M19 12H5M12 19l-7-7 7-7" />
      </svg>
      <span class="tip">{t("settings.back")}</span>
    </button>
    <h1 class="text-xl font-bold">{t("settings.title")}</h1>
  </div>

  <div class="card-xl flex flex-col gap-6 md:flex-row">
    <div class="shrink-0 md:w-52">
      <ReactIsland
        component={BounceSidebar}
        props={{
          items: items(),
          value: section,
          onChange: (i: number) => (section = i),
          dotColor: "#8839ef",
        }}
      />
    </div>

    <div class="min-w-0 flex-1">
      {#if section === 0}
        <div class="flex flex-col gap-4">
          <div>
            <p class="label">{t("common.theme")}</p>
            <div class="flex flex-wrap gap-2">
              {#each (["auto", "light", "dark"] as Theme[]) as v (v)}
                <button
                  class="chip"
                  style={getTheme() === v
                    ? "border-color: var(--accent); color: var(--fg);"
                    : ""}
                  onclick={() => setTheme(v)}
                  aria-pressed={getTheme() === v}
                >
                  {v === "auto"
                    ? t("common.auto")
                    : v === "light"
                      ? t("common.light")
                      : t("common.dark")}
                </button>
              {/each}
            </div>
          </div>
          <div>
            <p class="label">Language / Idioma</p>
            <div class="flex flex-wrap gap-2">
              {#each (["es", "en"] as Lang[]) as v (v)}
                <button
                  class="chip"
                  style={getLang() === v
                    ? "border-color: var(--accent); color: var(--fg);"
                    : ""}
                  onclick={() => setLang(v)}
                  aria-pressed={getLang() === v}
                >
                  {v === "es" ? "Español" : "English"}
                </button>
              {/each}
            </div>
          </div>
        </div>
      {:else if section === 1}
        <div class="flex items-center justify-between gap-4">
          <div>
            <p class="text-sm font-bold">{t("settings.gesturesOn")}</p>
            <p class="muted mt-0.5 text-xs">{t("settings.gesturesHint")}</p>
          </div>
          <button
            class="switch"
            role="switch"
            aria-checked={assistant.gesturesOn}
            aria-label={t("settings.gesturesOn")}
            onclick={() => assistant.toggleGestures()}
          ></button>
        </div>
      {:else if section === 2}
        <div class="flex flex-col gap-4">
          <p class="muted text-sm">{t("settings.providerNote")}</p>
          {#if assistant.providersError}
            <p class="error-box">{assistant.providersError}</p>
          {/if}
          <div>
            <p class="label">{t("chat.provider")}</p>
            <SelectMenu
              label={t("chat.provider")}
              value={assistant.activeProvider}
              options={assistant.providers.map((p) => ({
                value: p.id,
                label: p.id,
                disabled: !p.available,
                hint: p.available ? undefined : t("providers.offline"),
              }))}
              onChange={(v) => void assistant.selectProvider(v)}
            />
          </div>
          <div>
            <p class="label">{t("chat.model")}</p>
            <SelectMenu
              label={t("chat.model")}
              value={assistant.activeModel}
              options={assistant.activeModels().map((m) => ({ value: m, label: m }))}
              onChange={(v) => void assistant.selectModel(v)}
            />
          </div>
          {#if assistant.selectError}
            <p class="error-box">{assistant.selectError}</p>
          {/if}
        </div>
      {:else}
        <div class="flex flex-col gap-3">
          <div>
            <p class="label">{t("auth.email")}</p>
            <p class="text-sm font-semibold">{auth.user?.email}</p>
            <p class="faint mt-0.5 text-xs">{t("settings.accountNote")}</p>
          </div>
          <div class="flex flex-wrap gap-2">
            <button class="btn btn-ghost" onclick={logout}>
              {t("auth.logout")}
            </button>
            <button class="btn btn-danger" onclick={remove}>
              {t("auth.delete")}
            </button>
          </div>
        </div>
      {/if}
    </div>
  </div>
</div>
