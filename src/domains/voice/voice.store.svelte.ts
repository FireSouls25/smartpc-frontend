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
const DEVICE_KEY = "smartpc.voice.device";
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

function loadDevice(): string | null {
  try {
    const d = (window.localStorage.getItem(DEVICE_KEY) || "").trim();
    return d ? d.slice(0, 128) : null;
  } catch {
    return null;
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

function setNotice(text: string): void {
  notice = text;
  if (noticeTimer !== null) window.clearTimeout(noticeTimer);
  if (text) {
    noticeTimer = window.setTimeout(() => {
      notice = "";
      noticeTimer = null;
    }, 4000);
  }
}

let phase = $state<VoicePhase>("idle");
let mode = $state<VoiceMode>(loadMode());
let wakeWord = $state<string>(loadWake());
let device = $state<string | null>(loadDevice());
let error = $state("");
// Transient acknowledgment ("heard the wake word, talk now"), cleared after
// a few seconds or on the next state change.
let notice = $state("");
let noticeTimer: number | null = null;
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
  setNotice("");
  let epoch: number;
  try {
    const res = await voiceApi.listen({
      mode,
      wake_word: wakeWord,
      lang: getLang(),
    });
    epoch = res.epoch;
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
  void pollLoop(my, epoch);
}

async function pollLoop(my: number, epoch: number): Promise<void> {
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
      // The queue is global across sessions: drop anything that isn't ours
      // (a previous session's Transcript/End replayed here used to kill the
      // new poll loop and orphan a live session → permanent 409s).
      if (ev.epoch !== epoch) continue;
      await handle(ev);
    }
  }
}

async function handle(ev: VoiceEvent): Promise<void> {
  switch (ev.type) {
    case "started":
      // Session confirmed live; we're already in listening.
      break;
    case "capturing":
      phase = ev.active ? "capturing" : "listening";
      chat.setOrb("listening");
      break;
    case "wake":
      // The session heard the wake word and is recording the command:
      // say so out loud in the UI, or users talk into the void.
      phase = "capturing";
      setNotice(t("voice.hello"));
      break;
    case "transcript":
      setNotice("");
      await onTranscript(ev.text);
      break;
    case "error": {
      // Server errors are terminal (the session ends server-side too), so
      // force local cleanup first: this guarantees the next press starts
      // fresh instead of 409ing on a stuck record.
      const msg = t("voice.error") + (ev.message ? `: ${ev.message}` : "");
      await stop();
      phase = "error";
      error = msg;
      break;
    }
    case "end":
      setNotice("");
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
  setNotice("");
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
  get notice(): string {
    return notice;
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
