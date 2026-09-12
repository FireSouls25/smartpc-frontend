import { aiApi, type ProviderInfo, type SessionSummary } from "./assistant.api";
import { getLang, t, type I18nKey } from "../../lib/i18n.svelte";
export type OrbState = "idle" | "listening" | "thinking";

export interface ChatMsg {
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
let sessions = $state<SessionSummary[]>([]);
let sessionsError = $state("");
let activeSessionId = $state<string | null>(null);
let providers = $state<ProviderInfo[]>([]);
let providersLoading = $state(false);
let providersError = $state("");
let selectError = $state("");
let keyStatus = $state<Record<string, boolean>>({});
let keyModal = $state<{ provider: string; hasKey: boolean } | null>(null);
let keyBusy = $state(false);
let keyError = $state("");
let activeProvider = $state("ollama");
let activeModel = $state("");
let contextUsed = $state(0);
let contextWindow = $state<number | null>(null);
let gesturesOn = $state<boolean>(true);
let draft = $state("");
let voiceError = $state("");
// eslint-disable-next-line @typescript-eslint/no-explicit-any
let recognition: any = null;

function defaultModelFor(list: ProviderInfo[], id: string): string {
  const p = list.find((x) => x.id === id);
  return p?.default_model || p?.models[0] || "";
}

function syncContextWindow(): void {
  contextWindow =
    providers.find((p) => p.id === activeProvider)?.context_window ?? null;
}

function toEvent(a: {
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

async function loadProviders(): Promise<void> {
  providersLoading = true;
  providersError = "";
  try {
    const res = await aiApi.providers();
    providers = res.providers;
    await refreshKeyStatus();
    try {
      const sel = await aiApi.selection();
      activeProvider = sel.provider;
      activeModel = sel.model || defaultModelFor(providers, activeProvider);
    } catch {
      /* first run: no selection stored yet */
    }
    syncContextWindow();
  } catch (err) {
    providersError = err instanceof Error ? err.message : "Error";
  } finally {
    providersLoading = false;
  }
}

async function selectProvider(id: string, model?: string | null): Promise<void> {
  selectError = "";
  const p = providers.find((x) => x.id === id);
  if (!p || !p.available) {
    selectError = t("providers.offline");
    return;
  }
  if (p.needs_key && !keyStatus[p.id]) {
    openKeyModal(id);
    return;
  }
  try {
    const res = await aiApi.select(id, model ?? undefined);
    activeProvider = res.provider;
    activeModel = res.model || defaultModelFor(providers, activeProvider);
    syncContextWindow();
  } catch (err) {
    selectError = err instanceof Error ? err.message : "Error";
  }
}

async function selectModel(m: string): Promise<void> {
  await selectProvider(activeProvider, m);
}

async function refreshKeyStatus(): Promise<void> {
  try {
    const res = await aiApi.keyStatus();
    const next: Record<string, boolean> = {};
    for (const k of res.keys) next[k.provider] = k.has_key;
    keyStatus = next;
  } catch {
    /* key status is advisory; providers still list */
  }
}

function openKeyModal(provider: string): void {
  keyError = "";
  keyModal = { provider, hasKey: !!keyStatus[provider] };
}

function closeKeyModal(): void {
  keyModal = null;
  keyError = "";
}

async function saveKey(provider: string, key: string): Promise<void> {
  keyBusy = true;
  keyError = "";
  try {
    const res = await aiApi.saveKey(provider, key);
    await refreshKeyStatus();
    // Fresh live catalog now that the key exists, then pre-select a model
    // that actually answers — the dropdown keeps offering every model.
    await loadProviders();
    closeKeyModal();
    await selectProvider(provider, res.suggested_model ?? undefined);
  } catch (err) {
    keyError = err instanceof Error ? err.message : "Error";
  } finally {
    keyBusy = false;
  }
}

async function deleteKey(provider: string): Promise<void> {
  keyBusy = true;
  keyError = "";
  try {
    await aiApi.deleteKey(provider);
    await refreshKeyStatus();
    closeKeyModal();
  } catch (err) {
    keyError = err instanceof Error ? err.message : "Error";
  } finally {
    keyBusy = false;
  }
}

function activeModels(): string[] {
  return providers.find((p) => p.id === activeProvider)?.models ?? [];
}

function activeAvailable(): boolean {
  return providers.find((p) => p.id === activeProvider)?.available ?? false;
}

async function refreshSessions(): Promise<void> {
  try {
    sessions = (await aiApi.sessions()).sessions;
    sessionsError = "";
  } catch (err) {
    sessionsError = err instanceof Error ? err.message : "Error";
  }
}

function newChat(): void {
  activeSessionId = null;
  messages = greeting();
  events = [];
}

async function openSession(id: string): Promise<void> {
  const d = await aiApi.sessionDetail(id);
  activeSessionId = id;
  // Machine turns (role "tool") stay server-side; the chat shows people only.
  messages = d.messages
    .filter((m) => m.role !== "tool")
    .map((m) => ({
      role: m.role === "assistant" ? "assistant" : "user",
      text: m.content,
    }));
  events = d.actions.map(toEvent);
  // Adopt the session's combo so follow-ups keep its context.
  await selectProvider(d.session.provider, d.session.model);
}

async function deleteSession(id: string): Promise<void> {
  await aiApi.deleteSession(id);
  sessions = sessions.filter((s) => s.id !== id);
  if (activeSessionId === id) newChat();
}

function setDraft(v: string): void {
  draft = v;
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
    const res = await aiApi.run(clean, activeSessionId, { lang: getLang() });
    activeSessionId = res.session_id;
    contextUsed = res.context.used_tokens;
    if (res.context.window != null) contextWindow = res.context.window;
    const d = await aiApi.sessionDetail(res.session_id);
    const steps = (res.steps ?? []).map((s) => ({ tool: s.tool, ok: s.ok }));
    // Machine turns (role "tool") never render as chat bubbles: without
    // this filter they show up as raw-JSON user messages.
    messages = d.messages
      .filter((m) => m.role !== "tool")
      .map((m, i, arr) => ({
        role: m.role === "assistant" ? "assistant" : "user",
        text: m.content,
        ...(i === arr.length - 1 && steps.length > 0 ? { steps } : {}),
      }));
    events = d.actions.map(toEvent);
    await refreshSessions();
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Error";
    messages = [...messages, { role: "assistant", text: `Error: ${msg}` }];
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
  get sessions(): SessionSummary[] {
    return sessions;
  },
  get sessionsError(): string {
    return sessionsError;
  },
  get activeSessionId(): string | null {
    return activeSessionId;
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
  get keyStatus(): Record<string, boolean> {
    return keyStatus;
  },
  get keyModal(): { provider: string; hasKey: boolean } | null {
    return keyModal;
  },
  get keyBusy(): boolean {
    return keyBusy;
  },
  get keyError(): string {
    return keyError;
  },
  get activeProvider(): string {
    return activeProvider;
  },
  get activeModel(): string {
    return activeModel;
  },
  get contextUsed(): number {
    return contextUsed;
  },
  get contextWindow(): number | null {
    return contextWindow;
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
  refreshSessions,
  selectProvider,
  selectModel,
  refreshKeyStatus,
  openKeyModal,
  closeKeyModal,
  saveKey,
  deleteKey,
  newChat,
  openSession,
  deleteSession,
  toggleGestures(): void {
    gesturesOn = !gesturesOn;
  },
  toggleListening,
  send,
};
