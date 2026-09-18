// Kills stray Smart PC dev processes (current user only, exact matches).
// The hardened dev runner normally cleans up after itself; this is the
// manual escape hatch: `npm run dev:clean`.
import { execSync } from "node:child_process";

const patterns = [
  /vite\s.*--port\s*5173/, // our vite only (port-pinned)
  /node_modules\/\.bin\/electron/, // electron binary (not editors)
  /smartpc-native/, // rust sidecar
];

let rows = [];
try {
  const out = execSync("ps -eo pid,args", { encoding: "utf8" });
  rows = out.split("\n").slice(1);
} catch {
  console.error("dev:clean needs `ps` (non-Windows).");
  process.exit(1);
}

const skip = new Set([process.pid, process.ppid]);
let killed = 0;
for (const row of rows) {
  const m = row.trim().match(/^(\d+)\s+(.*)$/);
  if (!m) continue;
  const pid = Number(m[1]);
  if (skip.has(pid)) continue;
  if (patterns.some((re) => re.test(m[2]))) {
    try {
      process.kill(pid, "SIGTERM");
      killed++;
      console.log(`signaled ${pid} (${m[2].slice(0, 90)})`);
    } catch (e) {
      console.error(`could not signal ${pid}: ${e.message}`);
    }
  }
}
console.log(
  killed ? `done, ${killed} process(es) signaled` : "nothing stray running",
);
