# 09 — Local voice input (mic → VAD → whisper → agent)

Implemented 2026-09-18. Press (or say "hey"), talk, pause — the silence
finalizes the utterance, whisper transcribes on-device, the text is sent to
the agent as if typed. No finish button, no finish keyword, no network.

## Pipeline

```text
cpal mic ──► 16 kHz mono f32 ──► energy VAD ──► utterance (+300 ms pre-roll)
                                                        │
                         wake mode: transcribe onset ──► wake word in it?
                                                        │ yes → capture NEXT
                                                        │       utterance
                                                        ▼
manual mode: first utterance ──► transcribe ──► event + auto-stop
wake mode:   command utterance ──► transcribe ──► event, re-arm
```

`src/native/src/stt/`: `audio` (capture, mixdown, linear resample),
`vad` (threshold + min-speech gate + silence hangover), `wake` (text match),
`model` (naming, path, download), `engine` (whisper-rs wrapper),
`session` (one global session, listener thread, event queue), `routes`.

## Modes

- **manual** (default): mic button starts, first utterance transcribes,
  session ends by itself. Starting is a press; finishing never is.
- **wake**: armed until the wake word ("hey" default, editable in
  Settings → Voice, 1–32 chars) appears in a transcribed onset utterance;
  the next utterance is the command; re-arms until stopped.

Wake detection is textual, not acoustic: VAD-gated onsets are transcribed
with tiny and matched whole-word ("they" ≠ "hey"; "¡Hey!" = "hey").
Cost: one tiny transcription per speech onset while armed — nothing runs
continuously. Limitation, stated: any mention wakes, background chatter
costs CPU, accents are significant.

## Silence finalize

VAD frames are 30 ms RMS at 16 kHz. Defaults: threshold `0.02`,
`VOICE_SILENCE_MS=1200` hangover, `VOICE_MIN_SPEECH_MS=400` gate (clicks
and coughs never start an utterance), `VOICE_MAX_UTTERANCE_S=30` cap.
All env-overridable (`VOICE_THRESHOLD` is raw RMS). Pre-roll keeps the
~300 ms before onset so the first syllable survives the gate.

## Model (low-hardware first)

Default `tiny` (~75 MB): 11 s of speech transcribes in ~1.5 s on a laptop
CPU (measured 2026-09-18, realtime factor ~0.14×). `WHISPER_MODEL` picks
`tiny|tiny.en|base|base.en|small`; `WHISPER_THREADS` caps CPU (default
min(4, cpus)); language per session (`es` default, from the UI lang).
First use downloads from HuggingFace into `<db-dir>/models/`
(`WHISPER_MODEL_DIR` overrides); progress goes to diagnostics. Greedy,
single-segment, blank-suppressed: commands want latency, not poetry.

## HTTP contract (sidecar-gated, like providers)

- `GET /v1/voice/status` → `{listening, capturing, mode, model, wake_word,
mic, model_ready}`. Never blocks on audio.
- `POST /v1/voice/listen {mode, wake_word?, lang?, model?}` → `{ok}`.
  Blocks on first-use download (client budget 3 min). Errors:
  `already_listening` 409, `no_microphone` 503, `invalid_mode|
invalid_wake_word|invalid_lang` 400, `model_failed` 502.
- `POST /v1/voice/stop` → `{ok}` always (idempotent).
- `GET /v1/voice/events?cursor=N` → `{events, next}`, holds ~25 s.
  Events: `capturing{active}`, `wake{word}`, `transcript{text}`,
  `error{code,message}`, `end{}`.

## Frontend (`domains/voice/`)

`voice.api.ts` (typed client) + `voice.store.svelte.ts` (prefs in
localStorage, poll loop with cursor + generation counter so stale async work
stands down, transcript routing). Final transcripts call `chat.send()` —
they render as user bubbles and run the agent identically to typed text; if
the agent is busy the words land in the composer instead of being lost.
Orb: capturing → `listening`; handoff leaves the agent's `thinking` alone.
Mic button toggles the preferred mode; Settings → Voice holds mode, wake
word, mic/model status. Prefs: `smartpc.voice.mode`, `smartpc.voice.wake`.

## Platforms & build

- Capture backends: ALSA (Linux), CoreAudio (macOS), WASAPI (Windows) via
  cpal — no per-OS code in the app. Mic permission is the OS dialog (macOS
  consent, PipeWire portal); browsers are out of the loop (the sidecar
  captures, not the page).
- Build needs: cmake + C++ compiler (whisper.cpp compiles from bundled
  sources) + `libasound2-dev` on Linux. CI installs both (see `ci.yml`).
- Verified live 2026-09-18: real DMIC present, tiny downloaded, session
  start/status/poll/stop all green against hardware. Acoustic loopback was
  impossible here (HDMI/speakers silent, mic at noise floor), so
  transcription was proven on the JFK sample through the same engine path.

## Limits & next

- No partial transcripts (whisper is batch; VAD level could feed a meter).
- One global session; no TTS reply path yet.
- Wake word is single-phrase text match — a proper acoustic spotter
  (openWakeWord/Porcupine) is the upgrade if false-wake cost ever matters.
- 19 Rust unit tests (VAD machine, wake match, resample/mixdown, model map);
  contract pins status shape + listen validation; E2E covers the Voice
  settings section (headless has no mic — asserts graceful `notReady`).
