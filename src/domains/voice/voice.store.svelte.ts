// Local voice control: mic button, wake/manual sessions, transcript routing.
// Capture + VAD + whisper all live in the sidecar; this store owns the poll
// loop and hands final transcripts to the chat store (they render as normal
// user messages and run through the agent like typed text).
import { voiceApi, type VoiceEvent, type VoiceMode } from "./voice.api";
import { ApiError } from "../../lib/api";
import { chatStore as chat } from "../assistant/chat.store.svelte";
import { getLang, t } from "../../lib/i18n.svelte";

export type { VoiceMode };
export type VoicePhase =
  "idle" | "starting" | "listening" | "capturing" | "error";

const MODE_KEY = "smartpc.voice.mode";
const WAKE_KEY = "smartpc.voice.wake";
export const DEFAULT_WAKE_WORD = "hey";

function loadMode(): VoiceMode {
  try {
    return window.localStorage.getItem(MODE_KEY) === "wake" ? "wake" : "manual";
  } catch {
    return "manual";
  }
}

function loadWake(): string {
  try {
    const w = (window.localStorage.getItem(WAKE_KEY) || "").trim();
    return w ? w.slice(0, 32) : DEFAULT_WAKE_WORD;
  } catch {
    return DEFAULT_WAKE_WORD;
  }
}

function persist(key: string, value: string): void {
  try {
    window.localStorage.setItem(key, value);
  } catch {
    /* private mode */
  }
}

const sleep = (ms: number): Promise<void> =>
  new Promise((r) => setTimeout(r, ms));

let phase = $state<VoicePhase>("idle");
let mode = $state<VoiceMode>(loadMode());
let wakeWord = $state<string>(loadWake());
let error = $state("");
// Generation counter: stale async work (a listen that resolves after stop,
// an old poll loop) stands down instead of fighting the current state.
let run = 0;
let cursor = 0;

function mapListenError(err: unknown): string {
  const code = err instanceof ApiError ? err.code : undefined;
  if (code === "no_microphone" || code === "unsupported_audio") {
    return t("voice.unsupported");
  }
  if (code === "timeout") return t("voice.downloading");
  return t("voice.error") + (code ? ` (${code})` : "");
}

async function start(requested?: VoiceMode): Promise<void> {
  if (phase === "starting" || phase === "listening" || phase === "capturing") {
    return;
  }
  const my = ++run;
  if (requested) setMode(requested);
  phase = "starting";
  error = "";
  try {
    await voiceApi.listen({
      mode,
      wake_word: wakeWord,
      lang: getLang(),
    });
  } catch (err) {
    if (my !== run) return;
    phase = "error";
    error = mapListenError(err);
    return;
  }
  if (my !== run) {
    // Stopped while the (possibly downloading) listen was in flight:
    // release the orphan session server-side.
    try {
      await voiceApi.stop();
    } catch {
      /* already gone */
    }
    return;
  }
  cursor = 0;
  phase = "listening";
  chat.setOrb("listening");
  void pollLoop(my);
}

async function pollLoop(my: number): Promise<void> {
  while (my === run) {
    let batch;
    try {
      batch = await voiceApi.poll(cursor);
    } catch {
      if (my !== run) return;
      // Transient (sidecar restart, blip): the server holds session state,
      // so back off and resume polling with the same cursor.
      await sleep(1000);
      continue;
    }
    if (my !== run) return;
    cursor = batch.next;
    for (const ev of batch.events) {
      if (my !== run) return;
      await handle(ev);
    }
  }
}

async function handle(ev: VoiceEvent): Promise<void> {
  switch (ev.type) {
    case "capturing":
      phase = ev.active ? "capturing" : "listening";
      chat.setOrb("listening");
      break;
    case "wake":
      phase = "capturing";
      break;
    case "transcript":
      await onTranscript(ev.text);
      break;
    case "error":
      phase = "error";
      error = t("voice.error") + (ev.message ? `: ${ev.message}` : "");
      run++;
      if (chat.orb === "listening") chat.setOrb("idle");
      break;
    case "end":
      phase = "idle";
      run++;
      if (chat.orb === "listening") chat.setOrb("idle");
      break;
  }
}

async function onTranscript(text: string): Promise<void> {
  const clean = text.trim();
  if (!clean) return;
  const dispatched = await chat.send(clean);
  if (!dispatched) {
    // Agent busy: keep the words in the composer instead of losing them.
    chat.setDraft(clean);
    error = t("voice.busy");
  }
}

async function stop(): Promise<void> {
  run++;
  const wasActive =
    phase === "starting" || phase === "listening" || phase === "capturing";
  phase = "idle";
  if (wasActive) {
    try {
      await voiceApi.stop();
    } catch {
      /* already gone */
    }
  }
  if (chat.orb === "listening") chat.setOrb("idle");
}

function toggle(): void {
  if (phase === "starting" || phase === "listening" || phase === "capturing") {
    void stop();
  } else {
    void start();
  }
}

function setMode(m: VoiceMode): void {
  mode = m;
  persist(MODE_KEY, m);
}

function setWakeWord(w: string): void {
  wakeWord = w.trim().slice(0, 32) || DEFAULT_WAKE_WORD;
  persist(WAKE_KEY, wakeWord);
}

export const voice = {
  get phase(): VoicePhase {
    return phase;
  },
  get mode(): VoiceMode {
    return mode;
  },
  get wakeWord(): string {
    return wakeWord;
  },
  get error(): string {
    return error;
  },
  get listening(): boolean {
    return phase === "listening" || phase === "capturing";
  },
  get capturing(): boolean {
    return phase === "capturing";
  },
  start,
  stop,
  toggle,
  setMode,
  setWakeWord,
};
