# 04 — Correctness audit (verified 2026-09-18)

Method: full read of renderer, electron, scripts, tests, supabase; `svelte-check`
(0 errors), `tsc --noEmit` (clean), `tsc --listFiles` proof for item 1.
Severity: **H**igh / **M**edium / **L**ow.

## H1 — `bounce-sidebar.tsx` has zero type coverage
✅ Fixed 2026-09-18: `tsconfig.json` now includes `src/**/*.tsx`
(plus `tests/**/*.ts`), verified in the `tsc --listFiles` program; clean.

## H2 — E2E suite is not runnable as-is (no harness wiring)
✅ Fixed 2026-09-18: `webServer` boots sidecar (temp db) + vite from
`npm run test:e2e` (`test:e2e:all` incl. `@slow`); ports live in one place
(`playwright.config.ts` env, shared via `tests/helpers.ts`); minimal CI
workflow runs the fast path. See `05-testing.md`.

## M1 — No request timeout/abort in `lib/api.ts`
✅ Fixed 2026-09-18: `api()` defaults to a 30 s abort budget
(`timeoutMs: null` opts out, a number overrides); chat 5 min, run 10 min,
key-save 90 s, provider-start 45 s, `/health` 10 s. Hung sidecar now surfaces
a localized `chat.timeout` error instead of wedging `orb` at `thinking`.

## M2 — Refresh token in `localStorage` (XSS-readable)
`auth.store.svelte.ts:3-25`. Any injected script exfiltrates the long-lived
credential. The code comments admit it ("moves to safeStorage", "follows the
same path") but nothing does. Electron has `safeStorage`; web fallback needs
a short-lived in-memory token + silent refresh design. No XSS vector is known
today (no `innerHTML`, deps minimal), so M, not H.

## M3 — `ReactIsland.svelte` mount is fragile
✅ Fixed 2026-09-18: `host`/`root` are `$state`, root is created once per host
with cleanup, and re-renders are signature-gated (functions compared by source,
not identity — documented in the file header).

## M4 — Chat list keyed by index
✅ Fixed 2026-09-18: `ChatMsg.id` carries the server message id; the each-block
keys on `m.id ?? local-${i}`, steps key by index.

## M5 — Opening a session mutates global provider selection
✅ Fixed 2026-09-18: `openSession` is view-only; it records `sessionCombo`
and CenterPanel offers an explicit "use session's model" chip (cleared on
send/newChat/adopt). Browsing history no longer POSTs `/v1/ai/select`.

## M6 — `saveKey` triple round-trip + redundant status fetch
✅ Fixed 2026-09-18: `saveKey` folds `SaveKeyResponse.models` into local state
(6 requests → 2: save + select persist, with a silent-refresh fallback if the
catalog entry is missing); `deleteKey` flips `keyStatus` locally.

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
✅ Fixed 2026-09-18: deleted, replaced by `tests/smoke.spec.ts` (asserts
`#app` hydrates with zero `pageerror`).

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
