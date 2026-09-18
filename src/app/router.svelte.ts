// Tiny hash router: #/login, #/register, #/settings[/<section>], #/ (home).
// No dependency. Unknown hashes set `unknown` so the app can redirect once.
export type RouteName = "home" | "login" | "register" | "settings";

export type RouteTarget =
  "" | "login" | "register" | "settings" | `settings/${number}`;

interface Parsed {
  name: RouteName;
  /** Raw section index from #/settings/<n>; the page clamps it. */
  settingsSection: number;
  unknown: boolean;
}

function parse(): Parsed {
  if (typeof window === "undefined")
    return { name: "home", settingsSection: 0, unknown: false };
  const h = window.location.hash.replace(/^#\/?/, "");
  if (h === "login")
    return { name: "login", settingsSection: 0, unknown: false };
  if (h === "register")
    return { name: "register", settingsSection: 0, unknown: false };
  if (h === "settings" || h.startsWith("settings/")) {
    const rest = h === "settings" ? "" : h.slice("settings/".length);
    const n = rest === "" ? 0 : Number(rest);
    return {
      name: "settings",
      settingsSection: Number.isInteger(n) && n >= 0 ? n : 0,
      unknown: false,
    };
  }
  if (h === "") return { name: "home", settingsSection: 0, unknown: false };
  return { name: "home", settingsSection: 0, unknown: true };
}

let current = $state<Parsed>(parse());

function resync(): void {
  current = parse();
}

export { resync };

if (typeof window !== "undefined") {
  window.addEventListener("hashchange", resync);
}

export const route = {
  get name(): RouteName {
    return current.name;
  },
  get settingsSection(): number {
    return current.settingsSection;
  },
  get unknown(): boolean {
    return current.unknown;
  },
};

export function navigate(to: RouteTarget, opts?: { replace?: boolean }): void {
  const hash = "#/" + to;
  if (opts?.replace) {
    window.history.replaceState(null, "", hash);
    resync();
  } else {
    window.location.hash = hash;
  }
}

/**
 * Single auth-routing truth (replaces the scattered Shell/AuthPage
 * redirects): anonymous users land on login, authed users never sit on
 * login/register.
 */
export function syncAuthRoute(user: unknown): void {
  if (user && (current.name === "login" || current.name === "register")) {
    navigate("");
  } else if (
    !user &&
    (current.name === "home" || current.name === "settings")
  ) {
    navigate("login");
  }
}
