import { test, expect } from "@playwright/test";

// Needs the sidecar on :18088. No model and no real key needed: saving a
// bogus key must fail with a clear error inside the modal.
const APP = "http://127.0.0.1:5199";
const SIDECAR = "http://127.0.0.1:18081";
const GATE = "dev-token-min-16-chars";

test.beforeEach(async ({ context, request }) => {
  const tag = `k${Date.now()}${Math.floor(Math.random() * 1e6)}`;
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

test("opencode without key opens the key modal; bogus key errors", async ({
  page,
}) => {
  await page.goto(APP, { waitUntil: "networkidle" });
  await expect(page.getByText("Chats").first()).toBeVisible({ timeout: 20000 });

  const menus = page.locator("[data-selectmenu]");
  await menus.nth(0).getByRole("button").click();
  await page.getByRole("option", { name: "opencode" }).click();

  // Centered modal asking for the key (not a silent select).
  await expect(page.getByRole("dialog")).toBeVisible({ timeout: 10000 });
  await page.getByPlaceholder(/Pega tu API key|Paste your API key/).fill("badkey12");
  await page.getByRole("button", { name: /Guardar|Save/ }).click();
  await expect(page.getByText(/rejected|rechazado/i).first()).toBeVisible({
    timeout: 60000,
  });
  await page.screenshot({ path: "/tmp/layout-keymodal.png" });
});
