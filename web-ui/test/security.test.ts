import { describe, it, expect, vi, beforeEach } from "vitest";
import { get } from "svelte/store";
import { isLoopbackBind, securityView, needsConfirmation } from "../src/lib/security";
import { detectBootState } from "../src/lib/boot";
import { translations } from "../src/lib/i18n";

describe("securityView (AC-13, AC-14)", () => {
  it("shows the mode in effect and allows never only on a loopback bind", () => {
    expect(securityView({ bind: "127.0.0.1:1516", auth_mode: "private" })).toEqual({
      mode: "private",
      fromEnv: false,
      neverAllowed: true,
    });
    expect(securityView({ bind: "0.0.0.0:1516", auth_mode: "always" }).neverAllowed).toBe(false);
    expect(securityView({ bind: "192.168.1.10:1516" }).mode).toBe("always");
  });

  it("marks a mode set by the environment (AC-09)", () => {
    const view = securityView({ bind: "0.0.0.0:1516", auth_mode: "private", auth_mode_from_env: true });
    expect(view.fromEnv).toBe(true);
    expect(view.mode).toBe("private");
  });

  it("reads loopback binds", () => {
    for (const b of ["127.0.0.1:1516", "localhost:1516", "[::1]:1516", "127.0.0.5:80"]) {
      expect(isLoopbackBind(b), b).toBe(true);
    }
    for (const b of ["0.0.0.0:1516", "[::]:1516", "192.168.1.10:1516", "krake.lan:1516"]) {
      expect(isLoopbackBind(b), b).toBe(false);
    }
  });
});

describe("needsConfirmation (AC-15)", () => {
  it("asks before turning the login off, not before turning it on", () => {
    expect(needsConfirmation("always", "private")).toBe(true);
    expect(needsConfirmation("always", "never")).toBe(true);
    expect(needsConfirmation("private", "never")).toBe(true);
    expect(needsConfirmation("private", "always")).toBe(false);
    expect(needsConfirmation("private", "private")).toBe(false);
  });
});

describe("boot (AC-12)", () => {
  it("opens the app without the login page when the server lets the browser in", async () => {
    const state = await detectBootState(
      async () => ({ needs_setup: false, needs_pin: false }),
      async () => ({ kind: "local", mode: "private" }),
    );
    expect(state).toBe("ready");
  });

  it("shows the login page when the server wants a login", async () => {
    const state = await detectBootState(
      async () => ({ needs_setup: false, needs_pin: false }),
      async () => {
        throw new Error("HTTP 401");
      },
    );
    expect(state).toBe("login");
  });

  it("a later 401 flips the app to the login page", async () => {
    const { authRequired } = await import("../src/lib/stores");
    authRequired.set(false);
    vi.stubGlobal("fetch", vi.fn(async () => new Response("{}", { status: 401 })));
    const { getDownloads } = await import("../src/lib/api");
    await expect(getDownloads()).rejects.toThrow();
    expect(get(authRequired)).toBe(true);
    vi.unstubAllGlobals();
  });
});

describe("security strings (AC-16)", () => {
  it("every security.* key exists in English and German", async () => {
    const { readFileSync } = await import("node:fs");
    const { join } = await import("node:path");
    const used = new Set<string>();
    for (const file of ["../src/components/settings/SettingsSecurity.svelte", "../src/lib/security.ts"]) {
      const text = readFileSync(join(__dirname, file), "utf8");
      for (const m of text.matchAll(/"(security\.[a-z_]+)"/g)) used.add(m[1]);
      for (const m of text.matchAll(/`security\.\$\{[a-z]+\}(_desc)?`/g)) {
        for (const mode of ["always", "private", "never"]) used.add(`security.${mode}${m[1] ?? ""}`);
      }
    }
    expect(used.size).toBeGreaterThanOrEqual(10);
    for (const key of used) {
      expect(translations.en[key], `en: ${key}`).toBeTruthy();
      expect(translations.de[key], `de: ${key}`).toBeTruthy();
    }
  });
});
