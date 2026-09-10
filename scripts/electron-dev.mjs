// Dev runner: builds the Rust sidecar, starts Vite, then opens Electron
// (main spawns the sidecar itself). Killing Electron (or Ctrl+C) stops vite.
import { spawn } from "node:child_process";
import net from "node:net";

const shell = process.platform === "win32";

console.log("[dev] building sidecar (cargo)…");
await new Promise((resolve, reject) => {
  const build = spawn("cargo", ["build", "--manifest-path", "src/native/Cargo.toml"], {
    stdio: "inherit",
    shell,
  });
  build.on("exit", (code) =>
    code === 0 ? resolve() : reject(new Error("cargo build failed")),
  );
  build.on("error", reject);
});

const PORT = 5173;
const vite = spawn("node_modules/.bin/vite", ["--port", String(PORT)], {
  stdio: "inherit",
  shell,
});

function waitFor(port, tries = 60) {
  return new Promise((resolve, reject) => {
    const attempt = (left) => {
      const sock = net.connect(port, "127.0.0.1");
      sock.on("connect", () => {
        sock.end();
        resolve();
      });
      sock.on("error", () => {
        if (left <= 0) reject(new Error("vite did not start"));
        else setTimeout(() => attempt(left - 1), 500);
      });
    };
    attempt(tries);
  });
}

function shutdown() {
  vite.kill();
  process.exit(0);
}

try {
  await waitFor(PORT);
  const app = spawn("node_modules/.bin/electron", ["."], {
    stdio: "inherit",
    shell,
    env: { ...process.env, VITE_DEV_URL: `http://localhost:${PORT}` },
  });
  app.on("exit", shutdown);
  process.on("SIGINT", () => {
    app.kill();
    shutdown();
  });
} catch (err) {
  console.error(err);
  shutdown();
}
