// @vitest-environment happy-dom
import { describe, it, expect, vi, afterEach } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import SettingsSecurity from "../src/components/settings/SettingsSecurity.svelte";
import { locale } from "../src/lib/i18n";

// Clicking a mode in Settings ▸ Security (spec local-auth-bypass, AC-15):
// turning the login off asks first, cancel changes nothing, confirm saves.

let app: ReturnType<typeof mount> | null = null;

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

/** Wait until the dialog is gone (its exit transition has ended). */
async function dialogClosed() {
  await vi.waitFor(() => {
    flushSync();
    expect(dialog()).toBeNull();
  });
}

function setup(server: Record<string, unknown>) {
  locale.set("en");
  const config = { server: { ...server } } as never as { server: { auth_mode: string } };
  const onsave = vi.fn();
  document.body.innerHTML = "";
  app = mount(SettingsSecurity, { target: document.body, props: { config, onsave } });
  flushSync();
  return { config, onsave };
}

const radio = (mode: string) => document.querySelector<HTMLInputElement>(`input[value="${mode}"]`)!;
const dialog = () => document.querySelector('[role="dialog"]');
const button = (label: string) =>
  [...document.querySelectorAll("button")].find((b) => b.textContent?.trim() === label)!;

afterEach(() => {
  if (app) unmount(app);
  app = null;
});

describe("choosing a mode (AC-15)", () => {
  it("asks before turning the login off and changes nothing on cancel", async () => {
    const { config, onsave } = setup({ bind: "127.0.0.1:1516", auth_mode: "always" });
    radio("private").click();
    flushSync();
    expect(dialog()).not.toBeNull();
    expect(onsave).not.toHaveBeenCalled();
    expect(config.server.auth_mode).toBe("always");
    expect(radio("always").checked).toBe(true);

    button("Cancel").click();
    await dialogClosed();
    expect(onsave).not.toHaveBeenCalled();
    expect(config.server.auth_mode).toBe("always");
    expect(radio("always").checked).toBe(true);
  });

  it("saves the mode once the user allows it", async () => {
    const { config, onsave } = setup({ bind: "127.0.0.1:1516", auth_mode: "always" });
    radio("never").click();
    flushSync();
    button("Allow").click();
    await dialogClosed();
    expect(onsave).toHaveBeenCalledTimes(1);
    expect(config.server.auth_mode).toBe("never");
  });

  it("turns the login back on without asking", () => {
    const { config, onsave } = setup({ bind: "127.0.0.1:1516", auth_mode: "private" });
    radio("always").click();
    flushSync();
    expect(dialog()).toBeNull();
    expect(onsave).toHaveBeenCalledTimes(1);
    expect(config.server.auth_mode).toBe("always");
  });
});
