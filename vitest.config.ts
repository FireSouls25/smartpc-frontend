import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vitest/config";

// Unit-test runtime: node, no browser. The svelte plugin compiles
// `.svelte.ts` runes modules (stores) so pure helpers stay importable.
// Fast `unit` project needs nothing external; `contract` spawns its own
// sidecar (see the spec) so `npm run test:contract` is hermetic too.
export default defineConfig({
  plugins: [svelte()],
  test: {
    projects: [
      {
        test: {
          name: "unit",
          include: ["src/**/*.test.ts"],
          exclude: ["src/**/*.contract.test.ts"],
          environment: "node",
        },
      },
      {
        test: {
          name: "contract",
          include: ["src/**/*.contract.test.ts"],
          environment: "node",
        },
      },
    ],
  },
});
