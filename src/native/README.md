# smartpc-native — local backend sidecar (Rust)

Spawned by the Electron app. Serves a small JSON API on `127.0.0.1`,
stores data in local SQLite, and will host the heavy native work
(`enigo` input, `whisper-rs` STT…) in-process later — out of Electron's way.

```
src/native/
  src/main.rs            CLI (--port/--token/--db), READY contract, shutdown
  src/api.rs             router, sidecar-token gate, user gate, error mapping
  src/auth/              domain: model · service · store · routes · schema.sql
  src/ai/                local models: provider trait · openai_compat client ·
                         ollama + llamacpp presets · chat route
  src/chat/               sessions · messages · actions · selection (per user)
  src/harness/            computer-use sandbox: context · tools · exec · agent loop
  src/platform/          shared SQLite setup (each domain owns its schema)
```

## Protocol

- Loopback only (`127.0.0.1`, `--port 0` = OS-assigned).
- `X-Sidecar-Token: <per-launch token>` required on every `/v1/*` route.
  `/health` stays open for liveness probes.
- Same auth contract as before: `POST /v1/auth/register|login|refresh|logout`,
  `GET /v1/auth/me`, `DELETE /v1/auth/account`, `{error:{code,message}}`.
- Local models: `POST /v1/ai/chat`
  `{session_id?, provider?, model?, message}` → `{reply, model, provider,
session_id}`. Turns persist in the session (last 20 feed the model).
  Plain chat never creates actions.
- `GET /v1/ai/providers` (live detection; each entry carries `startable`
  - `installed`) · `POST /v1/ai/providers/{id}/start` (launches `ollama
serve` when down, waits until it answers) · `POST /v1/ai/select` (validates
  - persists) · `GET /v1/ai/selection`.
- Sessions: `GET|POST /v1/chat/sessions`, `GET|DELETE /v1/chat/sessions/{id}`
  (detail includes messages + actions).
- Actions (executor hook): `GET /v1/actions?session_id=`,
  `POST /v1/actions`, `PATCH /v1/actions/{id}`. All user-scoped.

## Harness (computer use)

The model never touches the OS. Per run it receives a fresh `SystemContext`
(OS/version, X11/Wayland session, focused app, input capabilities via
sysinfo + active-win-pos-rs) plus an OS-specific interaction guide, then
reasons in an agent loop (`POST /v1/ai/run`) over one OpenAI-style `tools`
array that works for Ollama and llama.cpp (`--jinja`).

- Catalog (closed): `get_system_context`, `list_processes` (read-only),
  `open_app` (low), `close_app` (medium), `press_key` (medium, key allowlist),
  `type_text` (high, needs `HARNESS_ALLOW_RISKY=1`).
- Debug: `HARNESS_DEBUG=1` logs every loop turn (user text, model text,
  tool calls) to sidecar stderr — the evidence to paste when reporting
  misbehavior.
- Execution is the sandbox: unknown tools, bad args and policy blocks fail
  as results. Input via enigo (X11 full; Wayland restricted by design —
  attempted once, reported honestly). App launching via per-OS spawn
  (PATH / `open -a` / `start`), never a shell; friendly names resolve
  (`terminal`, `Prism Launcher` → real binaries), spawns are verified alive
  after 600 ms (instant exit = honest failure, not fake success), and
  `close_app` terminates by name with the same verification (refuses its
  own backend; exited children are reaped so no zombies linger).
- Portability: the Windows/macOS branches are minimal std-only code paths
  (reviewed, not compiled here — cross-checking needs mingw/mac SDKs, i.e.
  CI). Wayland input stays experimental upstream and is intentionally off.
- Only mutating tools become Action rows (read-only stays in the run
  trace); every step returns `{tool, ok, action_id}` in the run response.

## Security notes

- Passwords: argon2id. Access: short HS256 JWT. Refresh: opaque, hashed,
  rotated, reuse kills the chain.
- The JWT secret is derived per launch from the sidecar token, so access
  tokens die with the process; nothing secret travels on the command line
  except the token itself (same trust domain: the spawning app).
- No HSTS / no TLS on purpose: plain HTTP on loopback must stay plain;
  the token gate (not the origin) is the boundary, so CORS is open locally.
- Blocking SQLite calls run inline; fine for local auth traffic, revisit
  with `spawn_blocking` when streaming STT lands.

## Remote providers with keys (OpenCode Zen)

`opencode` (https://opencode.ai/zen, OpenAI-compatible chat + public model
catalog) joins ollama/llama.cpp through the same `LlmProvider` trait.
API keys live in the OS credential store (Keychain / Credential Manager /
Secret Service) under `smart-pc`, account `<user-id>:<provider>-api-key` —
never in SQLite, logs or responses. `<PROVIDER>_API_KEY` env vars override
for containers/CI. Saving a key verifies it with one tiny live call, so
typos fail fast (`invalid_key`) instead of mysteriously at chat time.
Notes from the field: Zen sits behind bot protection — the client
identifies as `smartpc-native/<version>` (requests without a UA get
challenged), verification tries chat-compatible catalog models first and
spaces attempts because rapid bursts get HTML 404s instead of API errors.
Anything inconclusive surfaces as `unverified`, never as a false "invalid".

## Dev

```bash
cargo build                    # from src/native/
cargo test
./target/debug/smartpc-native --port 18080 --token dev-token-min-16 --db /tmp/dev.db
curl localhost:18080/health
```
