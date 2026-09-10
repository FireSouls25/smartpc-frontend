import { aiApi, type ProviderInfo } from "./assistant.api";
import { getLang, t, type I18nKey } from "../../lib/i18n.svelte";

export type OrbState = "idle" | "listening" | "thinking";

export interface ChatMsg {
  id: number;
  role: "user" | "assistant";
  text?: string;
  textKey?: I18nKey;
}

export interface AppEvent {
  id: number;
  title: string;
  status: "running" | "done" | "failed";
  /** One-shot actions finish; continuous ones (slides, dictation…) stay running. */
  continuous: boolean;
}

let seq = 1;
let orb = $state<OrbState>("idle");
let messages = $state<ChatMsg[]>([
  { id: 0, role: "assistant", textKey: "chat.hello" },
]);
let events = $state<AppEvent[]>([]);
let providers = $state<ProviderInfo[]>([]);
let providersLoading = $state(false);
let providersError = $state("");
let selectError = $state("");
let activeProvider = $state("ollama");
let activeModel = $state("");
let gesturesOn = $state<boolean>(true);
let draft = $state("");
let voiceError = $state("");
// eslint-disable-next-line @typescript-eslint/no-explicit-any
let recognition: any = null;

function defaultModelFor(list: ProviderInfo[], id: string): string {
  const p = list.find((x) => x.id === id);
  return p?.default_model || p?.models[0] || "";
}

async function loadProviders(): Promise<void> {
  providersLoading = true;
  providersError = "";
  try {
    const res = await aiApi.providers();
    providers = res.providers;
    activeProvider = res.active.provider;
    activeModel =
      res.active.model || defaultModelFor(providers, activeProvider);
  } catch (err) {
    providersError = err instanceof Error ? err.message : "Error";
  } finally {
    providersLoading = false;
  }
}

async function selectProvider(id: string): Promise<void> {
  selectError = "";
  const p = providers.find((x) => x.id === id);
  if (!p || !p.available) {
    selectError = t("providers.offline");
    return;
  }
  try {
    const res = await aiApi.select(id);
    activeProvider = res.provider;
    activeModel = res.model || defaultModelFor(providers, activeProvider);
  } catch (err) {
    selectError = err instanceof Error ? err.message : "Error";
  }
}

async function selectModel(m: string): Promise<void> {
  selectError = "";
  try {
    const res = await aiApi.select(activeProvider, m);
    activeProvider = res.provider;
    activeModel = res.model || m;
  } catch (err) {
    selectError = err instanceof Error ? err.message : "Error";
  }
}

function activeModels(): string[] {
  return providers.find((p) => p.id === activeProvider)?.models ?? [];
}

function activeAvailable(): boolean {
  return (
    providers.find((p) => p.id === activeProvider)?.available ?? false
  );
}

function setDraft(v: string): void {
  draft = v;
}

/** Real send: user msg → thinking → local model reply + completed event. */
async function send(text: string): Promise<void> {
  const clean = text.trim();
  if (!clean || orb === "thinking") return;
  try {
    recognition?.stop();
  } catch {
    /* not listening */
  }
  voiceError = "";
  messages = [...messages, { id: seq++, role: "user", text: clean }];
  draft = "";
  orb = "thinking";
  const ev: AppEvent = {
    id: seq++,
    title: clean.slice(0, 80),
    status: "running",
    continuous: false,
  };
  events = [ev, ...events];
  try {
    const res = await aiApi.chat([{ role: "user", content: clean }]);
    messages = [...messages, { id: seq++, role: "assistant", text: res.reply }];
    events = events.map((e) =>
      e.id === ev.id ? { ...e, status: "done" as const } : e,
    );
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Error";
    messages = [
      ...messages,
      { id: seq++, role: "assistant", text: `Error: ${msg}` },
    ];
    events = events.map((e) =>
      e.id === ev.id ? { ...e, status: "failed" as const } : e,
    );
  } finally {
    orb = "idle";
  }
}

function speechCtor(): (new () => unknown) | null {
  const w = window as unknown as Record<string, unknown>;
  const ctor = w.SpeechRecognition ?? w.webkitSpeechRecognition ?? null;
  return ctor as (new () => unknown) | null;
}

/**
 * Real voice receptor (Web Speech API: works in Electron/Chromium).
 * Final transcripts auto-send to the active local model.
 */
function toggleListening(): void {
  if (orb === "thinking") return;
  if (orb === "listening") {
    try {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (recognition as any)?.stop();
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
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    recognition = new (Ctor as any)();
  } catch {
    voiceError = t("voice.error");
    return;
  }
  recognition.lang = getLang() === "es" ? "es-ES" : "en-US";
  recognition.interimResults = true;
  recognition.maxAlternatives = 1;
  recognition.onresult = (e: {
    resultIndex: number;
    results: ArrayLike<{ isFinal: boolean; 0: { transcript: string } }>;
  }) => {
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

export const assistant = {
  get orb(): OrbState {
    return orb;
  },
  get messages(): ChatMsg[] {
    return messages;
  },
  get events(): AppEvent[] {
    return events;
  },
  get providers(): ProviderInfo[] {
    return providers;
  },
  get providersLoading(): boolean {
    return providersLoading;
  },
  get providersError(): string {
    return providersError;
  },
  get selectError(): string {
    return selectError;
  },
  get activeProvider(): string {
    return activeProvider;
  },
  get activeModel(): string {
    return activeModel;
  },
  activeModels,
  activeAvailable,
  get gesturesOn(): boolean {
    return gesturesOn;
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
  loadProviders,
  selectProvider,
  selectModel,
  toggleGestures(): void {
    gesturesOn = !gesturesOn;
  },
  toggleListening,
  send,
};
