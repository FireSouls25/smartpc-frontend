import { test, expect } from "@playwright/test";
import { APP } from "./helpers";

// Boot smoke test: the app hydrates with zero page errors. No auth needed —
// an anonymous boot lands on login, which must still render cleanly.
test("boot renders without page errors", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(String(e).slice(0, 300)));
  await page.goto(APP, { waitUntil: "networkidle" });
  await expect(page.locator("#app")).not.toBeEmpty({ timeout: 20000 });
  expect(errors).toEqual([]);
});
