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

✅ Done 2026-09-18 — `svelte-check` clean, `test:e2e` 5/5, `dist/` 464K → 132K.

10. ~~**Refresh-token storage** (M2)~~ — `vault:*` IPC + `safeStorage`;
    web fallback + migration documented in `02-architecture.md`.
11. ~~**Split `assistant.store.svelte.ts`**~~ — `providers` / `sessions` /
    `chat` (voice stays in chat, documented); `sessionsError` dropped.
12. ~~**Router upgrade**~~ — `#/settings[/section]`, `syncAuthRoute`,
    unknown-hash fallback; covered by the deep-link test.
13. ~~**Decide the gestures story**~~ — cut (toggle, section, store, i18n);
    returns with the pipeline. Decision #14.
14. ~~**Decide the Supabase story**~~ — module + dep removed, design in
    `08-sync-design.md`. Decision #15.
15. ~~**Bundle diet**~~ — BounceSidebar ported to Svelte (WAAPI arc,
    reduced-motion aware); `react`/`react-dom`/`motion` deleted.

## P3 — Polish & process

✅ Done 2026-09-18 — `npm run verify` (check + lint + format + 25 unit),
contract 7/7, `cargo test` 48/48, `test:e2e:all` 6/6.

16. ~~**Unit tests for pure logic**~~ — vitest, colocated `src/**/*.test.ts`:
    `api()` mapping, `toEvent`, `timeOf`/`fmtK`/`contextPct`, `defaultModelFor`,
    i18n parity, router parse/guard. (`.svelte.ts` stores importable via the
    svelte plugin; `timeOf`/`fmtK` extracted to `lib/format.ts` to be so.)
17. ~~**Contract test renderer↔sidecar**~~ — `src/sidecar.contract.test.ts`
    spawns its own sidecar (temp db, free port) and pins protocol + shapes +
    envelopes + deterministic errors. Hermetic: `npm run test:contract`.
18. ~~**CI workflow**~~ — `.github/workflows/ci.yml`: `verify` job (verify +
    cargo test + contract) and `fast-e2e` job, both with rust-cache.
19. ~~**Lint/format baseline**~~ — eslint flat (js + ts + svelte + prettier
    compat) + prettier `--write` baseline enforced by `format:check`; the two
    `any`s typed (`Options.body: unknown`, `SpeechRecognitionLike`);
    one `verify` script. Baseline fixes: CJS `require` allowance for
    `electron/*.cjs`, dropped dead `vite` handle, `void dprTick` tracking read.
20. ~~**Small UI correctness**~~ — theme-aware Electron `backgroundColor`;
    dot re-seats on `items` change by construction; `SelectMenu` options keep
    `aria-selected` only (TopBar's `menuitemradio` + `aria-checked` was
    already correct).
21. **Docs reconciliation**: top-level `/docs/` still says "no code yet" and
    prescribes Vercel/Render/Neon + SvelteKit + MediaPipe. Either update it to
    the sidecar reality or mark it superseded by `frontend/docs/` + sidecar
    README, or drift will keep compounding.
