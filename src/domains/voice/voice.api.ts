import { api } from "../../lib/api";

export type VoiceMode = "manual" | "wake";

export interface VoiceStatus {
  listening: boolean;
  capturing: boolean;
  mode: VoiceMode | null;
  model: string;
  wake_word: string | null;
  mic: boolean;
  device: string | null;
  inputs: string[];
  model_ready: boolean;
}

export type VoiceEvent =
  | { seq: number; epoch: number; type: "started" }
  | { seq: number; epoch: number; type: "capturing"; active: boolean }
  | { seq: number; epoch: number; type: "wake"; word: string }
  | { seq: number; epoch: number; type: "transcript"; text: string }
  | { seq: number; epoch: number; type: "error"; code: string; message: string }
  | { seq: number; epoch: number; type: "end" };

export const voiceApi = {
  status: () => api<VoiceStatus>("/v1/voice/status"),

  /** Blocks on first-use model download — generous budget. */
  listen: (opts: {
    mode: VoiceMode;
    wake_word: string;
    lang: string;
    device?: string;
  }) =>
    api<{ ok: boolean; epoch: number }>("/v1/voice/listen", {
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
