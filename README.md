# Smart PC — Frontend (Svelte 5 + Electron + Rust sidecar)

Domain architecture: each business domain owns its screens, API calls and
state. Transversal code (theme, i18n, fetch) lives in `lib/` and `shared/`.
Rare UI islands are vendored under `components/ui/` (see each file header).

```
src/
  app/            App (boot + restore) · Shell · TopBar · hash router
  domains/
    auth/         LoginPage · RegisterPage · auth.api · auth.store (runes)
    assistant/    CenterPanel (orb + chat) · EventsFeed · assistant.store
    settings/     SettingsPage (Bounce Sidebar sections, incl. account)
  components/ui/  matrix-orb.svelte (Svelte port) · bounce-sidebar.tsx (island)
  native/         Rust sidecar: local backend Electron spawns (see its README)
  supabase/       remote sync client (accounts + preferences only, unwired)
  lib/
    theme.css     THE single theme file (Catppuccin Latte + dark)
    theme.svelte  light/dark/auto manager (data-theme)
    i18n/         es.ts (key contract) · en.ts
    i18n.svelte   reactive t() + persistence
    api.ts        typed fetch → sidecar (bridge url+token, else VITE_* env)
  shared/         Logo · LangTheme · ReactIsland (rare-ui bridge)
electron/         main.cjs (spawns sidecar, privileged) · preload.cjs bridge
scripts/          electron-dev.mjs (cargo build + vite + electron)
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
`npm run check` → svelte-check.

Only auth is wired (to the sidecar); the assistant is a visual mockup to
design the rest of the look and functions. Renderer ↔ Main speak through
`window.smartpc` (preload allow-list); raw Node never reaches the UI.
