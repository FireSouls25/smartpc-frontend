import { api } from "../../lib/api";

export interface ProviderInfo {
  id: string;
  name: string;
  available: boolean;
  models: string[];
  default_model: string;
}

export interface ProvidersResponse {
  providers: ProviderInfo[];
  active: { provider: string; model: string | null };
}

export interface ChatMessage {
  role: "system" | "user" | "assistant";
  content: string;
}

export const aiApi = {
  providers: () => api<ProvidersResponse>("/v1/ai/providers"),
  select: (provider: string, model?: string) =>
    api<{ provider: string; model: string | null }>("/v1/ai/select", {
      method: "POST",
      body: { provider, model },
    }),
  chat: (
    messages: ChatMessage[],
    opts?: { provider?: string; model?: string },
  ) =>
    api<{ reply: string; model: string; provider: string }>("/v1/ai/chat", {
      method: "POST",
      body: { provider: opts?.provider, model: opts?.model, messages },
    }),
};
