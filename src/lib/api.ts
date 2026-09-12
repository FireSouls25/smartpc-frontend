// Minimal typed fetch wrapper. Talks to the Rust sidecar:
// - inside Electron: url + per-launch token come from the preload bridge
//   (main spawns the sidecar and injects them).
// - plain web dev: VITE_API_URL / VITE_SIDECAR_TOKEN (run the sidecar manually).
interface SidecarInfo {
  url: string;
  token: string;
}

function sidecar(): SidecarInfo | null {
  return window.smartpc?.sidecar ?? null;
}

function base(): string {
  return (
    sidecar()?.url ??
    import.meta.env.VITE_API_URL ??
    "http://127.0.0.1:18080"
  );
}

function sidecarToken(): string {
  return sidecar()?.token ?? import.meta.env.VITE_SIDECAR_TOKEN ?? "";
}

/** Must match the sidecar PROTOCOL const; the shell warns on mismatch. */
export const SIDECAR_PROTOCOL = 2;

export async function fetchHealth(): Promise<{
  status: string;
  protocol: number;
}> {
  const res = await fetch(base() + "/health");
  return (await res.json()) as { status: string; protocol: number };
}

export class ApiError extends Error {
  status: number;
  code?: string;
  constructor(status: number, message: string, code?: string) {
    super(message);
    this.status = status;
    this.code = code;
  }
}

interface Options {
  method?: "GET" | "POST" | "DELETE";
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  body?: any;
  token?: string;
}

export async function api<T>(path: string, opts: Options = {}): Promise<T> {
  const gate = sidecarToken();
  const res = await fetch(base() + path, {
    method: opts.method ?? "GET",
    headers: {
      "Content-Type": "application/json",
      ...(opts.token ? { Authorization: `Bearer ${opts.token}` } : {}),
      ...(gate ? { "X-Sidecar-Token": gate } : {}),
    },
    body: opts.body !== undefined ? JSON.stringify(opts.body) : undefined,
  });
  if (res.status === 204) return undefined as T;
  const data = await res.json().catch(() => null);
  if (!res.ok) {
    throw new ApiError(
      res.status,
      data?.error?.message ?? "Request failed",
      data?.error?.code,
    );
  }
  return data as T;
}
