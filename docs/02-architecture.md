# 02 — Architecture

## Layers

```
┌─ Renderer (Svelte 5 runes, no Node) ─────────────────┐
│ app/ → domains/ → shared/ + components/ui/ → lib/    │
└──────────────────┬───────────────────────────────────┘
                   │ HTTP loopback (fetch) + window.smartpc bridge
┌─ Electron main (Node, privileged) ───────────────────┐
│ spawns sidecar, injects url+token via env → preload  │
└──────────────────┬───────────────────────────────────┘
                   │ spawn --port 0 --token <random> --db <userData>
┌─ Rust sidecar (smartpc-native) ──────────────────────┐
│ auth · ai providers · chat sessions · actions ·      │
│ harness (computer-use sandbox) · SQLite              │
└──────────────────────────────────────────────────────┘
```

Hard boundary: renderer ↔ main speak **only** through `window.smartpc`
(`electron/preload.cjs` allow-list: `versions`, `ping`, `sidecar`).
`nodeIntegration: false`, `sandbox: true`, `contextIsolation: true`
(`electron/main.cjs:66-80`).

## Renderer: domain ownership

Each business domain owns screens + API client + runes store:

- `domains/auth/` — `auth.api.ts` (6 endpoints), `auth.store.svelte.ts`
  (module-level `$state`: `user`, in-memory `accessToken`, `localStorage`
  refresh token). `login → register → restore → logout → deleteAccount`.
- `domains/assistant/` — `assistant.api.ts` (providers/selection/chat/run/
  sessions/actions/keys), `assistant.store.svelte.ts` (~416 lines, single
  god-store: orb, messages, events, sessions, providers, keys, voice, draft).
  `run` → `sessionDetail` → re-render (tool turns filtered out) → `refreshSessions`.
- `domains/settings/` — `SettingsPage.svelte` only; reads both stores, no
  store of its own.

Transversal code: `lib/api.ts` (base resolution: bridge → env → `127.0.0.1:18080`;
`X-Sidecar-Token` gate + `Authorization: Bearer` user token), `lib/theme.*`,
`lib/i18n.*`, `shared/` primitives.

## Routing & boot

Tiny hash router (`app/router.svelte.ts`): `#/login | #/register | #/`.
`App.svelte`: `initLang(); initTheme();` → `auth.restore()` → gate.
`Shell.svelte`: protocol check + `loadProviders().then(refreshSessions)`,
redirect to `#/login` when `auth.user` is null. Settings is **local view
state** (`view: "main" | "settings"`), not a route — not deep-linkable,
lost on reload.

## React islands

Rare UI components are vendored, not installed:
`matrix-orb.svelte` (hand-ported canvas orb, dependency-free, reduced-motion
aware) and `bounce-sidebar.tsx` (React + `motion`, mounted via
`shared/ReactIsland.svelte` → `createRoot`). Cost: `react`, `react-dom`,
`motion` in the prod bundle for one sidebar widget.

## Sidecar contract (renderer view)

- Gate: `X-Sidecar-Token` on every `/v1/*`; `/health` open (liveness).
- Auth: `POST /v1/auth/register|login|refresh|logout`, `GET /v1/auth/me`,
  `DELETE /v1/auth/account`; `{error:{code,message}}` envelope
  (`src/native/README.md`, `src/lib/api.ts:66-73`).
- AI: `GET /v1/ai/providers` (live detection), `POST /v1/ai/select`,
  `GET /v1/ai/selection`, `POST /v1/ai/chat` (plain, no actions),
  `POST /v1/ai/run` (agentic; mutating tool calls → Action rows),
  `GET|POST /v1/ai/keys`, `DELETE /v1/ai/keys/{provider}`.
- Chat: `GET /v1/chat/sessions`, `GET|DELETE /v1/chat/sessions/{id}`
  (detail = session + messages + actions).
- Actions: `GET /v1/actions?session_id=`, `POST /v1/actions`,
  `PATCH /v1/actions/{id}`. Plain chat never creates actions.
- Support: `GET /v1/support/diagnostics` (stderr mirror for the UI).

## Data & identity

- Local SQLite (`smartpc.db` in Electron `userData`, `/tmp/*.db` in dev).
  JWT secret derived per launch from the sidecar token → access tokens die
  with the process (sidecar README).
- Refresh tokens: opaque, hashed, rotated, reuse kills the chain; stored in
  renderer `localStorage` (`smartpc.refresh`) pending a safeStorage move
  (noted in `auth.store.svelte.ts:6-8`).
