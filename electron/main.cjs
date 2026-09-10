// Smart PC — Electron main process (Node side, privileged).
// Spawns the Rust sidecar (local backend) and shows its UI.
// The renderer NEVER gets raw Node APIs: only window.smartpc from preload.cjs.
const { app, BrowserWindow, dialog, ipcMain } = require("electron");
const path = require("node:path");
const { spawn } = require("node:child_process");
const crypto = require("node:crypto");

const isDev = !app.isPackaged;
let sidecar = null;

function sidecarBin() {
  const name =
    process.platform === "win32" ? "smartpc-native.exe" : "smartpc-native";
  if (isDev) {
    return path.join(
      __dirname,
      "..",
      "src",
      "native",
      "target",
      "debug",
      name,
    );
  }
  return path.join(process.resourcesPath, "bin", name);
}

// Starts the sidecar with a random per-launch token on an OS-assigned port.
// Resolves once the binary prints its READY line.
async function startSidecar() {
  const bin = sidecarBin();
  const token = crypto.randomBytes(32).toString("hex");
  const db = path.join(app.getPath("userData"), "smartpc.db");
  const child = spawn(bin, ["--port", "0", "--token", token, "--db", db], {
    stdio: ["ignore", "pipe", "inherit"],
  });
  const url = await new Promise((resolve, reject) => {
    const timer = setTimeout(
      () => reject(new Error("timed out waiting for READY")),
      20000,
    );
    let buf = "";
    child.stdout.on("data", (d) => {
      buf += d.toString();
      const m = buf.match(/READY port=(\d+)/);
      if (m) {
        clearTimeout(timer);
        resolve(`http://127.0.0.1:${m[1]}`);
      }
    });
    child.on("error", (err) => {
      clearTimeout(timer);
      reject(err);
    });
    child.on("exit", (code) => {
      clearTimeout(timer);
      reject(new Error("sidecar exited with code " + code));
    });
  });
  process.env.SIDECAR_URL = url;
  process.env.SIDECAR_TOKEN = token;
  return child;
}

function createWindow() {
  const win = new BrowserWindow({
    width: 1280,
    height: 860,
    minWidth: 1024,
    minHeight: 680,
    autoHideMenuBar: true,
    backgroundColor: "#eff1f5",
    webPreferences: {
      preload: path.join(__dirname, "preload.cjs"),
      contextIsolation: true,
      sandbox: true,
      nodeIntegration: false,
    },
  });

  if (isDev) {
    win.loadURL(process.env.VITE_DEV_URL || "http://localhost:5173");
  } else {
    win.loadFile(path.join(__dirname, "../dist/index.html"));
  }
  return win;
}

// System channels. Heavy local work already lives in the sidecar process;
// executor/mic/vault IPC lands here in later iterations.
ipcMain.handle("system:ping", () => ({
  ok: true,
  at: new Date().toISOString(),
}));

app.whenReady().then(async () => {
  try {
    sidecar = await startSidecar();
  } catch (err) {
    dialog.showErrorBox(
      "Smart PC",
      "Local backend failed to start:\n" +
        err.message +
        "\n\nThe app will open without local services.",
    );
  }
  createWindow();
  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

app.on("before-quit", () => {
  if (sidecar) sidecar.kill();
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});
