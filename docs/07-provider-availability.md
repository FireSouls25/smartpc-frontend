# 07 — Provider availability watch + auto-start

Implemented 2026-09-18. Problem: if `ollama serve` wasn't running at boot,
the UI showed "not detected" forever — no re-check, no recovery path.

## Mechanism

```
store poll (every PROVIDER_POLL_MS = 3000, skipped when tab hidden)
  → GET /v1/ai/providers (sidecar token only, no user needed)
    → 3 concurrent probes, each capped at 3 s (probe_get timeout)
  → silent store update: no loading flag, no error surface,
    no selection re-read (initial load still reads server selection;
    user changes persist via selectProvider)
  → UI derives everything: activeAvailable(), needServer chip,
    ProviderStart button visibility

"Start <id>" button (only when startable && offline && installed !== false)
  → POST /v1/ai/providers/{id}/start (45 s client timeout)
    → already answering? { ok, already_running: true }
    → binary on PATH? else 404 not_installed
    → spawn detached + wait ≤ ~24×(0.5 s + probe) → { ok } |
      502 start_timeout (child keeps running; the poll picks it up)
  → immediate silent refresh so the UI flips without waiting for the tick
```

## Contract (additive — no PROTOCOL bump)

- `ProviderInfo` gains `startable: boolean`, `installed: boolean | null`
  (`true` when answering; PATH check when a startable server is down;
  `null` where the concept doesn't apply — llama.cpp needs a model path,
  opencode is key-based).
- `POST /v1/ai/providers/{id}/start` → `{ ok, already_running }`.
  Error codes: `not_startable` (400), `not_installed` (404),
  `start_failed` (500), `start_timeout` (502).

## Cross-platform (Windows / Linux / macOS)

- Lookup is a `PATH` scan in `ai/supervise.rs::find_on_path` — no shell,
  no per-OS paths. Windows also tries `.exe`/`.bat`/`.cmd`; Unix checks
  the executable bit. Stock Ollama installers put `ollama` on PATH on all
  three OSes; if not, the UI shows the notInstalled hint instead of the button.
- Spawn: `Command` with stdio nulled. Unix children reparent past the sidecar;
  Windows sets `CREATE_NO_WINDOW` (`0x0800_0000`) so no console popup appears.
- The child **outlives the sidecar by design**: `ollama serve` is a user-level
  server (closing its terminal doesn't stop it either). Verified live:
  endpoint launched `ollama serve` and it answered `/api/tags`.
- Start/timeout events go to `diagnostics::push`, so they show up in
  Settings → AI → diagnostics like every other backend event.

## Missing-model auto-pull (2026-09-22)

Fresh installs ask for `llama3.1` while only e.g. `gemma4` is downloaded —
every turn (typed or voice-driven) failed `model not found`. Now `chat`
and `run` call `ai/ollama.rs::ensure_model_present` for ollama targets:
exact or tag-implied match (`llama3.1` ↔ `llama3.1:8b`) skips; otherwise one
`ollama pull` runs (per-model lock, 30 min cap, progress in diagnostics),
then the turn proceeds. A down server is not a pull failure (the turn
reports `unreachable` as before). Pull failure → 502 `model_pull_failed`
naming the installed models so the user can pick one in Settings → AI model.

## Deliberate non-goals

- No auto-switch of the active provider when another comes online — browsing
  availability must never yank the composer's selection (see audit M5).
- llama.cpp is not startable (model path + flags unknowable); opencode needs
  no server. Only `ollama` maps in `start_spec`.
- The 3 s poll is renderer-driven, not a server push: each tick is one HTTP
  round-trip; refused connections fail fast, so idle cost is ~3 small probes.
  In-flight guard prevents overlap; hidden tabs skip ticks.

## Files

- Sidecar: `src/native/src/ai/supervise.rs` (new, 4 unit tests),
  `ai/routes.rs` (`probe` fields + `start_provider`), `ai/mod.rs`,
  `api.rs` (route).
- Renderer: `assistant.api.ts` (`ProviderInfo`, `startProvider`),
  `assistant.store.svelte.ts` (silent `loadProviders`, `startProviderWatch`/
  `stopProviderWatch`, `startProvider`), `ProviderStart.svelte` (new),
  `CenterPanel.svelte` + `SettingsPage.svelte` wiring, `Shell.svelte`
  (watch lifecycle), `lib/api.ts` (`timeoutMs` option, TimeoutError → 504),
  `i18n/es.ts` + `en.ts` (5 keys each).
