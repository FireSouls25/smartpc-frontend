// Smart PC — preload bridge (the ONLY code with both Node and DOM access).
// Exposes a minimal, typed API on window.smartpc. Allow-list, nothing else.
const { contextBridge, ipcRenderer } = require("electron");

contextBridge.exposeInMainWorld("smartpc", {
  versions: {
    electron: process.versions.electron,
    chrome: process.versions.chrome,
    node: process.versions.node,
  },
  ping: () => ipcRenderer.invoke("system:ping"),
  // Set by main after spawning the Rust sidecar (undefined in plain web).
  sidecar:
    process.env.SIDECAR_URL && process.env.SIDECAR_TOKEN
      ? { url: process.env.SIDECAR_URL, token: process.env.SIDECAR_TOKEN }
      : undefined,
  // Future: executor.enqueue(plan), mic/cam brokers, vault.
  // Renderer must never receive raw ipcRenderer or Node builtins.
});
