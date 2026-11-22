// @vitest-environment happy-dom
import { describe, it, expect, vi, afterEach, beforeEach } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import UpdatePrompt from "../src/components/UpdatePrompt.svelte";
import SettingsAbout from "../src/components/settings/SettingsAbout.svelte";
import { locale } from "../src/lib/i18n";
import { desktopMode, desktopModeKnown } from "../src/lib/desktopMode";
import { resetLayers } from "../src/lib/overlays.svelte";
import type { DesktopModeState } from "../src/lib/api";

// Spec desktop-app: the first-start question, the notify announcement and
// Settings ▸ About against a fake server (AC-25 to AC-30).

// happy-dom's Web Animations never finish, so transitions would keep a closed
// dialog in the DOM. This stand-in finishes every animation at once.
Element.prototype.animate = function () {
  const animation = {
    onfinish: null as null | (() => void),
    cancel() {},
    finish() {},
    play() {},
    pause() {},
    currentTime: 0,
    playState: "finished",
    effect: { getComputedTiming: () => ({ progress: 1 }) },
  };
  setTimeout(() => animation.onfinish?.());
  return animation as unknown as Animation;
};

type Call = { method: string; path: string; body: unknown };
type Route = (call: Call) => { status: number; body?: unknown } | undefined;

let calls: Call[] = [];
let app: ReturnType<typeof mount> | null = null;

function state(over: Partial<DesktopModeState> = {}): DesktopModeState {
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

/** A fake server: `route` answers what it knows, everything else is 404. */
function serve(route: Route) {
  calls = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string, init?: RequestInit) => {
      const call = {
        method: init?.method ?? "GET",
        path: url.replace("/api/v1", ""),
        body: init?.body ? JSON.parse(init.body as string) : undefined,
      };
      calls.push(call);
      const answer = route(call) ?? { status: 404, body: { error: "not found" } };
      return new Response(answer.body === undefined ? null : JSON.stringify(answer.body), {
        status: answer.status,
        headers: { "Content-Type": "application/json" },
      });
    }),
  );
}

const dialog = () => document.querySelector('[role="dialog"]');
const button = (label: string) =>
  [...document.querySelectorAll("button")].find((b) => b.textContent?.trim() === label);
const radio = (value: string) => document.querySelector<HTMLInputElement>(`input[type="radio"][value="${value}"]`);
const sent = (method: string, path: string) => calls.filter((c) => c.method === method && c.path === path);

async function settle(check: () => void) {
  await vi.waitFor(() => {
    flushSync();
    check();
  });
}

function show(component: typeof UpdatePrompt | typeof SettingsAbout) {
  document.body.innerHTML = "";
  app = mount(component, { target: document.body });
  flushSync();
}

beforeEach(() => {
  locale.set("en");
  desktopMode.set(null);
  desktopModeKnown.set(false);
  resetLayers();
});

afterEach(() => {
  if (app) unmount(app);
  app = null;
  vi.unstubAllGlobals();
});

describe("first-start question (AC-25, AC-26)", () => {
  it("AC-25 asks with install automatically preselected in desktop mode", async () => {
    serve((c) => (c.path === "/updates/mode" ? { status: 200, body: state() } : undefined));
    show(UpdatePrompt);
    await settle(() => expect(dialog()?.textContent).toContain("How should Krake handle updates?"));
    expect(radio("auto")?.checked).toBe(true);
    expect(radio("notify")?.checked).toBe(false);
    expect(dialog()?.textContent).toContain("Install automatically (recommended)");
    expect(dialog()?.textContent).toContain("Notify me when an update is available");
    expect(dialog()?.textContent).toContain("Never check for updates");
    expect(button("Confirm")).toBeTruthy();
  });

  it("AC-25 never asks outside desktop mode (404)", async () => {
    serve(() => undefined);
    show(UpdatePrompt);
    await settle(() => expect(sent("GET", "/updates/mode").length).toBe(1));
    await new Promise((r) => setTimeout(r, 0));
    flushSync();
    expect(dialog()).toBeNull();
  });

  it("AC-25 never asks a copy that cannot update itself", async () => {
    serve((c) => (c.path === "/updates/mode" ? { status: 200, body: state({ can_self_update: false }) } : undefined));
    show(UpdatePrompt);
    await settle(() => expect(sent("GET", "/updates/mode").length).toBe(1));
    await new Promise((r) => setTimeout(r, 0));
    flushSync();
    expect(dialog()).toBeNull();
  });

  it("AC-26 Confirm stores the chosen option", async () => {
    serve((c) => {
      if (c.path !== "/updates/mode") return undefined;
      if (c.method === "PUT") return { status: 200, body: state({ update_mode: "notify", question_pending: false, update_mode_chosen: true }) };
      return { status: 200, body: state() };
    });
    show(UpdatePrompt);
    await settle(() => expect(dialog()).not.toBeNull());
    radio("notify")!.click();
    flushSync();
    button("Confirm")!.click();
    await settle(() => expect(dialog()).toBeNull());
    expect(sent("PUT", "/updates/mode").map((c) => c.body)).toEqual([{ mode: "notify" }]);
  });

  it("AC-26 closing without confirming sends dismissed and stores nothing", async () => {
    serve((c) => {
      if (c.path !== "/updates/mode") return undefined;
      if (c.method === "PUT") return { status: 200, body: state({ question_pending: false }) };
      return { status: 200, body: state() };
    });
    show(UpdatePrompt);
    await settle(() => expect(dialog()).not.toBeNull());
    radio("off")!.click();
    flushSync();
    document.querySelector<HTMLButtonElement>('[role="dialog"] button[aria-label="Close"]')!.click();
    await settle(() => expect(dialog()).toBeNull());
    expect(sent("PUT", "/updates/mode").map((c) => c.body)).toEqual([{ dismissed: true }]);
  });

  it("AC-26 Escape counts as closing without a choice", async () => {
    serve((c) => {
      if (c.path !== "/updates/mode") return undefined;
      if (c.method === "PUT") return { status: 200, body: state({ question_pending: false }) };
      return { status: 200, body: state() };
    });
    show(UpdatePrompt);
    await settle(() => expect(dialog()).not.toBeNull());
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    await settle(() => expect(dialog()).toBeNull());
    expect(sent("PUT", "/updates/mode").map((c) => c.body)).toEqual([{ dismissed: true }]);
  });
});

describe("notify announcement (AC-28)", () => {
  const announcing = state({
    update_mode: "notify",
    update_mode_chosen: true,
    question_pending: false,
    announcement: "1.2.0",
  });

  it("AC-28 shows the version; showing it marks nothing; Later marks it", async () => {
    serve((c) => {
      if (c.path === "/updates/mode") return { status: 200, body: announcing };
      if (c.path === "/updates/announced") return { status: 200, body: { ...announcing, announcement: null } };
      return undefined;
    });
    show(UpdatePrompt);
    await settle(() => expect(dialog()?.textContent).toContain("Krake 1.2.0 is available"));
    expect(sent("POST", "/updates/announced")).toEqual([]);
    button("Later")!.click();
    await settle(() => expect(dialog()).toBeNull());
    expect(sent("POST", "/updates/announced").map((c) => c.body)).toEqual([{ version: "1.2.0" }]);
    expect(sent("POST", "/updates/core")).toEqual([]);
  });

  it("AC-28 Install applies, reads ready, then marks the version", async () => {
    serve((c) => {
      if (c.path === "/updates/mode") return { status: 200, body: announcing };
      if (c.path === "/updates/core") return { status: 200, body: { status: "ready", version: "1.2.0" } };
      if (c.path === "/updates/announced") return { status: 200, body: { ...announcing, announcement: null } };
      return undefined;
    });
    show(UpdatePrompt);
    await settle(() => expect(button("Install")).toBeTruthy());
    button("Install")!.click();
    await settle(() => expect(dialog()?.textContent).toContain("Update ready — takes effect at the next start"));
    await settle(() => expect(sent("POST", "/updates/announced").map((c) => c.body)).toEqual([{ version: "1.2.0" }]));
    const order = calls.map((c) => `${c.method} ${c.path}`);
    expect(order.indexOf("POST /updates/core")).toBeLessThan(order.indexOf("POST /updates/announced"));
  });

  it("AC-28 a failed Install shows the error and leaves the version unannounced", async () => {
    serve((c) => {
      if (c.path === "/updates/mode") return { status: 200, body: announcing };
      if (c.path === "/updates/core") return { status: 502, body: { error: "signature mismatch" } };
      return undefined;
    });
    show(UpdatePrompt);
    await settle(() => expect(button("Install")).toBeTruthy());
    button("Install")!.click();
    await settle(() => expect(dialog()?.textContent).toContain("signature mismatch"));
    expect(dialog()?.textContent).toContain("Update failed");
    expect(sent("POST", "/updates/announced")).toEqual([]);
  });
});

describe("Settings ▸ About (AC-30)", () => {
  it("AC-30 outside the desktop app shows the running version only", async () => {
    serve((c) => (c.path === "/status" ? { status: 200, body: { status: "ok", version: "0.1.10" } } : undefined));
    show(SettingsAbout);
    await settle(() => expect(document.querySelector('[data-testid="about-version"]')?.textContent).toBe("v0.1.10"));
    await settle(() => expect(sent("GET", "/updates/mode").length).toBe(1));
    flushSync();
    expect(document.body.textContent).not.toContain("v0.1.0 ");
    expect(radio("auto")).toBeNull();
    expect(button("Check for updates")).toBeUndefined();
  });

  it("AC-30 in the desktop app shows the pending version, the choices and stores a selection at once", async () => {
    serve((c) => {
      if (c.path === "/status") return { status: 200, body: { status: "ok", version: "1.0.0" } };
      if (c.path === "/updates/mode" && c.method === "PUT") {
        return { status: 200, body: state({ update_mode: "off", update_mode_chosen: true, question_pending: false, saved: false }) };
      }
      if (c.path === "/updates/mode") {
        return { status: 200, body: state({ update_mode: "notify", update_mode_chosen: true, question_pending: false, pending_restart: "1.1.0" }) };
      }
      return undefined;
    });
    show(SettingsAbout);
    await settle(() => expect(radio("notify")?.checked).toBe(true));
    expect(document.body.textContent).toContain("Version 1.1.0 is downloaded and takes effect at the next start");
    expect(document.body.textContent).toContain("v1.0.0");
    radio("off")!.click();
    await settle(() => expect(sent("PUT", "/updates/mode").map((c) => c.body)).toEqual([{ mode: "off" }]));
    await settle(() => expect(document.body.textContent).toContain("could not be saved"));
    expect(radio("off")?.checked).toBe(true);
  });

  it("AC-30 shows the package-manager reason instead of the choices for deb/rpm", async () => {
    serve((c) => {
      if (c.path === "/status") return { status: 200, body: { status: "ok", version: "1.0.0" } };
      if (c.path === "/updates/mode") return { status: 200, body: state({ can_self_update: false, question_pending: false }) };
      return undefined;
    });
    show(SettingsAbout);
    await settle(() => expect(document.querySelector('[data-testid="about-package-manager"]')).not.toBeNull());
    expect(document.body.textContent).toContain("Update with your package manager");
    expect(radio("auto")).toBeNull();
  });

  it("AC-30 Check for updates checks and applies like the banner", async () => {
    serve((c) => {
      if (c.path === "/status") return { status: 200, body: { status: "ok", version: "1.0.0" } };
      if (c.path === "/updates/mode") return { status: 200, body: state({ update_mode: "off", update_mode_chosen: true, question_pending: false }) };
      if (c.path === "/updates/check") {
        return {
          status: 200,
          body: {
            core: {
              current_version: "1.0.0",
              latest_version: "1.1.0",
              update_available: true,
              release_notes: null,
              distribution: "tauri",
              can_self_update: true,
              pending_restart: null,
              update_mode: "off",
              update_mode_chosen: true,
            },
            plugins: [],
          },
        };
      }
      if (c.path === "/updates/core") return { status: 200, body: { status: "ready", version: "1.1.0" } };
      return undefined;
    });
    show(SettingsAbout);
    await settle(() => expect(button("Check for updates")).toBeTruthy());
    button("Check for updates")!.click();
    await settle(() => expect(document.body.textContent).toContain("Update ready — takes effect at the next start"));
    expect(sent("GET", "/updates/check").length).toBe(1);
    expect(sent("POST", "/updates/core").length).toBe(1);
  });
});
