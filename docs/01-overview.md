# 01 — Overview

Smart PC is a **local-first desktop assistant**: a Svelte 5 UI in Electron,
driven by a Rust sidecar (`src/native/`, binary `smartpc-native`) that owns
auth, chat sessions, provider routing and OS actions over loopback HTTP.
The renderer never touches the OS directly.

## Runtime modes

| Mode            | UI              | Sidecar url+token from                                                               | Command                                                |
| --------------- | --------------- | ------------------------------------------------------------------------------------ | ------------------------------------------------------ |
| Electron (real) | desktop window  | preload bridge (`window.smartpc.sidecar`, per-launch random token, OS-assigned port) | `npm run dev:electron` (cargo build + vite + electron) |
| Web dev         | browser `:5173` | `VITE_API_URL` / `VITE_SIDECAR_TOKEN` (`.env` → `:18081`)                            | `npm run dev` + sidecar manually                       |

Packaged Electron loads static `dist/` (`npm run build`); main resolves the
sidecar from `resources/bin` instead of `src/native/target/debug`.

## Repo map (renderer-relevant)

```
src/
  main.ts                 entry: theme.css + mount App
  app/                    App.svelte (boot+restore+route guard) · Shell.svelte
                          TopBar.svelte · router.svelte.ts (hash routes + auth guard)
  domains/
    auth/                 LoginPage · RegisterPage · auth.api · auth.store
                          (vault-first refresh tokens, localStorage fallback)
    assistant/            CenterPanel (orb+chat) · EventsFeed (actions+context) ·
                          SessionsPane (history) · ProviderStart (server start) ·
                          assistant.api · chat.store (conversation+lifecycle) ·
                          providers.store (catalog/selection/keys/watch) ·
                          sessions.store (directory: list + active pointer)
    voice/                voice.api · voice.store (poll loop, transcript→agent)
    settings/             SettingsPage (#/settings[/section], incl. voice)
  components/ui/          matrix-orb.svelte · bounce-sidebar.svelte (Svelte port)
  shared/                 Logo · LangTheme · AuthPage · SelectMenu (upward dropdown) ·
                          ApiKeyModal
  lib/                    api.ts (typed fetch, 30 s default budget) · theme.css
                          theme.svelte · i18n/ (es contract, en) · i18n.svelte ·
                          cn.ts · format.ts (pure, unit-tested)
electron/                 main.cjs (sidecar spawn + token vault) · preload.cjs
scripts/                  electron-dev.mjs (cargo build + vite + electron)
tests/                    smoke · layout (incl. @slow inference) · ai-keys ·
                          helpers (ports + user seeding)
                          (+ colocated src/**/*.test.ts unit, *.contract.test.ts)
eslint.config.js          js + ts + svelte + prettier-compat (npm run lint)
vitest.config.ts          unit + contract projects (test:unit / test:contract)
```

## State snapshot (verified 2026-09-18, P2 landed)

- `svelte-check`: **0 errors, 0 warnings**. `tsc --noEmit`: clean, `.tsx`
  included (no `.tsx` sources remain — the island is Svelte now).
- Auth (register/login/refresh/logout/delete) works against the sidecar;
  access token in memory, refresh token in the OS-keychain vault under
  Electron (localStorage fallback on web + one-time migration).
- Chat: persisted turns via `POST /v1/ai/run` (agentic, tools→actions);
  sessions list/detail/delete; view-only history browse with explicit
  model-adopt; provider select; OpenCode API-key flow with live verification;
  Ollama/llama.cpp local providers.
- Voice: local mic → energy VAD → whisper tiny → agent. Manual mode
  (press to start, silence auto-finishes) + wake mode ("hey" default,
  configurable). Transcripts send as user messages; busy agent keeps them in
  the composer. Mic button + Settings → Voice. See `docs/09-voice.md`.
- Gestures: cut from the UI (P2 #13) — the toggle switched state with no
  detection pipeline. Returns with the pipeline; tagline unchanged (vision).
- Supabase: module + dependency removed (P2 #14); design preserved in
  `docs/08-sync-design.md`. Bundle: `dist/` 464K → 132K (JS 104K).
- Diagnostics: sidecar stderr mirror at `GET /v1/support/diagnostics`,
  viewable/copyable in Settings → AI.
- Protocol guard: `SIDECAR_PROTOCOL = 2` (`src/lib/api.ts`); Shell warns
  on mismatch with `/health`.
- Provider availability is polled every 3 s (silent, paused when the tab is
  hidden); a startable-but-offline server (Ollama) offers a one-click Start
  in the chat pane and Settings → AI. See `docs/07-provider-availability.md`.
- Routes: `#/login | #/register | #/settings[/section] | #/` with one
  `syncAuthRoute` guard and an unknown-hash fallback to home.
