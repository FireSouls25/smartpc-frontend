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
- `GET /v1/ai/providers` (live detection) · `POST /v1/ai/select` (validates
  + persists) · `GET /v1/ai/selection`.
- Sessions: `GET|POST /v1/chat/sessions`, `GET|DELETE /v1/chat/sessions/{id}`
  (detail includes messages + actions).
- Actions (executor hook): `GET /v1/actions?session_id=`,
  `POST /v1/actions`, `PATCH /v1/actions/{id}`. All user-scoped.

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

## Dev

```bash
cargo build                    # from src/native/
cargo test
./target/debug/smartpc-native --port 18080 --token dev-token-min-16 --db /tmp/dev.db
curl localhost:18080/health
```
