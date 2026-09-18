// Open conversation: messages, events, draft, voice, context meter, and the
// session lifecycle (new/open/delete/adopt) orchestrating the session
// directory + provider selection. Depends on sessions + providers stores;
// they never import back (acyclic by construction).
import { aiApi } from "./assistant.api";
import { ApiError } from "../../lib/api";
import { getLang, t, type I18nKey } from "../../lib/i18n.svelte";
import { sessionStore } from "./sessions.store.svelte";
import { providerStore } from "./providers.store.svelte";

export type OrbState = "idle" | "listening" | "thinking";

export interface ChatMsg {
  /** Server message id when persisted; undefined for the local greeting. */
  id?: string;
  role: "user" | "assistant";
  text?: string;
  textKey?: I18nKey;
  steps?: { tool: string; ok: boolean }[];
}

export interface AppEvent {
  id: string;
  title: string;
  status: "running" | "done" | "failed";
  continuous: boolean;
}

const greeting = (): ChatMsg[] => [
  { role: "assistant", textKey: "chat.hello" },
];

let orb = $state<OrbState>("idle");
let messages = $state<ChatMsg[]>(greeting());
let events = $state<AppEvent[]>([]);
let contextUsed = $state(0);
let draft = $state("");
let voiceError = $state("");

// Minimal Web Speech API surface (no DOM lib dependency on the event
// shapes; replaces the previous `any` + eslint-disable).
interface SpeechRecognitionResultLike {
  readonly isFinal: boolean;
  readonly 0: { readonly transcript: string };
}

interface SpeechRecognitionEventLike {
  readonly resultIndex: number;
  readonly results: ArrayLike<SpeechRecognitionResultLike>;
}

interface SpeechRecognitionLike {
  lang: string;
  interimResults: boolean;
  maxAlternatives: number;
  onresult: ((event: SpeechRecognitionEventLike) => void) | null;
  onerror: ((event: { error?: string }) => void) | null;
  onend: (() => void) | null;
  start(): void;
  stop(): void;
}

let recognition: SpeechRecognitionLike | null = null;

export function toEvent(a: {
  id: string;
  title: string;
  status: string;
}): AppEvent {
  const status =
    a.status === "done" ? "done" : a.status === "failed" ? "failed" : "running";
  return {
    id: a.id,
    title: a.title,
    status,
    continuous: status === "running",
  };
}

function setDraft(v: string): void {
  draft = v;
}

function newChat(): void {
  sessionStore.clearActive();
  messages = greeting();
  events = [];
}

async function openSession(id: string): Promise<void> {
  const d = await aiApi.sessionDetail(id);
  sessionStore.setActive(id, {
    provider: d.session.provider,
    model: d.session.model,
  });
  // Machine turns (role "tool") stay server-side; the chat shows people only.
  messages = d.messages
    .filter((m) => m.role !== "tool")
    .map((m) => ({
      id: m.id,
      role: m.role === "assistant" ? "assistant" : "user",
      text: m.content,
    }));
  events = d.actions.map(toEvent);
  // View-only: the global selection is untouched. Follow-ups run under the
  // active combo until the user explicitly adopts this session's (see
  // adoptSessionCombo, surfaced in CenterPanel).
}

/** Adopt the viewed session's combo as the global selection. */
async function adoptSessionCombo(): Promise<void> {
  const combo = sessionStore.sessionCombo;
  if (!combo) return;
  await providerStore.selectProvider(combo.provider, combo.model ?? undefined);
  sessionStore.clearCombo();
}

async function deleteSession(id: string): Promise<void> {
  await aiApi.deleteSession(id);
  sessionStore.removeFromList(id);
  if (sessionStore.activeSessionId === id) newChat();
}

/**
 * Real send: the turn is persisted server-side (session + history).
 * Plain chat never creates actions — those come only from command execution.
 */
async function send(text: string): Promise<void> {
  const clean = text.trim();
  if (!clean || orb === "thinking") return;
  try {
    recognition?.stop();
  } catch {
    /* not listening */
  }
  voiceError = "";
  messages = [...messages, { role: "user", text: clean }];
  draft = "";
  orb = "thinking";
  try {
    const res = await aiApi.run(clean, sessionStore.activeSessionId, {
      lang: getLang(),
    });
    sessionStore.setActive(res.session_id, null);
    contextUsed = res.context.used_tokens;
    if (res.context.window != null)
      providerStore.setContextWindow(res.context.window);
    const d = await aiApi.sessionDetail(res.session_id);
    const steps = (res.steps ?? []).map((s) => ({ tool: s.tool, ok: s.ok }));
    // Machine turns (role "tool") never render as chat bubbles: without
    // this filter they show up as raw-JSON user messages.
    messages = d.messages
      .filter((m) => m.role !== "tool")
      .map((m, i, arr) => ({
        id: m.id,
        role: m.role === "assistant" ? "assistant" : "user",
        text: m.content,
        ...(i === arr.length - 1 && steps.length > 0 ? { steps } : {}),
      }));
    events = d.actions.map(toEvent);
    await sessionStore.refreshSessions();
    // The turn above ran under the active combo, so the viewed session now
    // continues under it too — the adopt affordance has served its purpose.
    sessionStore.clearCombo();
  } catch (err) {
    const msg =
      err instanceof ApiError && err.code === "timeout"
        ? t("chat.timeout")
        : err instanceof Error
          ? err.message
          : "Error";
    messages = [...messages, { role: "assistant", text: `Error: ${msg}` }];
  } finally {
    orb = "idle";
  }
}

function speechCtor(): (new () => SpeechRecognitionLike) | null {
  const w = window as unknown as Record<string, unknown>;
  const ctor = w.SpeechRecognition ?? w.webkitSpeechRecognition ?? null;
  return ctor as (new () => SpeechRecognitionLike) | null;
}

/**
 * Real voice receptor (Web Speech API: works in Electron/Chromium).
 * Final transcripts auto-send to the active local model.
 */
function toggleListening(): void {
  if (orb === "thinking") return;
  if (orb === "listening") {
    try {
      recognition?.stop();
    } catch {
      /* already stopped */
    }
    return;
  }
  const Ctor = speechCtor();
  voiceError = "";
  if (!Ctor) {
    voiceError = t("voice.unsupported");
    return;
  }
  try {
    recognition = new Ctor();
  } catch {
    voiceError = t("voice.error");
    return;
  }
  recognition.lang = getLang() === "es" ? "es-ES" : "en-US";
  recognition.interimResults = true;
  recognition.maxAlternatives = 1;
  recognition.onresult = (e) => {
    let interim = "";
    let fin = "";
    for (let i = e.resultIndex; i < e.results.length; i++) {
      const r = e.results[i];
      if (r.isFinal) fin += r[0].transcript;
      else interim += r[0].transcript;
    }
    if (interim) draft = interim;
    if (fin.trim()) void send(fin);
  };
  recognition.onerror = (e: { error?: string }) => {
    voiceError = t("voice.error") + (e?.error ? ` (${e.error})` : "");
    if (orb === "listening") orb = "idle";
  };
  recognition.onend = () => {
    if (orb === "listening") orb = "idle";
  };
  try {
    recognition.start();
    orb = "listening";
  } catch {
    voiceError = t("voice.error");
    orb = "idle";
  }
}

export const chatStore = {
  get orb(): OrbState {
    return orb;
  },
  get messages(): ChatMsg[] {
    return messages;
  },
  get events(): AppEvent[] {
    return events;
  },
  get contextUsed(): number {
    return contextUsed;
  },
  get listening(): boolean {
    return orb === "listening";
  },
  get draft(): string {
    return draft;
  },
  get voiceError(): string {
    return voiceError;
  },
  setDraft,
  newChat,
  openSession,
  adoptSessionCombo,
  deleteSession,
  toggleListening,
  send,
};
