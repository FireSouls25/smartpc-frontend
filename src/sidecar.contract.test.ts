import { afterAll, beforeAll, describe, expect, test } from "vitest";
import { spawn, type ChildProcess } from "node:child_process";
import os from "node:os";
import path from "node:path";
import { SIDECAR_PROTOCOL } from "./lib/api";

// Renderer↔sidecar contract: shapes and envelopes both sides rely on, plus
// the protocol pin (imported from the renderer's own api module, so either
// side drifting fails this). Only deterministic routes: nothing here depends
// on models being installed or servers running.
const PORT = Number(process.env.CONTRACT_SIDECAR_PORT ?? 18082);
const TOKEN = "contract-token-min-16-chars!!";
const BASE = `http://127.0.0.1:${PORT}`;
const gate = { "X-Sidecar-Token": TOKEN, "Content-Type": "application/json" };

let child: ChildProcess | null = null;

async function waitForHealth(deadlineMs = 300000): Promise<void> {
  const start = Date.now();
  for (;;) {
    try {
      const res = await fetch(`${BASE}/health`);
      if (res.ok) return;
    } catch {
      /* not up yet */
    }
    if (Date.now() - start > deadlineMs)
      throw new Error("sidecar did not boot");
    await new Promise((r) => setTimeout(r, 500));
  }
}

beforeAll(async () => {
  const db = path.join(os.tmpdir(), `smartpc-contract-${process.pid}.db`);
  try {
    await import("node:fs/promises").then((fs) =>
      fs.unlink(db).catch(() => {}),
    );
  } catch {
    /* fresh file anyway */
  }
  child = spawn(
    "cargo",
    [
      "run",
      "--manifest-path",
      path.resolve(process.cwd(), "src/native/Cargo.toml"),
      "--",
      "--port",
      String(PORT),
      "--token",
      TOKEN,
      "--db",
      db,
    ],
    { cwd: process.cwd(), stdio: "ignore" },
  );
  await waitForHealth();
}, 600000);

afterAll(() => {
  child?.kill("SIGTERM");
  child = null;
});

describe("sidecar contract", () => {
  test("health carries the renderer protocol", async () => {
    const res = await fetch(`${BASE}/health`);
    expect(res.ok).toBe(true);
    const body = (await res.json()) as { status: string; protocol: number };
    expect(body.status).toBe("ok");
    expect(body.protocol).toBe(SIDECAR_PROTOCOL);
  });

  test("sidecar gate rejects without token", async () => {
    const res = await fetch(`${BASE}/v1/ai/providers`);
    expect(res.status).toBe(401);
    const body = (await res.json()) as { error: { code: string } };
    expect(body.error.code).toBe("unauthorized");
  });

  test("providers list the known catalog with stable shapes", async () => {
    const res = await fetch(`${BASE}/v1/ai/providers`, { headers: gate });
    expect(res.ok).toBe(true);
    const body = (await res.json()) as {
      providers: Record<string, unknown>[];
    };
    const ids = body.providers.map((p) => p["id"]).sort();
    expect(ids).toEqual(["llama.cpp", "ollama", "opencode"]);
    for (const p of body.providers) {
      expect(typeof p["id"]).toBe("string");
      expect(typeof p["name"]).toBe("string");
      expect(typeof p["available"]).toBe("boolean");
      expect(Array.isArray(p["models"])).toBe(true);
      expect(typeof p["default_model"]).toBe("string");
      expect(typeof p["needs_key"]).toBe("boolean");
      expect(
        typeof p["context_window"] === "number" || p["context_window"] === null,
      ).toBe(true);
      expect(typeof p["startable"]).toBe("boolean");
      expect(
        typeof p["installed"] === "boolean" || p["installed"] === null,
      ).toBe(true);
    }
  });

  test("auth failures use the error envelope", async () => {
    const res = await fetch(`${BASE}/v1/auth/login`, {
      method: "POST",
      headers: gate,
      body: JSON.stringify({
        email: "nobody@test.co",
        password: "x".repeat(20),
      }),
    });
    expect(res.status).toBe(401);
    const body = (await res.json()) as {
      error: { code: string; message: string };
    };
    expect(typeof body.error.code).toBe("string");
    expect(typeof body.error.message).toBe("string");
  });

  test("register validation uses the envelope, creates nothing", async () => {
    const res = await fetch(`${BASE}/v1/auth/register`, {
      method: "POST",
      headers: gate,
      body: JSON.stringify({ email: "not-an-email", password: "short" }),
    });
    expect(res.status).toBe(400);
    const body = (await res.json()) as { error: { code: string } };
    expect(body.error.code).toBe("validation");
  });

  test("user routes require a bearer token", async () => {
    const res = await fetch(`${BASE}/v1/chat/sessions`, { headers: gate });
    expect(res.status).toBe(401);
    const body = (await res.json()) as { error: { code: string } };
    expect(body.error.code).toBe("unauthorized");
  });

  test("unstartable providers fail closed, deterministically", async () => {
    for (const id of ["llama.cpp", "nope"]) {
      const res = await fetch(
        `${BASE}/v1/ai/providers/${encodeURIComponent(id)}/start`,
        { method: "POST", headers: gate },
      );
      expect(res.status).toBe(400);
      const body = (await res.json()) as { error: { code: string } };
      expect(body.error.code).toBe("not_startable");
    }
  });
});
