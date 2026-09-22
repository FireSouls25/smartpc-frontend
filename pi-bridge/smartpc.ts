/**
 * Smart PC tool bridge for pi (`pi --mode rpc -e <this-file>`).
 *
 * Zero runtime dependencies by design: only a type-only import (erased by
 * jiti) plus global fetch. Tool schemas AND local-provider catalogs come
 * from the sidecar at startup, so this file never drifts from the Rust
 * catalog — adding a tool is one Rust entry, nothing here.
 *
 * Execution stays in Rust: each tool POSTs to the sidecar, which enforces
 * the risk policy and runs the OS call. This process never touches the
 * machine directly.
 */
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

interface Env {
  SMARTPC_SIDECAR_URL?: string;
  SMARTPC_SIDECAR_TOKEN?: string;
}

function env(): Env {
  const g = globalThis as unknown as {
    process?: { env?: Record<string, string | undefined> };
  };
  return (g.process?.env ?? {}) as Env;
}

const SIDECAR_URL = env().SMARTPC_SIDECAR_URL ?? "http://127.0.0.1:18080";
const SIDECAR_TOKEN = env().SMARTPC_SIDECAR_TOKEN ?? "";
const TIMEOUT_MS = 120000;

async function sidecar<T>(path: string, body: unknown): Promise<T> {
  const res = await fetch(`${SIDECAR_URL}${path}`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-Sidecar-Token": SIDECAR_TOKEN,
    },
    body: JSON.stringify(body),
    signal: AbortSignal.timeout(TIMEOUT_MS),
  });
  if (!res.ok) throw new Error(`smartpc bridge ${path}: ${res.status}`);
  return (await res.json()) as T;
}

interface BridgeToolDef {
  name: string;
  description: string;
  parameters: Record<string, unknown>;
}

/** Minimal surface we need from the tool-execution context. */
interface ToolContext {
  sessionManager?: {
    getSessionFile?: () => string | null;
  };
}

interface BridgeProvider {
  id: string;
  baseUrl: string;
  apiKey: string;
  api: string;
  models: Array<{
    id: string;
    name: string;
    reasoning: boolean;
    input: string[];
    cost: {
      input: number;
      output: number;
      cacheRead: number;
      cacheWrite: number;
    };
    contextWindow: number;
    maxTokens: number;
  }>;
}

export default async function (pi: ExtensionAPI) {
  // Local providers pi lacks (ids match the sidecar selection store 1:1).
  // Fetched live so model lists never go stale within a sidecar boot.
  try {
    const boot = await sidecar<{ providers: BridgeProvider[] }>(
      "/internal/pi/bootstrap",
      {},
    );
    for (const p of boot.providers) {
      pi.registerProvider(p.id, {
        baseUrl: p.baseUrl,
        apiKey: p.apiKey,
        api: p.api,
        models: p.models,
      });
    }
  } catch (err) {
    console.error(`[smartpc] provider bootstrap failed: ${String(err)}`);
  }

  // OS tools: schemas from Rust, execution back in Rust.
  const { tools } = await sidecar<{ tools: BridgeToolDef[] }>(
    "/internal/pi/tools",
    {},
  );
  for (const t of tools) {
    pi.registerTool({
      name: t.name,
      label: t.name,
      description: t.description,
      parameters: t.parameters,
      async execute(
        _toolCallId: string,
        params: unknown,
        _signal?: unknown,
        _onUpdate?: unknown,
        ctx?: ToolContext,
      ) {
        // Pi session file for user attribution server-side (the bridge
        // carries no user token by design).
        const pi_session = ctx?.sessionManager?.getSessionFile?.() ?? undefined;
        const out = await sidecar<{ ok: boolean; output: string }>(
          "/internal/pi/tool",
          { pi_session, name: t.name, args: params ?? {} },
        );
        // Failures arrive as results (policy blocks, bad args): the model
        // sees them and continues, exactly like the native harness.
        return {
          content: [{ type: "text", text: out.output }],
          details: { ok: out.ok },
        };
      },
    });
  }
}
