<script lang="ts">
  import Logo from "../shared/Logo.svelte";
  import { getLang, setLang, t, type Lang } from "../lib/i18n.svelte";
  import { getTheme, setTheme, type Theme } from "../lib/theme.svelte";

  let { onSettings }: { onSettings: () => void } = $props();
  let open: "theme" | "lang" | null = $state(null);

  function toggle(which: "theme" | "lang"): void {
    open = open === which ? null : which;
  }

  const themes: Theme[] = ["auto", "light", "dark"];
  const langs: Lang[] = ["es", "en"];
  const themeLabel = (v: Theme): string =>
    v === "auto"
      ? t("common.auto")
      : v === "light"
        ? t("common.light")
        : t("common.dark");
</script>

<svelte:window
  onclick={(e) => {
    const el = e.target as HTMLElement | null;
    if (el?.closest?.("[data-menu]")) return;
    open = null;
  }}
/>

<header class="flex shrink-0 items-center justify-between gap-3 py-4">
  <div class="flex items-center gap-2.5">
    <Logo />
    <div>
      <p class="font-bold leading-tight">Smart PC</p>
      <p class="faint text-[11px] leading-tight">{t("brand.tagline")}</p>
    </div>
  </div>

  <div class="flex items-center gap-2">
    <div class="relative" data-menu>
      <button
        class="icon-btn"
        onclick={() => toggle("theme")}
        aria-label={t("common.theme")}
      >
        {#if getTheme() === "dark"}
          <svg
            viewBox="0 0 24 24"
            class="h-5 w-5"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z" />
          </svg>
        {:else if getTheme() === "light"}
          <svg
            viewBox="0 0 24 24"
            class="h-5 w-5"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
            stroke-linecap="round"
          >
            <circle cx="12" cy="12" r="4" />
            <path
              d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"
            />
          </svg>
        {:else}
          <svg
            viewBox="0 0 24 24"
            class="h-5 w-5"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <rect x="2" y="4" width="20" height="13" rx="2" />
            <path d="M8 21h8M12 17v4" />
          </svg>
        {/if}
        <span class="tip">{t("common.theme")}</span>
      </button>
      {#if open === "theme"}
        <div class="dropdown" role="menu">
          {#each themes as v (v)}
            <button
              role="menuitemradio"
              aria-checked={getTheme() === v}
              onclick={() => {
                setTheme(v);
                open = null;
              }}
            >
              <span
                class="dot"
                style="background: {getTheme() === v
                  ? 'var(--accent)'
                  : 'var(--border-strong)'};"
              ></span>
              {themeLabel(v)}
            </button>
          {/each}
        </div>
      {/if}
    </div>

    <div class="relative" data-menu>
      <button
        class="icon-btn"
        onclick={() => toggle("lang")}
        aria-label="Language"
      >
        <svg
          viewBox="0 0 24 24"
          class="h-5 w-5"
          fill="none"
          stroke="currentColor"
          stroke-width="1.8"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <circle cx="12" cy="12" r="10" />
          <path
            d="M2 12h20M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"
          />
        </svg>
        <span class="tip">{getLang().toUpperCase()}</span>
      </button>
      {#if open === "lang"}
        <div class="dropdown" role="menu">
          {#each langs as v (v)}
            <button
              role="menuitemradio"
              aria-checked={getLang() === v}
              onclick={() => {
                setLang(v);
                open = null;
              }}
            >
              <span
                class="dot"
                style="background: {getLang() === v
                  ? 'var(--accent)'
                  : 'var(--border-strong)'};"
              ></span>
              {v === "es" ? "Español" : "English"}
            </button>
          {/each}
        </div>
      {/if}
    </div>

    <button
      class="icon-btn"
      onclick={onSettings}
      aria-label={t("topbar.settings")}
    >
      <svg
        viewBox="0 0 24 24"
        class="h-5 w-5"
        fill="none"
        stroke="currentColor"
        stroke-width="1.8"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <circle cx="12" cy="12" r="3" />
        <path
          d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"
        />
      </svg>
      <span class="tip">{t("topbar.settings")}</span>
    </button>
  </div>
</header>
