// Dev runner: builds the Rust sidecar, starts Vite, then opens Electron
// (main spawns the sidecar itself). The app opens in its OWN desktop window;
// no browser needed. Killing Electron (or Ctrl+C) stops vite.
import { spawn } from "node:child_process";

const shell = process.platform === "win32";
const HOST = "127.0.0.1";
const PORT = 5173;

console.log("[dev] building sidecar (cargo)…");
await new Promise((resolve, reject) => {
  const build = spawn(
    "cargo",
    ["build", "--manifest-path", "src/native/Cargo.toml"],
    { stdio: "inherit", shell },
  );
  build.on("exit", (code) =>
    code === 0 ? resolve() : reject(new Error("cargo build failed")),
  );
  build.on("error", reject);
});

const vite = spawn(
  "node_modules/.bin/vite",
  ["--port", String(PORT), "--host", HOST, "--strictPort"],
  { stdio: "inherit", shell },
);

async function waitFor(url, tries = 60) {
  for (let i = 0; i < tries; i++) {
    try {
      await fetch(url);
      return;
    } catch {
      await new Promise((r) => setTimeout(r, 500));
    }
  }
  throw new Error("vite did not start at " + url);
}

function shutdown(code = 0) {
  vite.kill();
  process.exit(code);
}

try {
  await waitFor(`http://${HOST}:${PORT}/`);
  console.log("[dev] vite ready → launching Electron (desktop window)…");
  const app = spawn("node_modules/.bin/electron", ["."], {
    stdio: "inherit",
    shell,
    env: { ...process.env, VITE_DEV_URL: `http://${HOST}:${PORT}` },
  });
  app.on("exit", (code) => {
    if (code !== 0 && code !== null) {
      console.error(
        `[dev] electron exited with code ${code}. On Linux this usually means ` +
          `missing system libraries (libgtk-3-0, libnss3, libasound2, libxss1…).`,
      );
    }
    shutdown(code ?? 0);
  });
  process.on("SIGINT", () => {
    app.kill();
    shutdown();
  });
} catch (err) {
  console.error(err);
  shutdown(1);
}
