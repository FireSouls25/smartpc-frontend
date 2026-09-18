# 05 — Testing (current state)

## Layers

| Layer      | Runner     | Command                 | What                                   |
| ---------- | ---------- | ----------------------- | -------------------------------------- |
| Unit       | vitest     | `npm run test:unit`     | 25 tests, colocated `src/**/*.test.ts` |
| Contract   | vitest     | `npm run test:contract` | 7 tests vs self-spawned sidecar        |
| E2E fast   | Playwright | `npm run test:e2e`      | 5 tests, `@slow` excluded (CI default) |
| E2E full   | Playwright | `npm run test:e2e:all`  | 6 tests incl. real inference           |
| Sidecar    | cargo      | `cargo test …/native/`  | 48 Rust tests                          |
| All static | npm        | `npm run verify`        | check + lint + format:check + unit     |

CI (`.github/workflows/ci.yml`): `verify` job (verify + cargo test +
contract) and `fast-e2e` job, both with rust-cache.

## Unit inventory (`src/**/*.test.ts`)

- `lib/api.test.ts` — `api()` 200/204/envelope/generic-error mapping, gate +
  bearer headers, TimeoutError → 504/`timeout`, transport passthrough.
  (Env pinned via `stubEnv` so a local `.env` can't move assertions.)
- `lib/format.test.ts` — `timeOf`/`fmtK`/`contextPct` (extracted from
  SessionsPane/EventsFeed into `lib/format.ts` to be importable).
- `lib/i18n.test.ts` — `en` covers exactly the `es` key contract, no empties.
- `domains/assistant/chat.test.ts` — `toEvent` normalization (imports the
  real store module; the svelte plugin compiles `.svelte.ts` under vitest).
- `domains/assistant/providers.test.ts` — `defaultModelFor` fallback chain.
- `app/router.test.ts` — route parsing, bad-section clamp, navigate/resync,
  `syncAuthRoute` truth table (fresh stubbed window per case).

## Contract inventory (`src/sidecar.contract.test.ts`)

Spawns `cargo run` on a free port with a temp db, waits for `/health`, kills
on teardown — hermetic, no manual terminals. Pins: protocol equality with the
renderer's own `SIDECAR_PROTOCOL`, provider catalog shapes (all 9 keys +
types), error envelopes (login 401, register 400, user-gate 401, sidecar-gate
401), deterministic `not_startable`. Nothing depends on installed models.

## E2E inventory (`tests/`)

| Spec                    | What it does                                                                                                                                                                                                                                                                     | Needs                                                   |
| ----------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------- |
| `tests/smoke.spec.ts`   | Boot: `#app` hydrates, zero `pageerror`. No auth.                                                                                                                                                                                                                                | harness only                                            |
| `tests/layout.spec.ts`  | (1) viewport-lock. (2) **view-only sessions**: seeds a session via API, opens it, asserts no `POST /v1/ai/select` + adopt chip appears. (3) **settings deep-link**: gear → `#/settings`, sidebar hop → `#/settings/1`, unknown hash → home, Back → shell. (4) `@slow` real send. | sidecar; a real Ollama model for (4) (else `test.skip`) |
| `tests/ai-keys.spec.ts` | Selects `opencode` → key modal appears → bogus key → asserts `/rejected\|rechazado/i` within 60 s.                                                                                                                                                                               | sidecar, no real key                                    |
| `tests/helpers.ts`      | Shared `APP`/`SIDECAR`/`GATE` (env-driven, no hardcoded ports) + `seedUser()` (fresh user + rotating refresh token per test) + `accessTokenFor()`.                                                                                                                               | —                                                       |

`playwright.config.ts`: `testDir ./tests`, 120 s timeout,
`channel: chromium-headless-shell`, 1600×900, `reducedMotion: reduce`
(deterministic WAAPI/orb), plus `webServer` (sidecar + vite, ports from one
place, `reuseExistingServer` locally).

Verified 2026-09-18: verify green, contract 7/7, `test:e2e` 5/5 in ~18 s,
`test:e2e:all` 6/6 in ~30 s (incl. real Ollama inference + persistence).

## Remaining gaps

1. **No component tests.** `CenterPanel` scroll-follow, `ApiKeyModal`
   focus/escape/backdrop, `AuthPage` busy/error states — covered only through
   E2E. (Vitest is in place; `@testing-library/svelte` would be next.)
2. **Contract pins shapes, not full schemas.** Renderer types
   (`SessionSummary`, `RunResponse`, …) vs sidecar JSON are asserted
   structurally per-route, not generated from a shared schema. A zod/shared-
   types step would close it if drift ever bites.
