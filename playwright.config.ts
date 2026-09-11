import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  timeout: 120000,
  use: {
    channel: "chromium-headless-shell",
    viewport: { width: 1600, height: 900 },
  },
});
