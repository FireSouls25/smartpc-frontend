import { test, expect } from "@playwright/test";
import { APP, SIDECAR, GATE, seedUser } from "./helpers";

test.beforeEach(async ({ request, context }) => seedUser(request, context));

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
  await page.screenshot({ path: "test-results/layout-chat.png" });
});

test(
  "real send persists and appears in sessions",
  { tag: "@slow" },
  async ({ page }) => {
    // Cold VRAM loads of multi-GB models are slow: allow generously.
    test.setTimeout(360000);
    await page.goto(APP, { waitUntil: "networkidle" });
    await expect(page.getByText("Chats").first()).toBeVisible({ timeout: 20000 });

    // Use a model that truly exists here (the default may not be installed).
    const detected = await page.evaluate(
      async ({ sidecar, gate }) => {
        const r = await fetch(`${sidecar}/v1/ai/providers`, {
          headers: { "X-Sidecar-Token": gate },
        });
        return (await r.json()) as {
          providers: { id: string; available: boolean; models: string[] }[];
        };
      },
      { sidecar: SIDECAR, gate: GATE },
    );
    const ollama = detected.providers.find((p) => p.id === "ollama");
    test.skip(
      !ollama?.available || ollama.models.length === 0,
      "no local model available",
    );
    const model = ollama?.models[0];
    if (!model) throw new Error("no local model available");
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
    await page.screenshot({ path: "test-results/layout-main.png" });
  },
);
