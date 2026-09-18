# 01 — Overview

Smart PC is a **local-first desktop assistant**: a Svelte 5 UI in Electron,
driven by a Rust sidecar (`src/native/`, binary `smartpc-native`) that owns
auth, chat sessions, provider routing and OS actions over loopback HTTP.
The renderer never touches the OS directly.

## Runtime modes

| Mode | UI | Sidecar url+token from | Command |
|------|----|------------------------|---------|
| Electron (real) | desktop window | preload bridge (`window.smartpc.sidecar`, per-launch random token, OS-assigned port) | `npm run dev:electron` (cargo build + vite + electron) |
| Web dev | browser `:5173` | `VITE_API_URL` / `VITE_SIDECAR_TOKEN` (`.env` → `:18081`) | `npm run dev` + sidecar manually |

Packaged Electron loads static `dist/` (`npm run build`); main resolves the
sidecar from `resources/bin` instead of `src/native/target/debug`.

## Repo map (renderer-relevant)

```
src/
  main.ts                 entry: theme.css + mount App
  app/                    App.svelte (boot+restore) · Shell.svelte (3-pane layout)
                          TopBar.svelte · router.svelte.ts (hash router)
  domains/
    auth/                 LoginPage · RegisterPage · auth.api · auth.store (runes)
    assistant/            CenterPanel (orb+chat) · EventsFeed (actions+context)
                          SessionsPane (history) · assistant.api · assistant.store
    settings/             SettingsPage (Bounce Sidebar sections incl. account)
  components/ui/          matrix-orb.svelte (Svelte port) · bounce-sidebar.tsx (React island)
  shared/                 Logo · LangTheme · AuthPage · SelectMenu (upward dropdown)
                          ReactIsland (React-in-Svelte bridge) · ApiKeyModal
  lib/                    api.ts (typed fetch) · theme.css (THE theme file)
                          theme.svelte · i18n/ (es contract, en) · i18n.svelte · cn.ts
  supabase/               remote sync client — unwired (accounts+preferences only)
  native/                 Rust sidecar (see its README.md)
electron/                 main.cjs (spawns sidecar, privileged) · preload.cjs (bridge)
scripts/                  electron-dev.mjs (hardened dev runner) · kill-strays.mjs
tests/                    3 Playwright specs (manual harness, see 05-testing.md)
```

## State snapshot (verified 2026-09-18)

- `svelte-check`: **0 errors, 0 warnings**. `tsc --noEmit`: clean — but the
  `.tsx` island is **not in the program** (see 04-correctness-audit.md).
- Auth (register/login/refresh/logout/delete) works against the sidecar;
  access token in memory, refresh token in `localStorage`.
- Chat: persisted turns via `POST /v1/ai/run` (agentic, tools→actions);
  sessions list/detail/delete; provider select; OpenCode API-key flow with
  live verification; Ollama/llama.cpp local providers.
- Voice: Web Speech API receptor, final transcripts auto-send. No TTS.
- Gestures: **UI toggle only** — no camera, no detection pipeline.
  `assistant.gesturesOn` is state with no consumer besides the switch.
- Supabase: code present, **no UI imports it** (except nothing — grep shows
  zero imports from `src/supabase` outside itself).
- Diagnostics: sidecar stderr mirror at `GET /v1/support/diagnostics`,
  viewable/copyable in Settings → AI.
- Protocol guard: `SIDECAR_PROTOCOL = 2` (`src/lib/api.ts:27`); Shell warns
  on mismatch with `/health`.
- Provider availability is polled every 3 s (silent, paused when the tab is
  hidden); a startable-but-offline server (Ollama) offers a one-click Start
  in the chat pane and Settings → AI. See `docs/07-provider-availability.md`.
