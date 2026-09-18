import { aiApi, type ProviderInfo, type SessionSummary } from "./assistant.api";
import { ApiError } from "../../lib/api";
import { getLang, t, type I18nKey } from "../../lib/i18n.svelte";
export type OrbState = "idle" | "listening" | "thinking";

/** Provider availability re-check interval (see docs/07-provider-availability.md). */
export const PROVIDER_POLL_MS = 3000;

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
let sessions = $state<SessionSummary[]>([]);
let sessionsError = $state("");
let activeSessionId = $state<string | null>(null);
// Combo the viewed session was created with. Browsing history never mutates
// the global selection (that surprised); the UI offers an explicit adopt.
let sessionCombo = $state<{ provider: string; model: string | null } | null>(
  null,
);
let providers = $state<ProviderInfo[]>([]);
let providersLoading = $state(false);
let providersError = $state("");
let selectError = $state("");
let keyStatus = $state<Record<string, boolean>>({});
let keyModal = $state<{ provider: string; hasKey: boolean } | null>(null);
let keyBusy = $state(false);
let keyError = $state("");
let startingProvider = $state<string | null>(null);
let startError = $state("");
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

async function loadProviders(opts?: { silent?: boolean }): Promise<void> {
  const silent = opts?.silent ?? false;
  if (!silent) {
    providersLoading = true;
    providersError = "";
  }
  try {
    const res = await aiApi.providers();
    providers = res.providers;
    await refreshKeyStatus();
    if (!silent) {
      try {
        const sel = await aiApi.selection();
        activeProvider = sel.provider;
        activeModel = sel.model || defaultModelFor(providers, activeProvider);
      } catch {
        /* first run: no selection stored yet */
      }
    }
    syncContextWindow();
  } catch (err) {
    if (!silent) providersError = err instanceof Error ? err.message : "Error";
  } finally {
    if (!silent) providersLoading = false;
  }
}

// Availability watch: re-probes providers every PROVIDER_POLL_MS so a server
// started after boot (e.g. `ollama serve` in another terminal) flips the UI
// from "not detected" to usable without a manual refresh. Silent on purpose:
// no loading flicker, no error surfaces, no selection clobbering (the server
// selection is only read on the initial load; user changes go through
// selectProvider which persists + updates local state itself).
let pollTimer: number | null = null;
let pollInFlight = false;

async function pollProviders(): Promise<void> {
  if (pollInFlight) return;
  if (typeof document !== "undefined" && document.hidden) return;
  pollInFlight = true;
  try {
    await loadProviders({ silent: true });
  } catch {
    /* silent: the next tick retries */
  } finally {
    pollInFlight = false;
  }
}

function startProviderWatch(): void {
  if (pollTimer !== null || typeof window === "undefined") return;
  void pollProviders();
  pollTimer = window.setInterval(() => void pollProviders(), PROVIDER_POLL_MS);
}

function stopProviderWatch(): void {
  if (pollTimer !== null) {
    window.clearInterval(pollTimer);
    pollTimer = null;
  }
}

function startErrorFor(code: string | undefined): I18nKey {
  if (code === "not_installed") return "providers.notInstalled";
  if (code === "start_timeout" || code === "timeout") return "providers.startTimeout";
  return "providers.startFailed";
}

/** Ask the sidecar to launch a startable server; the watch picks it up. */
async function startProvider(id: string): Promise<void> {
  if (startingProvider) return;
  startingProvider = id;
  startError = "";
  try {
    await aiApi.startProvider(id);
    await loadProviders({ silent: true });
  } catch (err) {
    const code = err instanceof ApiError ? err.code : undefined;
    startError = t(startErrorFor(code));
  } finally {
    startingProvider = null;
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
    // The verify response already carries the fresh catalog for this
    // provider — fold it into local state instead of refetching the whole
    // list (was: keyStatus + full load incl. selection + keyStatus again).
    keyStatus = { ...keyStatus, [provider]: true };
    if (res.models.length > 0) {
      providers = providers.map((p) =>
        p.id === provider
          ? { ...p, available: true, models: [...res.models] }
          : p,
      );
      syncContextWindow();
    }
    closeKeyModal();
    // Persist the pre-selection for a model that actually answers — the
    // dropdown keeps offering every catalog model.
    if (!providers.some((p) => p.id === provider)) {
      await loadProviders({ silent: true });
    }
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
    keyStatus = { ...keyStatus, [provider]: false };
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
  sessionCombo = null;
  messages = greeting();
  events = [];
}

async function openSession(id: string): Promise<void> {
  const d = await aiApi.sessionDetail(id);
  activeSessionId = id;
  sessionCombo = { provider: d.session.provider, model: d.session.model };
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
  if (!sessionCombo) return;
  await selectProvider(sessionCombo.provider, sessionCombo.model ?? undefined);
  sessionCombo = null;
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
        id: m.id,
        role: m.role === "assistant" ? "assistant" : "user",
        text: m.content,
        ...(i === arr.length - 1 && steps.length > 0 ? { steps } : {}),
      }));
    events = d.actions.map(toEvent);
    await refreshSessions();
    // The turn above ran under the active combo, so the viewed session now
    // continues under it too — the adopt affordance has served its purpose.
    sessionCombo = null;
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
  get sessionCombo(): { provider: string; model: string | null } | null {
    return sessionCombo;
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
  get startingProvider(): string | null {
    return startingProvider;
  },
  get startError(): string {
    return startError;
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
  startProviderWatch,
  stopProviderWatch,
  startProvider,
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
  adoptSessionCombo,
  deleteSession,
  toggleGestures(): void {
    gesturesOn = !gesturesOn;
  },
  toggleListening,
  send,
};
