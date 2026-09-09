// Theme manager: light / dark / auto, persisted, applied via
// documentElement.dataset.theme (see the single theme file lib/theme.css).
export type Theme = "light" | "dark" | "auto";

const KEY = "smartpc.theme";

let theme = $state<Theme>("auto");

function resolve(): "light" | "dark" {
  if (theme === "dark") return "dark";
  if (theme === "light") return "light";
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

function apply(): void {
  document.documentElement.dataset.theme = resolve();
}

export function initTheme(): void {
  if (typeof window === "undefined") return;
  const saved = window.localStorage.getItem(KEY);
  theme = saved === "light" || saved === "dark" ? saved : "auto";
  apply();
  window
    .matchMedia("(prefers-color-scheme: dark)")
    .addEventListener("change", apply);
}

export function getTheme(): Theme {
  return theme;
}

export function setTheme(next: Theme): void {
  theme = next;
  try {
    window.localStorage.setItem(KEY, next);
  } catch {
    /* private mode */
  }
  apply();
}
