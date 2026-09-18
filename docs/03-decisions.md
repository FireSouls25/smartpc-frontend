# 03 — Decisions (observed in code)

Recorded here because top-level `/docs/` planning docs predate the code and
contradict it in places (e.g. "no code yet", Vercel/Render/Neon deploy,
SvelteKit web app, MediaPipe gestures). Code wins; planning docs need a
reconciliation pass.

1. **Sidecar over in-main backend.** Heavy work (SQLite, providers, harness)
   lives in a Rust child process, not Electron main. Main only spawns it and
   passes url+token. Rationale in code: crash isolation, no Node-native
   modules, reuse from plain web dev. (`electron/main.cjs:29-64`)
2. **Token gate, not origin, is the loopback boundary.** CORS open locally,
   no TLS on loopback, per-launch 64-hex-char token, `/health` unauthenticated.
   (sidecar README "Security notes")
3. **Renderer is unprivileged.** Preload allow-list only; no IPC beyond
   `system:ping` yet. Executor/mic/vault IPC explicitly deferred
   (`preload.cjs:17`, `main.cjs:91-95`).
4. **Domain-owned renderer state (Svelte 5 runes).** No global store lib;
   module-level `$state` + getter exports. `es.ts` is the i18n key contract,
   `en.ts` must satisfy `Record<I18nKey, string>` at compile time.
5. **Single theme file.** `lib/theme.css` layers raw palette → semantic tokens
   → `[data-theme]` variants; components use vars/classes only. FOUC guard
   inline in `index.html`. `auto` = OS media query + change listener.
6. **Vendored Rare UI, dual runtime.** Matrix orb ported to dependency-free
   Svelte; Bounce Sidebar kept as React via `ReactIsland` (accepts
   react+motion bundle cost for one widget).
7. **Plain chat vs agentic run are separate endpoints.** `/chat` persists
   turns, never creates actions; `/run` reasons with tools and only mutating
   calls become Action rows. Renderer only calls `/run`.
8. **API keys in OS credential store**, never SQLite/logs; env override for
   CI; save verifies with a tiny live call (`invalid_key` vs `unverified`
   distinguished — bot-protection notes in sidecar README).
9. **Supabase scoped to accounts+preferences sync, unwired.** Local auth stays
   the only login until multi-device sync is designed (`src/supabase/README.md`).
10. **Protocol version handshake** (`SIDECAR_PROTOCOL = 2`) with a UI banner
    on mismatch instead of silent breakage (`Shell.svelte:38-45`).
11. **Upward dropdowns by default** (`SelectMenu` `align="up"`) because chat
    controls sit low on screen.
12. **Manual E2E over fixtures.** Playwright specs seed real users via HTTP
    and drive the real sidecar; no mocks, no `webServer` wiring (see 05).
13. **Provider availability is watched, servers are startable.** The renderer
    re-probes providers every 3 s (silent, skipped when hidden) so a server
    started after boot flips the UI without manual refresh; the sidecar can
    launch `ollama serve` itself (PATH lookup, detached spawn, `CREATE_NO_WINDOW`
    on Windows) and the child outlives the sidecar by design. Additive contract,
    no PROTOCOL bump. (see `frontend/docs/07-provider-availability.md`)
