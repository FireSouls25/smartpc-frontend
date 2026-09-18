# 06 — Proposed improvements

Ordered by value/effort. Items reference the audit (`04`) where applicable.
Nothing here is started — pick top-down.

## P0 — Test harness that runs (fixes H2, most of 05)

✅ Done 2026-09-18 — verified `test:e2e` 3/3 (~16 s), `test:e2e:all` 4/4
(~27 s, incl. real Ollama inference).

1. ~~**One-command E2E.**~~ `test:e2e` / `test:e2e:all` scripts +
   `webServer` in `playwright.config.ts` (sidecar with temp db + vite with
   injected `VITE_*`, ports from one place via `APP_PORT`/`SIDECAR_PORT`/
   `SIDECAR_TOKEN` env, shared with specs through `tests/helpers.ts`).
   Minimal CI workflow (`.github/workflows/e2e.yml`) runs the fast path.
2. ~~**Promote or delete `probe.spec.ts`**~~ Deleted; replaced by
   `tests/smoke.spec.ts` (hydrates + zero `pageerror`).
3. ~~**Split `layout.spec.ts` test 2.**~~ Inference test tagged `@slow`
   (`test:e2e` inverts it); shared seeding extracted to `seedUser()`;
   screenshots moved from `/tmp/` to `test-results/` (now gitignored).

## P1 — Correctness fixes (small, verified above)

✅ Done 2026-09-18 — `svelte-check` clean, `test:e2e` 3/3.

4. ~~**Type-check the `.tsx` island** (H1)~~ — include covers `src/**/*.tsx`;
   island verified clean via `tsc --listFiles`.
5. ~~**Request timeouts** (M1)~~ — 30 s default in `api()`; chat 5 min, run
   10 min, key-save 90 s, `/health` 10 s; localized `chat.timeout` in `send()`.
6. ~~**Harden `ReactIsland`** (M3)~~ — `$state` host/root, single root per
   host, signature-gated renders.
7. ~~**Key chat messages by id** (M4)~~ — `ChatMsg.id` from server, keyed
   each-block, steps keyed by index.
8. ~~**Stop mutating global selection on history browse** (M5)~~ —
   view-only `openSession` + `sessionCombo` + explicit adopt chip.
9. ~~**Untangle `saveKey` round-trips** (M6)~~ — 6 requests → 2, local
   `keyStatus` updates.

## P2 — Security & architecture

10. **Refresh-token storage** (M2): Electron `safeStorage` via a new preload
    channel (`vault:set/get/delete`); web keeps `localStorage` only as fallback
    with a documented risk note. Precondition for any multi-device story.
11. **Split `assistant.store.svelte.ts`.** 416-line god-store covering chat,
    providers, keys, sessions, voice, gestures. Natural seams:
    `chat.store` · `providers.store` (+keys) · `sessions.store` · `voice`.
    Do after P0 so the split is covered.
12. **Router upgrade.** One `requireAuth` guard (kills L1 duplication);
    settings as `#/settings[/section]` route (kills L2); unknown hashes →
    explicit fallback instead of silent home.
13. **Decide the gestures story.** The toggle is dead UI: either spike a
    MediaPipe renderer prototype behind a feature flag or remove the toggle
    until the pipeline exists. Same for the gestures settings section.
14. **Decide the Supabase story.** Either wire `pushPreferences/pullPreferences`
    to theme/lang/provider changes (smallest useful sync) or move `src/supabase/`
    out of the bundle path so it doesn't ship dead weight + attack surface
    (`@supabase/supabase-js` in prod deps).
15. **Bundle diet.** `react` + `react-dom` + `motion` serve one sidebar widget:
    port `BounceSidebar` to Svelte (animation is a one-axis dot tween) or
    replace with a Svelte-native list. Drops ~3 prod deps; kills the dual
    runtime and the M3 class of bugs permanently.

## P3 — Polish & process

16. **Unit tests for pure logic** (05.2): `api()` mapping, `toEvent`,
    `timeOf`/`fmtK`, `defaultModelFor`, i18n parity runtime check. Vitest is
    the natural fit (vite-native, no new runner philosophy).
17. **Contract test renderer↔sidecar**: snapshot `PROTOCOL` + key response
    shapes against a live sidecar in CI; fails fast on drift instead of the
    runtime banner.
18. **CI workflow**: extend `.github/workflows/e2e.yml` (currently fast
    Playwright path only) with `npm run check` + `tsc` + unit tests +
    `cargo test`. No full pipeline exists yet.
19. **Lint/format baseline**: eslint + prettier + `svelte-check` in one
    `npm run verify`; add the two `eslint-disable` comments' underlying rules
    (`no-explicit-any` in `api.ts:50`, `assistant.store:44`) by typing
    `body: unknown` and a minimal `SpeechRecognition` interface.
20. **Small UI correctness**: Electron `backgroundColor` per theme (L3);
    Bounce dot re-snap on `items` change (L4); `aria-checked` vs
    `aria-selected` double-attrs on `SelectMenu` options (both set —
    `listbox`/`option` roles want `aria-selected` only).
21. **Docs reconciliation**: top-level `/docs/` still says "no code yet" and
    prescribes Vercel/Render/Neon + SvelteKit + MediaPipe. Either update it to
    the sidecar reality or mark it superseded by `frontend/docs/` + sidecar
    README, or drift will keep compounding.
