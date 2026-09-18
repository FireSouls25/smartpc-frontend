# 05 — Testing (current state)

## Inventory

| Spec | What it does | Needs |
|------|--------------|-------|
| `tests/layout.spec.ts` | (1) viewport-lock: page never scrolls, chat pane absorbs 60 injected bubbles. (2) real send: picks first Ollama model, sends "Reply with exactly: hola-test", asserts assistant bubble + session row. Timeout 6 min (cold VRAM). | sidecar `:18081`, seeded user, **real Ollama model** (else `test.skip`) |
| `tests/ai-keys.spec.ts` | Selects `opencode` provider → key modal appears → bogus key `badkey12` → asserts `/rejected\|rechazado/i` within 60 s. | sidecar `:18081`, no real key |
| `tests/probe.spec.ts` | Debug leftover: dumps console logs, body length, `/tmp/probe-boot.png`. No assertions. | vite `:5199` |

`playwright.config.ts`: `testDir ./tests`, 120 s timeout,
`channel: chromium-headless-shell`, 1600×900. No projects, no `webServer`,
no retries, no reporters beyond default.

## How to run today (undocumented, reconstructed)

```bash
# terminal 1 — sidecar matching .env + specs
cargo run --manifest-path src/native/Cargo.toml -- \
  --port 18081 --token dev-token-min-16-chars --db /tmp/test.db
# terminal 2 — vite on the port the specs expect (NOT the 5173 default)
npx vite --port 5199
# terminal 3
npx playwright test
```

Each spec's `beforeEach` registers a fresh user via HTTP and injects its
rotating refresh token into `localStorage` (rotation means tokens can't be
shared across tests — comment in `layout.spec.ts:10-11`).

## Gaps

1. **No `test:e2e` script, no `webServer` wiring** — three manual terminals;
   port `5199` appears only in spec files (grep-verified), so discovery is by
   reading source. CI impossible as-is. (H2 in audit.)
2. **No unit tests.** Zero coverage for: `api()` error mapping, auth token
   rotation handling, `toEvent` status normalization, `timeOf`, `fmtK`/`ctxPct`,
   provider/model defaulting (`defaultModelFor`), i18n key parity (only
   enforced es→en by types, not at runtime), `SelectMenu` keyboard/escape
   behavior.
3. **No component tests.** `CenterPanel` scroll-follow, `ApiKeyModal`
   focus/escape/backdrop, `AuthPage` busy/error states — all untested except
   through full E2E.
4. **`.tsx` island untested and untypechecked** (H1). A vitest + testing-library
   or even a `tsc` include fix would catch prop drift.
5. **Sidecar contract drift has one guard only**: the protocol banner.
   No schema test asserts renderer types (`SessionSummary`, `RunResponse`, …)
   still match sidecar JSON.
6. **Screenshots to `/tmp/`** (`layout-*.png`, `probe-boot.png`) — invisible in
   CI; should be `test-results/` attachments or removed.
7. **No CI workflow** (no `.github/` anywhere in the repo).
8. Rust sidecar has `cargo test` (per its README) — not evaluated here; the
   frontend never runs it as part of any flow.
