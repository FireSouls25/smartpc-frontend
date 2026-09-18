import { afterEach, describe, expect, test, vi } from "vitest";

// The router reads window at import time, so each case gets a fresh stubbed
// window + a freshly imported module.
async function loadRouter(hash: string) {
  vi.resetModules();
  let current = hash;
  vi.stubGlobal("window", {
    location: {
      get hash() {
        return current;
      },
      set hash(v: string) {
        current = v;
      },
    },
    history: {
      replaceState(_a: unknown, _b: string, url: string) {
        current = url.startsWith("#") ? url : `#${url}`;
      },
    },
    addEventListener() {},
  });
  return import("./router.svelte");
}

describe("router", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  test("parses the known routes", async () => {
    expect((await loadRouter("")).route.name).toBe("home");
    expect((await loadRouter("#/login")).route.name).toBe("login");
    expect((await loadRouter("#/register")).route.name).toBe("register");
    const settings = await loadRouter("#/settings/2");
    expect(settings.route.name).toBe("settings");
    expect(settings.route.settingsSection).toBe(2);
  });

  test("unknown hashes fall back home flagged", async () => {
    const { route } = await loadRouter("#/nope");
    expect(route.name).toBe("home");
    expect(route.unknown).toBe(true);
  });

  test("bad section indexes clamp to 0", async () => {
    expect((await loadRouter("#/settings/bogus")).route.settingsSection).toBe(
      0,
    );
    expect((await loadRouter("#/settings/-3")).route.settingsSection).toBe(0);
  });

  test("navigate + resync track the hash", async () => {
    const { navigate, resync, route } = await loadRouter("");
    navigate("settings/1");
    resync();
    expect(route.name).toBe("settings");
    expect(route.settingsSection).toBe(1);
    navigate("settings", { replace: true });
    expect(route.name).toBe("settings");
    expect(route.settingsSection).toBe(0);
  });

  test("syncAuthRoute is the single auth truth", async () => {
    const anon = await loadRouter("#/");
    anon.syncAuthRoute(null);
    anon.resync();
    expect(anon.route.name).toBe("login");

    const authed = await loadRouter("#/login");
    authed.syncAuthRoute({ id: "u" });
    authed.resync();
    expect(authed.route.name).toBe("home");

    // No-ops stay put.
    const stay = await loadRouter("#/login");
    stay.syncAuthRoute(null);
    stay.resync();
    expect(stay.route.name).toBe("login");
    const stay2 = await loadRouter("#/");
    stay2.syncAuthRoute({ id: "u" });
    stay2.resync();
    expect(stay2.route.name).toBe("home");
  });
});
