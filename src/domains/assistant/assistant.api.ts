import { api } from "../../lib/api";
import { auth } from "../auth/auth.store.svelte";

export interface ProviderInfo {
  id: string;
  name: string;
  available: boolean;
  models: string[];
  default_model: string;
}

export interface SessionSummary {
  id: string;
  title: string;
  provider: string;
  model: string | null;
  updated_at: string;
  preview: string | null;
  message_count: number;
}

export interface SessionMessage {
  id: string;
  role: string;
  content: string;
  created_at: string;
}

export interface SessionAction {
  id: string;
  session_id: string | null;
  kind: string;
  title: string;
  status: string;
  created_at: string;
  updated_at: string;
}

function withAuth(token?: string): { token?: string } {
  return token ? { token } : {};
}

export const aiApi = {
  providers: () =>
    api<{ providers: ProviderInfo[] }>("/v1/ai/providers"),

  selection: () =>
    api<{ provider: string; model: string | null }>(
      "/v1/ai/selection",
      withAuth(auth.token ?? undefined),
    ),

  select: (provider: string, model?: string) =>
    api<{ provider: string; model: string | null }>("/v1/ai/select", {
      method: "POST",
      body: { provider, model },
      ...withAuth(auth.token ?? undefined),
    }),

  /** Persisted chat turn. Creates the session when sessionId is null. */
  chat: (
    message: string,
    sessionId: string | null,
    opts?: { provider?: string; model?: string },
  ) =>
    api<{ reply: string; model: string; provider: string; session_id: string }>(
      "/v1/ai/chat",
      {
        method: "POST",
        body: {
          session_id: sessionId,
          provider: opts?.provider,
          model: opts?.model,
          message,
        },
        ...withAuth(auth.token ?? undefined),
      },
    ),

  sessions: () =>
    api<{ sessions: SessionSummary[] }>(
      "/v1/chat/sessions",
      withAuth(auth.token ?? undefined),
    ),

  sessionDetail: (id: string) =>
    api<{
      session: SessionSummary & { created_at: string };
      messages: SessionMessage[];
      actions: SessionAction[];
    }>(`/v1/chat/sessions/${id}`, withAuth(auth.token ?? undefined)),

  deleteSession: (id: string) =>
    api<{ ok: boolean }>(`/v1/chat/sessions/${id}`, {
      method: "DELETE",
      ...withAuth(auth.token ?? undefined),
    }),

  actions: (sessionId: string) =>
    api<{ actions: SessionAction[] }>(
      `/v1/actions?session_id=${encodeURIComponent(sessionId)}`,
      withAuth(auth.token ?? undefined),
    ),
};
