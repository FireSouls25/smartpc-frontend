import { test, expect } from "@playwright/test";

// Needs the sidecar on :18081 (same as frontend/.env) — a real
// Ollama behind it makes test 2 meaningful, but it also passes degraded
// (failed event + error bubble) as long as the plumbing works.
const APP = "http://127.0.0.1:5199";
const SIDECAR = "http://127.0.0.1:18081";
const GATE = "dev-token-min-16-chars";

test.beforeEach(async ({ context, request }) => {
  // Fresh user + token per test: refresh tokens rotate, so sharing one
  // across tests would trip reuse detection and kill the chain.
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
  const body = await login.json();
  await context.addInitScript((token: string) => {
    window.localStorage.setItem("smartpc.refresh", token);
  }, body.tokens.refresh_token as string);
});

test("shell locks to viewport; panes scroll internally", async ({ page }) => {
  await page.goto(APP, { waitUntil: "networkidle" });
  // Logged in via seeded refresh token: main shell with its three panes.
  await expect(page.getByText("Chats").first()).toBeVisible({ timeout: 20000 });
  await expect(page.getByText("Actividad").first()).toBeVisible();

  const before = await page.evaluate(() => ({
    pageScrollH: document.documentElement.scrollHeight,
    innerH: window.innerHeight,
  }));
  expect(before.pageScrollH).toBeLessThanOrEqual(before.innerH + 1);

  // Simulate a long chat: inject 60 bubbles into the chat scroller
  // (the orb's status span shares the aria-live attr, hence the div scope).
  const scroller = page.locator('div[aria-live="polite"]');
  await scroller.evaluate((el) => {
    for (let i = 0; i < 60; i++) {
      const d = document.createElement("div");
      d.className = "msg-in flex justify-end";
      const p = document.createElement("p");
      p.className = "bubble-user";
      p.textContent = `mensaje de prueba ${i} con algo de texto para ocupar líneas y forzar un overflow real del contenedor`;
      d.appendChild(p);
      el.appendChild(d);
    }
  });
  await page.waitForTimeout(400);

  const after = await page.evaluate(() => {
    const el = document.querySelector(
      'div[aria-live="polite"]',
    ) as HTMLElement | null;
    return {
      pageScrollH: document.documentElement.scrollHeight,
      innerH: window.innerHeight,
      scrollH: el ? el.scrollHeight : -1,
      clientH: el ? el.clientHeight : -1,
    };
  });
  // Page must NOT grow; the scroller must absorb the overflow.
  expect(after.pageScrollH).toBeLessThanOrEqual(after.innerH + 1);
  expect(after.scrollH).toBeGreaterThan(after.clientH + 200);
  await page.screenshot({ path: "/tmp/layout-chat.png" });
});

test("real send persists and appears in sessions", async ({ page }) => {
  // Cold VRAM loads of multi-GB models are slow: allow generously.
  test.setTimeout(360000);
  await page.goto(APP, { waitUntil: "networkidle" });
  await expect(page.getByText("Chats").first()).toBeVisible({ timeout: 20000 });

  // Use a model that truly exists here (the default may not be installed).
  const detected = await page.evaluate(async () => {
    const r = await fetch("http://127.0.0.1:18081/v1/ai/providers", {
      headers: { "X-Sidecar-Token": "dev-token-min-16-chars" },
    });
    return (await r.json()) as {
      providers: { id: string; available: boolean; models: string[] }[];
    };
  });
  const ollama = detected.providers.find((p) => p.id === "ollama");
  test.skip(
    !ollama?.available || ollama.models.length === 0,
    "no local model available",
  );
  const model = ollama.models[0];
  const menus = page.locator("[data-selectmenu]");
  await menus.nth(1).getByRole("button").click();
  await page.getByRole("option", { name: model }).click();
  // The closed menu button must show the new model (proves select persisted).
  await expect(menus.nth(1).getByRole("button")).toContainText(model);

  const input = page.getByPlaceholder(/Pídele algo|Ask your PC/);
  await input.fill("Reply with exactly: hola-test");
  await input.press("Enter");
  // User bubble shows instantly; the reply needs real inference.
  await expect(page.getByText(/hola-test/).first()).toBeVisible({
    timeout: 10000,
  });
  // After the server round-trip the greeting is replaced by the real reply,
  // so there is exactly one assistant bubble: assert on its content.
  await expect(page.locator(".bubble-assistant").last()).toContainText(
    "hola-test",
    { timeout: 240000 },
  );
  const reply = await page.locator(".bubble-assistant").last().textContent();
  expect(reply).not.toMatch(/^Error:/);
  // A session shows up in the right pane.
  await expect(page.getByText(/Reply with exactly/i).first()).toBeVisible({
    timeout: 10000,
  });
  await page.screenshot({ path: "/tmp/layout-main.png" });
});
