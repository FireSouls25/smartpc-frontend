import { test } from "@playwright/test";

test("probe boot", async ({ page }) => {
  const logs: string[] = [];
  page.on("console", (m) => logs.push(`${m.type()}: ${m.text().slice(0, 160)}`));
  page.on("pageerror", (e) => logs.push(`PAGEERROR: ${String(e).slice(0, 300)}`));
  await page.goto("http://127.0.0.1:5199", { waitUntil: "networkidle" });
  await page.waitForTimeout(5000);
  console.log("LOGS:\n" + (logs.join("\n") || "(none)"));
  console.log(
    "BODYLEN:",
    await page.evaluate(() => document.body.innerHTML.length),
  );
  await page.screenshot({ path: "/tmp/probe-boot.png" });
});
