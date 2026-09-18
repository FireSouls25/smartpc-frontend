import { api } from "../../lib/api";

export type VoiceMode = "manual" | "wake";

export interface VoiceStatus {
  listening: boolean;
  capturing: boolean;
  mode: VoiceMode | null;
  model: string;
  wake_word: string | null;
  mic: boolean;
  model_ready: boolean;
}

export type VoiceEvent =
  | { seq: number; type: "capturing"; active: boolean }
  | { seq: number; type: "wake"; word: string }
  | { seq: number; type: "transcript"; text: string }
  | { seq: number; type: "error"; code: string; message: string }
  | { seq: number; type: "end" };

export const voiceApi = {
  status: () => api<VoiceStatus>("/v1/voice/status"),

  /** Blocks on first-use model download — generous budget. */
  listen: (opts: { mode: VoiceMode; wake_word: string; lang: string }) =>
    api<{ ok: boolean }>("/v1/voice/listen", {
      method: "POST",
      body: opts,
      timeoutMs: 180000,
    }),

  stop: () =>
    api<{ ok: boolean }>("/v1/voice/stop", {
      method: "POST",
      timeoutMs: 10000,
    }),

  /** Long-poll: the server holds up to ~25 s for news. */
  poll: (cursor: number) =>
    api<{ events: VoiceEvent[]; next: number }>(
      `/v1/voice/events?cursor=${cursor}`,
      { timeoutMs: 35000 },
    ),
};
