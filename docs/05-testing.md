# 05 — Testing (current state)

## Inventory

| Spec | What it does | Needs |
|------|--------------|-------|
| `tests/smoke.spec.ts` | Boot: `#app` hydrates, zero `pageerror`. No auth. | harness only |
| `tests/layout.spec.ts` | (1) viewport-lock. (2) **view-only sessions**: seeds a session via API, opens it, asserts no `POST /v1/ai/select` + adopt chip appears. (3) **settings deep-link**: gear → `#/settings`, sidebar hop → `#/settings/1`, unknown hash → home, Back → shell. (4) `@slow` real send. | sidecar; a real Ollama model for (2) (else `test.skip`) |
| `tests/ai-keys.spec.ts` | Selects `opencode` → key modal appears → bogus key → asserts `/rejected\|rechazado/i` within 60 s. | sidecar, no real key |
| `tests/helpers.ts` | Shared `APP`/`SIDECAR`/`GATE` (env-driven, no hardcoded ports) + `seedUser()` (fresh user + rotating refresh token per test). | — |

`playwright.config.ts`: `testDir ./tests`, 120 s timeout,
`channel: chromium-headless-shell`, 1600×900, plus `webServer` (below).

## How to run

```bash
npm run test:e2e      # fast path: everything except @slow (default, CI)
npm run test:e2e:all  # full suite incl. real model inference (needs Ollama)
```

No manual terminals: the config boots the sidecar (`cargo run`, temp db per
run in `os.tmpdir()`) and vite on fixed ports, with `VITE_API_URL` /
`VITE_SIDECAR_TOKEN` injected so the app under test always hits the harness
sidecar. Locally an already-running pair is reused (`reuseExistingServer`);
CI starts fresh. Ports/tokens overridable per run:

```bash
APP_PORT=5200 SIDECAR_PORT=18082 npm run test:e2e
```

Verified 2026-09-18: `test:e2e` 5/5 in ~18 s, `test:e2e:all` 4/4 earlier
(~27 s, incl. real Ollama inference + session persistence).

`reducedMotion: "reduce"` is pinned in the Playwright config so WAAPI hops
and the orb render deterministically (screenshots included).

## Remaining gaps

1. **No unit tests.** Zero coverage for: `api()` error mapping, auth token
   rotation handling, `toEvent` status normalization, `timeOf`, `fmtK`/`ctxPct`,
   provider/model defaulting (`defaultModelFor`), i18n runtime parity,
   `SelectMenu` keyboard/escape behavior. (P3 #16: vitest.)
2. **`.tsx` island untested and still untypechecked** (`tsconfig.json` covers
   `src/**/*.ts` + `tests/**/*.ts`, not `.tsx`). A vitest + testing-library
   spec or a port to Svelte would close it. (P1 #4, P2 #15.)
3. **Sidecar contract drift has one guard only**: the protocol banner.
   No schema test asserts renderer types (`SessionSummary`, `RunResponse`, …)
   still match sidecar JSON. (P3 #17.)
4. Rust sidecar `cargo test` exists (48 tests) but no workflow runs it yet.
   (P3 #18; the new `e2e.yml` runs the fast Playwright path only.)
