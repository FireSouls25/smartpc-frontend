import { expect, type APIRequestContext, type BrowserContext } from "@playwright/test";

// Same env contract as playwright.config.ts: no hardcoded ports in specs.
const APP_PORT = Number(process.env.APP_PORT ?? 5199);
const SIDECAR_PORT = Number(process.env.SIDECAR_PORT ?? 18081);

export const APP = process.env.APP_URL ?? `http://127.0.0.1:${APP_PORT}`;
export const SIDECAR =
  process.env.SIDECAR_URL ?? `http://127.0.0.1:${SIDECAR_PORT}`;
export const GATE = process.env.SIDECAR_TOKEN ?? "dev-token-min-16-chars";

/**
 * Fresh user per test + refresh token seeded into localStorage.
 * Fresh (not shared): refresh tokens rotate, so sharing one across tests
 * would trip reuse detection and kill the chain.
 */
export async function seedUser(
  request: APIRequestContext,
  context: BrowserContext,
): Promise<void> {
  const tag = `t${Date.now()}${Math.floor(Math.random() * 1e6)}`;
  const email = `${tag}@test.co`;
  const headers = {
    "Content-Type": "application/json",
    "X-Sidecar-Token": GATE,
  };
  await request.post(`${SIDECAR}/v1/auth/register`, {
    data: { email, password: "correct-horse-1" },
    headers,
  });
  const login = await request.post(`${SIDECAR}/v1/auth/login`, {
    data: { email, password: "correct-horse-1" },
    headers,
  });
  expect(login.ok()).toBeTruthy();
  const body = await login.json();
  await context.addInitScript((token: string) => {
    window.localStorage.setItem("smartpc.refresh", token);
  }, body.tokens.refresh_token as string);
}
