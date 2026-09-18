import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { ApiError, api } from "./api";

const json = (body: unknown, status = 200): Response =>
  new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });

describe("api()", () => {
  const fetchMock = vi.fn();

  beforeEach(() => {
    fetchMock.mockReset();
    // No Electron bridge: falls back to env, then loopback default.
    // Pin the env so a local .env can't move the assertions.
    vi.stubEnv("VITE_API_URL", "http://127.0.0.1:18080");
    vi.stubEnv("VITE_SIDECAR_TOKEN", "");
    vi.stubGlobal("window", {});
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.unstubAllEnvs();
  });

  test("returns parsed data on 200", async () => {
    fetchMock.mockResolvedValue(json({ ok: true }));
    await expect(api("/v1/x")).resolves.toEqual({ ok: true });
    const [url] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("http://127.0.0.1:18080/v1/x");
  });

  test("204 resolves undefined without parsing", async () => {
    fetchMock.mockResolvedValue(new Response(null, { status: 204 }));
    await expect(api("/v1/x")).resolves.toBeUndefined();
  });

  test("error envelope maps to ApiError", async () => {
    fetchMock.mockResolvedValue(
      json({ error: { code: "invalid_credentials", message: "nope" } }, 401),
    );
    const err = await api("/v1/x").catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect(err).toMatchObject({
      status: 401,
      code: "invalid_credentials",
      message: "nope",
    });
  });

  test("non-JSON failure degrades to a generic message", async () => {
    fetchMock.mockResolvedValue(new Response("boom", { status: 500 }));
    const err = (await api("/v1/x").catch((e: unknown) => e)) as ApiError;
    expect(err.status).toBe(500);
    expect(err.code).toBeUndefined();
    expect(err.message).toBe("Request failed");
  });

  test("sends gate + bearer headers", async () => {
    vi.stubEnv("VITE_SIDECAR_TOKEN", "gate-123");
    fetchMock.mockResolvedValue(json({}));
    await api("/v1/x", { method: "POST", body: { a: 1 }, token: "user-abc" });
    const [, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    const headers = init.headers as Record<string, string>;
    expect(headers["X-Sidecar-Token"]).toBe("gate-123");
    expect(headers["Authorization"]).toBe("Bearer user-abc");
    expect(init.body).toBe(JSON.stringify({ a: 1 }));
  });

  test("TimeoutError maps to 504/timeout", async () => {
    fetchMock.mockRejectedValue(
      new DOMException("The operation timed out.", "TimeoutError"),
    );
    const err = (await api("/v1/x", { timeoutMs: 5 }).catch(
      (e: unknown) => e,
    )) as ApiError;
    expect(err).toMatchObject({ status: 504, code: "timeout" });
  });

  test("transport errors propagate untouched", async () => {
    const down = new Error("connection refused");
    fetchMock.mockRejectedValue(down);
    await expect(api("/v1/x")).rejects.toBe(down);
  });
});
