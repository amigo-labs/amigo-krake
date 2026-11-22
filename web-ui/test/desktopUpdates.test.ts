import { describe, it, expect, vi } from "vitest";
import type { DesktopModeState, UpdateCheck } from "../src/lib/api";
import {
  DEFAULT_UPDATE_MODE,
  UPDATE_MODES,
  aboutView,
  announcementToShow,
  bannerView,
  checkAndApply,
  readyVersion,
  shouldAskQuestion,
} from "../src/lib/desktopUpdates";
import { translations } from "../src/lib/i18n";

// Spec desktop-app: the update choice in the web UI (AC-18, AC-25 to AC-30, AC-33).

function mode(over: Partial<DesktopModeState> = {}): DesktopModeState {
  return {
    update_mode: "auto",
    update_mode_chosen: false,
    question_pending: true,
    can_self_update: true,
    pending_restart: null,
    announcement: null,
    saved: true,
    ...over,
  };
}

function check(core: Partial<UpdateCheck["core"]> = {}): UpdateCheck {
  return {
    core: {
      current_version: "1.0.0",
      latest_version: "1.1.0",
      update_available: true,
      release_notes: null,
      ...core,
    },
    plugins: [],
  };
}

describe("first-start question (AC-25, AC-26)", () => {
  it("AC-25 shows only in desktop mode for a copy that can update itself with no choice stored", () => {
    expect(shouldAskQuestion(mode(), false)).toBe(true);
    // Browser against krake-server / Docker: GET /updates/mode is 404.
    expect(shouldAskQuestion(null, false)).toBe(false);
    // .deb / .rpm or debug build.
    expect(shouldAskQuestion(mode({ can_self_update: false }), false)).toBe(false);
    // A choice is stored, or the question was closed in this run.
    expect(shouldAskQuestion(mode({ question_pending: false, update_mode_chosen: true }), false)).toBe(false);
  });

  it("AC-25 preselects install automatically and offers the three choices in order", () => {
    expect(DEFAULT_UPDATE_MODE).toBe("auto");
    expect(UPDATE_MODES).toEqual(["auto", "notify", "off"]);
  });

  it("AC-26 never shows again in this window once confirmed or closed", () => {
    expect(shouldAskQuestion(mode(), true)).toBe(false);
  });
});

describe("notify announcement (AC-28, AC-29)", () => {
  const none = new Set<string>();

  it("AC-28 shows the pending version", () => {
    expect(announcementToShow(mode({ update_mode: "notify", announcement: "1.2.0" }), none, false)).toBe("1.2.0");
  });

  it("AC-28 shows a version once: an answered version does not come back", () => {
    const state = mode({ update_mode: "notify", announcement: "1.2.0" });
    expect(announcementToShow(state, new Set(["1.2.0"]), false)).toBeNull();
    // A still newer version is announced.
    expect(announcementToShow({ ...state, announcement: "1.3.0" }, new Set(["1.2.0"]), false)).toBe("1.3.0");
  });

  it("AC-28 waits behind the first-start question", () => {
    expect(announcementToShow(mode({ update_mode: "notify", announcement: "1.2.0" }), none, true)).toBeNull();
  });

  it("AC-29 announces nothing under never check, nor outside desktop mode", () => {
    expect(announcementToShow(mode({ update_mode: "off", announcement: "1.2.0" }), none, false)).toBeNull();
    expect(announcementToShow(null, none, false)).toBeNull();
    expect(announcementToShow(mode({ update_mode: "notify" }), none, false)).toBeNull();
  });
});

describe("About section (AC-30)", () => {
  it("AC-30 outside the desktop app shows the version only", () => {
    expect(aboutView(null)).toEqual({ kind: "version" });
  });

  it("AC-30 shows the reason instead of the choices where the copy cannot update itself", () => {
    expect(aboutView(mode({ can_self_update: false }))).toEqual({ kind: "package-manager" });
  });

  it("AC-30 in the desktop app shows the effective choice and the pending version", () => {
    expect(aboutView(mode({ update_mode: "notify", pending_restart: "1.1.0" }))).toEqual({
      kind: "desktop",
      mode: "notify",
      pending: "1.1.0",
    });
  });

  it("AC-30 Check for updates checks, then applies whatever the choice", async () => {
    const apply = vi.fn().mockResolvedValue({ status: "ready", version: "1.1.0" });
    expect(await checkAndApply(async () => check({ distribution: "tauri", can_self_update: true }), apply)).toEqual({
      kind: "ready",
      version: "1.1.0",
    });
    expect(apply).toHaveBeenCalledTimes(1);
  });

  it("AC-30 Check for updates applies nothing when up to date", async () => {
    const apply = vi.fn();
    expect(await checkAndApply(async () => check({ update_available: false, latest_version: null }), apply)).toEqual({
      kind: "current",
    });
    expect(apply).not.toHaveBeenCalled();
  });

  it("AC-30 Check for updates reports a version already waiting for the next start", async () => {
    const apply = vi.fn();
    const info = check({ update_available: false, pending_restart: "1.1.0", distribution: "tauri" });
    expect(await checkAndApply(async () => info, apply)).toEqual({ kind: "ready", version: "1.1.0" });
    expect(apply).not.toHaveBeenCalled();
  });

  it("AC-30 a failed apply propagates its error", async () => {
    await expect(
      checkAndApply(async () => check(), () => Promise.reject(new Error("signature mismatch"))),
    ).rejects.toThrow("signature mismatch");
  });
});

describe("core-update banner (AC-18)", () => {
  it("AC-18 offers the Update button for an available update", () => {
    expect(bannerView(check(), null)).toEqual({ kind: "offer", from: "1.0.0", to: "1.1.0" });
    // A plain server or Docker keeps its button whatever can_self_update says.
    expect(bannerView(check({ distribution: "Docker", can_self_update: false }), null).kind).toBe("offer");
  });

  it("AC-18 shows the package-manager hint instead of the button for deb/rpm", () => {
    expect(bannerView(check({ distribution: "tauri", can_self_update: false }), null)).toEqual({
      kind: "package-manager",
      from: "1.0.0",
      to: "1.1.0",
    });
  });

  it("AC-18 reads ready after a successful apply, and on load with pending_restart", () => {
    expect(bannerView(check(), "1.1.0")).toEqual({ kind: "ready", version: "1.1.0" });
    expect(
      bannerView(check({ update_available: false, pending_restart: "1.1.0", distribution: "tauri" }), null),
    ).toEqual({ kind: "ready", version: "1.1.0" });
  });

  it("AC-18 hides when nothing is available", () => {
    expect(bannerView(null, null)).toEqual({ kind: "hidden" });
    expect(bannerView(check({ update_available: false }), null)).toEqual({ kind: "hidden" });
  });

  it("AC-18 reads the desktop apply answer", () => {
    expect(readyVersion({ status: "ready", version: "1.1.0" })).toBe("1.1.0");
    expect(readyVersion({ status: "started" })).toBeNull();
    expect(readyVersion(undefined)).toBeNull();
  });
});

describe("strings (AC-33)", () => {
  it("AC-33 every update text exists in English and German", () => {
    const keys = Object.keys(translations.en).filter((k) => k.startsWith("updates.") || k === "settings.tagline");
    expect(keys.length).toBeGreaterThan(15);
    for (const key of keys) {
      expect(translations.de[key], key).toBeTruthy();
      expect(translations.de[key], key).not.toBe("");
    }
    for (const m of UPDATE_MODES) {
      expect(translations.en[`updates.mode_${m}`]).toBeTruthy();
      expect(translations.de[`updates.mode_${m}`]).toBeTruthy();
    }
    expect(translations.en["updates.ready"]).toBe("Update ready — takes effect at the next start");
  });
});
