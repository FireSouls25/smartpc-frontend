# 04 — Correctness audit (verified 2026-09-18)

Method: full read of renderer, electron, scripts, tests, supabase; `svelte-check`
(0 errors), `tsc --noEmit` (clean), `tsc --listFiles` proof for item 1.
Severity: **H**igh / **M**edium / **L**ow.

## H1 — `bounce-sidebar.tsx` has zero type coverage
`tsconfig.json` includes `src/**/*.ts` which does **not** match `.tsx`
(verified: `tsc --listFiles` shows no `.tsx` in the program; `svelte-check`
also ignores it). The React island, its `motion` usage and its props contract
with `SettingsPage.svelte` are unchecked — a breaking prop rename compiles
green. `tests/` and `scripts/` are likewise outside the program.
Fix: `"include": ["src/**/*.ts", "src/**/*.tsx", "tests/**/*.ts", ...]`
(or `src/**/*`). — `tsconfig.json:13`

## H2 — E2E suite is not runnable as-is (no harness wiring)
`playwright.config.ts` has no `webServer`; nothing starts vite or the sidecar.
Specs hardcode `APP=http://127.0.0.1:5199` while `vite.config.ts` serves `:5173`
and no npm script documents/serves `:5199`. `package.json` has no `test*`
script at all. A fresh checkout cannot run tests without reverse-engineering
three terminals (vite `--port 5199`, sidecar `--port 18081`, then playwright).
See `05-testing.md`.

## M1 — No request timeout/abort in `lib/api.ts`
`fetch` has no `AbortSignal.timeout`; a hung sidecar leaves `send()` stuck
with `orb === "thinking"`, which also blocks all further sends
(`assistant.store.svelte.ts:226`) and all voice input (`:272`). One wedged
request bricks the assistant pane until reload.
Fix: `AbortSignal.timeout(…)` + `orb` reset is already in `finally` — just add
the signal and a timeout error string.

## M2 — Refresh token in `localStorage` (XSS-readable)
`auth.store.svelte.ts:3-25`. Any injected script exfiltrates the long-lived
credential. The code comments admit it ("moves to safeStorage", "follows the
same path") but nothing does. Electron has `safeStorage`; web fallback needs
a short-lived in-memory token + silent refresh design. No XSS vector is known
today (no `innerHTML`, deps minimal), so M, not H.

## M3 — `ReactIsland.svelte` mount is fragile
`host` is a plain `let`, not `$state` (`ReactIsland.svelte:18`). The first
`$effect` returns early when `host` is null and, if `bind:this` hasn't been
assigned before effects run, never re-runs → blank sidebar with no error.
Works today by mount ordering luck. Second `$effect` re-renders on *every*
reactive change it touches (`props` object identity changes each parent render
→ `createElement` churn). Fix: `let host = $state<…>(null)`, single effect
that creates the root once and renders on `component`/`props` change.

## M4 — Chat list keyed by index
`CenterPanel.svelte:48` — `{#each assistant.messages as m, i (i)}`. `send()`
replaces the whole array (`store:245-251`); index keys make Svelte patch
bubbles in place, so per-message state (e.g. the `steps` chips on the last
bubble) can stick to the wrong node across updates. Key by message id once the
API exposes one (detail returns `SessionMessage.id`), or key by `i + role + len`.

## M5 — Opening a session mutates global provider selection
`openSession` → `selectProvider(session.provider, session.model)`
(`store:207`). Browsing history POSTs `/v1/ai/select` and surfaces
`selectError` ("no detectado") for sessions whose provider is now offline —
reading history shouldn't fail or reconfigure the composer. Split "adopt"
(explicit user action) from "view".

## M6 — `saveKey` triple round-trip + redundant status fetch
`saveKey` → `refreshKeyStatus` → `loadProviders` (which itself calls
`refreshKeyStatus` again) → `selectProvider` (`store:140-156`). Works, but 4
sequential requests where the `SaveKeyResponse` (`models`, `suggested_model`)
already carries the data. Also `deleteKey` closes the modal even on provider
mismatch. Low risk, real latency on slow machines.

## L1 — Duplicate scroll-guard logic
`auth.user` redirect lives in `Shell.svelte:26-31` while `AuthPage` also
navigates on `auth.user` (`AuthPage.svelte:17-19`). Benign today (both agree),
but two sources of routing truth; a settings route would need a third.
Consider one `requireAuth` guard in the router.

## L2 — Settings view is not a route
`Shell.svelte:15` local `view` state: not deep-linkable, resets on reload,
Back button exits to main instead of browser back. Fine for 4 sections;
revisit when sections grow.

## L3 — Stale Electron `backgroundColor`
`main.cjs:73` hardcodes `#eff1f5` (old Catppuccin mantle); theme is now
`#e9e9ec` light / `#131315` dark → wrong flash color on cold start.

## L4 — Bounce dot misaligns on language switch
`bounce-sidebar.tsx:59-80` snaps the dot once on mount (`[]` deps). Switching
es↔en changes label heights → dot sits between rows until section change.
Include `items` (or a resize observer) in the snap deps.

## L5 — `probe.spec.ts` is a debug leftover
Console-dump + `/tmp/*.png` screenshots, no assertions. Either promote it to a
boot smoke test (assert `#app` hydrates, fail on `pageerror`) or delete it.

## Explicitly checked, no issue
- `api()` 204 handling, error envelope mapping, gate+user token headers.
- `restore()` failure clears both tokens; `register`→`login` chaining; `logout`
  best-effort server call then local wipe.
- Tool-role turns filtered from chat in both `openSession` and `send`.
- `openSession`/`deleteSession` keep `activeSessionId` consistent; deleting the
  active session resets to greeting.
- i18n: `en` is `Record<I18nKey,string>` — missing keys fail compile. `t()` falls
  back to the key (never throws).
- Theme: no FOUC for dark (inline script), `auto` tracks OS changes, persists.
- Voice: guards `thinking` state, stops recognition on send, surfaces
  unsupported/error states; no mic stays hot.
- Sidecar spawn: READY-line regex accumulates split chunks correctly; 20 s
  timeout with error dialog; kills child on quit.
