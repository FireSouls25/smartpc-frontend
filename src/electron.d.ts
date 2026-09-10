// Types for the Electron preload bridge (window.smartpc).
// Undefined when running as plain web (vite dev in a browser): then the
// app falls back to VITE_API_URL / VITE_SIDECAR_TOKEN.
export {};

declare global {
  interface Window {
    smartpc?: {
      versions: Record<string, string | undefined>;
      ping: () => Promise<{ ok: boolean; at: string }>;
      /** Rust sidecar endpoint + per-launch token, injected by main. */
      sidecar?: { url: string; token: string };
    };
  }
}
