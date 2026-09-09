<script lang="ts">
  import { t } from "../../lib/i18n.svelte";

  let listening = $state(false);

  const steps = [
    { key: "home.step1" as const, state: "done" },
    { key: "home.step2" as const, state: "running" },
    { key: "home.step3" as const, state: "pending" },
  ];

  function dot(state: string): string {
    if (state === "done") return "var(--success)";
    if (state === "running") return "var(--info)";
    return "var(--border-strong)";
  }
</script>

<h1 class="mb-1 text-2xl font-bold">{t("home.greeting")}</h1>
<p class="chip mb-5">{t("home.mockNote")}</p>

<div class="flex flex-col items-center gap-3 py-4">
  <button
    class="relative grid h-24 w-24 place-items-center rounded-full transition active:scale-95"
    style={listening
      ? "background: var(--danger);"
      : "background: var(--accent);"}
    onclick={() => (listening = !listening)}
    aria-pressed={listening}
    aria-label={listening ? t("home.listening") : t("home.listen")}
  >
    {#if listening}
      <span
        class="absolute inset-0 animate-ping rounded-full"
        style="background: var(--danger); opacity: 0.25;"
      ></span>
    {/if}
    <span class="flex items-end gap-1">
      <span
        class="w-1 rounded-full"
        style="height: 14px; background: var(--on-accent, #fff);"
      ></span>
      <span
        class="w-1 rounded-full"
        style="height: 24px; background: var(--on-accent, #fff);"
      ></span>
      <span
        class="w-1 rounded-full"
        style="height: 14px; background: var(--on-accent, #fff);"
      ></span>
    </span>
  </button>
  <p class="muted text-sm">
    {listening ? t("home.listening") : t("home.listen")}
  </p>
</div>

{#if listening}
  <div class="card mb-4">
    <p class="label">{t("home.transcript")}</p>
    <p class="text-lg">{t("home.transcriptMock")}</p>
    <p class="muted mt-2 text-sm">
      {t("home.understood")}: {t("home.understoodMock")}
    </p>
  </div>

  <div class="card mb-4">
    <p class="mb-3 font-bold">{t("home.plan")}</p>
    <ol class="flex flex-col gap-2">
      {#each steps as s (s.key)}
        <li
          class="flex items-center gap-2.5 rounded-lg p-2"
          style="background: var(--bg);"
        >
          <span class="dot" style="background: {dot(s.state)};"></span>
          <span class="text-sm">{t(s.key)}</span>
        </li>
      {/each}
    </ol>
    <div class="mt-3 flex flex-wrap gap-2">
      <button class="btn btn-primary" type="button">{t("home.confirm")}</button>
      <button class="btn btn-ghost" type="button">{t("home.edit")}</button>
      <button class="btn btn-danger" type="button">{t("home.stop")}</button>
    </div>
  </div>
{/if}

<div class="card">
  <p class="mb-2 font-bold">{t("home.timeline")}</p>
  <ul class="flex flex-col gap-2 text-sm">
    <li class="flex items-center gap-2">
      <span class="dot" style="background: var(--success);"></span>{t("home.t1")}
    </li>
    <li class="flex items-center gap-2">
      <span class="dot" style="background: var(--info);"></span>{t("home.t2")}
    </li>
  </ul>
</div>
