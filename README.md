# Smart PC — Frontend (Svelte 5 + Electron + Rust sidecar)

Domain architecture: each business domain owns its screens, API calls and
state. Transversal code (theme, i18n, fetch) lives in `lib/` and `shared/`.
Rare UI pieces are hand-ported to Svelte under `components/ui/`.

```
src/
  app/            App (boot + restore + route guard) · Shell (3-pane layout) ·
                  TopBar · router (#/login|register|settings[/section]|home)
  domains/
    auth/         LoginPage · RegisterPage · auth.api · auth.store
                  (vault-first refresh tokens, localStorage fallback on web)
    assistant/    CenterPanel (orb + chat) · EventsFeed (actions) ·
                  SessionsPane (chat history) · ProviderStart (server start) ·
                  assistant.api · chat.store (conversation+lifecycle) ·
                  providers.store (catalog/selection/keys/3s watch) ·
                  sessions.store (directory: list + active pointer)
    voice/        voice.api · voice.store (poll loop, transcript→agent)
    settings/     SettingsPage (General · AI · Voice · Account)
  components/ui/  matrix-orb.svelte · bounce-sidebar.svelte (Svelte ports)
  lib/
    theme.css     THE single theme file (mono light/dark, per-pane whites)
    theme.svelte  light/dark/auto manager (data-theme)
    i18n/         es.ts (key contract) · en.ts
    i18n.svelte   reactive t() + persistence
    api.ts        typed fetch → sidecar (bridge url+token, else VITE_* env;
                  30 s default budget, per-endpoint overrides)
  shared/         Logo · LangTheme · AuthPage · SelectMenu (upward dropdown) ·
                  ApiKeyModal
electron/         main.cjs (sidecar spawn + token vault, privileged) ·
                  preload.cjs bridge (sidecar info, ping, vault)
scripts/          electron-dev.mjs (cargo build + vite + electron)
tests/            smoke · layout (incl. @slow inference) · ai-keys · helpers
docs/             current-state docs (see docs/README.md)
```

## Run (needs Rust toolchain for the sidecar)

```bash
npm install
npm run dev:electron   # builds sidecar, starts vite + Electron (all-in-one)
```

Or pieces separately:

```bash
npm run dev            # web only → http://localhost:5173
# + in another terminal, the sidecar manually:
cargo run --manifest-path src/native/Cargo.toml -- \
  --port 18080 --token dev-token-min-16-chars --db /tmp/dev.db
```

`npm run build` → static `dist/` (what Electron loads packaged).
`npm run check` → svelte-check. `npm run verify` → check + lint +
format:check + unit tests. `npm run test:e2e` → Playwright fast path
(sidecar + vite boot automatically); `test:e2e:all` adds real inference;
`test:contract` → renderer↔sidecar shape pinning (spawns its own sidecar).

Chat turns persist per user (sessions + history in the sidecar); plain chat
never creates actions — those come only from command execution. Browsing
history never reselects the provider — adopt explicitly. Voice is local:
mic → VAD → whisper tiny in the sidecar; transcripts send as user messages
(manual button or "hey" wake word, silence auto-finishes). Renderer ↔ Main
speak through `window.smartpc` (preload allow-list); raw Node never
reaches the UI. Refresh tokens rest in the OS keychain under Electron.
