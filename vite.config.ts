import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  // react() compiles the Rare UI islands (.tsx); no fast-refresh needed there.
  plugins: [svelte(), react(), tailwindcss()],
  server: { port: 5173 },
});
