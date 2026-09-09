import { authApi, type User } from "./auth.api";

const REFRESH_KEY = "smartpc.refresh";

let user = $state<User | null>(null);
// Access token lives only in memory. When this runs inside Electron it moves
// to safeStorage (and the refresh token below follows the same path).
let accessToken: string | null = null;

function readRefresh(): string | null {
  try {
    return window.localStorage.getItem(REFRESH_KEY);
  } catch {
    return null;
  }
}

function writeRefresh(token: string | null): void {
  try {
    if (token) window.localStorage.setItem(REFRESH_KEY, token);
    else window.localStorage.removeItem(REFRESH_KEY);
  } catch {
    /* private mode */
  }
}

async function login(email: string, password: string): Promise<void> {
  const { user: u, tokens } = await authApi.login(email, password);
  user = u;
  accessToken = tokens.access_token;
  writeRefresh(tokens.refresh_token);
}

async function register(email: string, password: string): Promise<void> {
  await authApi.register(email, password);
  await login(email, password);
}

async function restore(): Promise<void> {
  const rt = readRefresh();
  if (!rt) return;
  try {
    const { user: u, tokens } = await authApi.refresh(rt);
    user = u;
    accessToken = tokens.access_token;
    writeRefresh(tokens.refresh_token);
  } catch {
    user = null;
    accessToken = null;
    writeRefresh(null);
  }
}

async function logout(): Promise<void> {
  const rt = readRefresh();
  try {
    if (rt) await authApi.logout(rt);
  } catch {
    /* already gone server-side */
  }
  user = null;
  accessToken = null;
  writeRefresh(null);
}

async function deleteAccount(): Promise<void> {
  if (!accessToken) throw new Error("No session");
  await authApi.deleteAccount(accessToken);
  user = null;
  accessToken = null;
  writeRefresh(null);
}

export const auth = {
  get user(): User | null {
    return user;
  },
  login,
  register,
  restore,
  logout,
  deleteAccount,
};
