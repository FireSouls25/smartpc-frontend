import type { I18nKey } from "../../lib/i18n.svelte";

export type OrbState = "idle" | "listening" | "thinking";

export interface ChatMsg {
  id: number;
  role: "user" | "assistant";
  text?: string;
  textKey?: I18nKey;
}

export interface AppEvent {
  id: number;
  titleKey: I18nKey;
  status: "running" | "done";
  /** One-shot actions finish; continuous ones (slides, dictation…) stay running. */
  continuous: boolean;
}

export const PROVIDERS = [
  { id: "groq", models: ["llama-3.3-70b", "mixtral-8x7b"] },
  { id: "openrouter", models: ["auto", "claude-haiku", "gpt-4o-mini"] },
  { id: "ollama", models: ["llama3.1:8b", "qwen2.5:7b"] },
] as const;

let seq = 1;
let orb = $state<OrbState>("idle");
let messages = $state<ChatMsg[]>([
  { id: 0, role: "assistant", textKey: "chat.hello" },
]);
let events = $state<AppEvent[]>([
  { id: 1, titleKey: "events.volume", status: "done", continuous: false },
  { id: 2, titleKey: "events.slides", status: "running", continuous: true },
]);
let provider = $state<string>("groq");
let model = $state<string>("llama-3.3-70b");
let gesturesOn = $state<boolean>(true);

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
  get provider(): string {
    return provider;
  },
  get model(): string {
    return model;
  },
  get gesturesOn(): boolean {
    return gesturesOn;
  },
  get listening(): boolean {
    return orb === "listening";
  },

  setProvider(p: string): void {
    provider = p;
    const found = PROVIDERS.find((x) => x.id === p);
    if (found) model = found.models[0];
  },
  setModel(m: string): void {
    model = m;
  },
  toggleGestures(): void {
    gesturesOn = !gesturesOn;
  },

  /** Mic button (and, later, the wake-word detector) drives this. */
  toggleListening(): void {
    if (orb === "thinking") return;
    orb = orb === "listening" ? "idle" : "listening";
  },

  /** Mock send: user msg → thinking → reply + completed event. */
  send(text: string): void {
    const clean = text.trim();
    if (!clean || orb === "thinking") return;
    messages = [...messages, { id: seq++, role: "user", text: clean }];
    orb = "thinking";
    const ev: AppEvent = {
      id: seq++,
      titleKey: "events.openBrowser",
      status: "running",
      continuous: false,
    };
    events = [ev, ...events];
    window.setTimeout(() => {
      messages = [
        ...messages,
        { id: seq++, role: "assistant", textKey: "chat.mockReply" },
      ];
      events = events.map((e) =>
        e.id === ev.id ? { ...e, status: "done" } : e,
      );
      orb = "idle";
    }, 1800);
  },
};
