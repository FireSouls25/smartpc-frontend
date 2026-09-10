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
  src/platform/          shared SQLite setup (each domain owns its schema)
```

## Protocol

- Loopback only (`127.0.0.1`, `--port 0` = OS-assigned).
- `X-Sidecar-Token: <per-launch token>` required on every `/v1/*` route.
  `/health` stays open for liveness probes.
- Same auth contract as before: `POST /v1/auth/register|login|refresh|logout`,
  `GET /v1/auth/me`, `DELETE /v1/auth/account`, `{error:{code,message}}`.
- Local models: `POST /v1/ai/chat`
  `{provider: "ollama"|"llama.cpp", model?, messages[], temperature?,
  max_tokens?, json_mode?}` → `{reply, model, provider}`.
  One OpenAI-compatible client serves both (Ollama's `/v1` endpoint and
  llama.cpp server speak it); vendors only preset URL/model/defaults.
  Errors: `unknown_provider` 400, `ai_upstream` 502 when the model server
  is down, `validation` 400.

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
