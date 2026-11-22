// @vitest-environment happy-dom
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import { get } from "svelte/store";

// Spec plugin-suggest-on-add: the suggestion cards of the add panel.

const api = vi.hoisted(() => ({
  suggestPlugins: vi.fn(),
  installPlugin: vi.fn(),
  setPluginEnabled: vi.fn(),
  addBatch: vi.fn(),
}));
vi.mock("../src/lib/api", async (orig) => ({ ...(await orig<object>()), ...api }));

import PluginSuggestions from "../src/components/PluginSuggestions.svelte";
import AddPanel from "../src/components/AddPanel.svelte";
import { locale } from "../src/lib/i18n";
import { toasts } from "../src/lib/toast";
import type { LinkSuggestion } from "../src/lib/pluginSuggest";

let app: ReturnType<typeof mount> | null = null;

const BUNKR: LinkSuggestion = {
  id: "bunkr",
  name: "Bunkr",
  version: "1.0.0",
  domains: ["bunkr.ac", "bunkr.ci", "bunkr.cr", "bunkr.su", "bunkr.ru"],
  urls: ["https://bunkr.cr/a/1", "https://bunkr.cr/a/2"],
  action: "install",
};

function render(links: string[]) {
  app = mount(PluginSuggestions, { target: document.body, props: { links } });
  flushSync();
}

function cards() {
  return [...document.querySelectorAll<HTMLElement>("[data-suggestion]")];
}

function button(card: HTMLElement, label: string) {
  return [...card.querySelectorAll("button")].find((b) => b.textContent?.trim() === label) as HTMLButtonElement;
}

async function cardsShown(n: number) {
  await vi.waitFor(() => {
    flushSync();
    expect(cards()).toHaveLength(n);
  });
}

beforeEach(() => {
  locale.set("en");
  toasts.set([]);
  for (const fn of Object.values(api)) fn.mockReset();
});

afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = "";
});

describe("suggestion cards", () => {
  it("AC-10 shows name, link count, domains behind show all, and Install", async () => {
    api.suggestPlugins.mockResolvedValue([BUNKR]);
    render(BUNKR.urls);
    await cardsShown(1);
    expect(api.suggestPlugins).toHaveBeenCalledWith(BUNKR.urls);

    const card = cards()[0];
    expect(card.textContent).toContain("Bunkr handles 2 of these links");
    expect(card.textContent).toContain("bunkr.ac, bunkr.ci, bunkr.cr");
    expect(card.textContent).not.toContain("bunkr.su");
    button(card, "Show all (5)").click();
    flushSync();
    expect(card.textContent).toContain("bunkr.su");
    expect(button(card, "Install")).toBeTruthy();
  });

  it("AC-10 names unscoped and empty domain lists, and Enable for an installed plugin", async () => {
    api.suggestPlugins.mockResolvedValue([
      { ...BUNKR, id: "a", name: "Any", domains: null, urls: ["https://a.test/1"] },
      { ...BUNKR, id: "n", name: "None", domains: [], urls: ["https://n.test/1"], action: "enable" },
    ]);
    render(["https://a.test/1", "https://n.test/1"]);
    await cardsShown(2);
    const [any, none] = cards();
    expect(any.textContent).toContain("Any handles 1 of these links");
    expect(any.textContent).toContain("any public host");
    expect(none.textContent).toContain("No hosts");
    expect(button(none, "Enable")).toBeTruthy();
  });

  it("AC-11 Install approves the domains, toasts and asks again", async () => {
    api.suggestPlugins.mockResolvedValueOnce([BUNKR]).mockResolvedValueOnce([]);
    api.installPlugin.mockResolvedValue({ id: "bunkr", version: "1.0.0" });
    render(BUNKR.urls);
    await cardsShown(1);

    button(cards()[0], "Install").click();
    await cardsShown(0);
    expect(api.installPlugin).toHaveBeenCalledWith("bunkr", true);
    expect(api.suggestPlugins).toHaveBeenCalledTimes(2);
    expect(get(toasts).map((t) => [t.type, t.title])).toEqual([["success", "Bunkr installed"]]);
  });

  it("AC-11 Enable enables the installed plugin", async () => {
    api.suggestPlugins.mockResolvedValueOnce([{ ...BUNKR, action: "enable" }]).mockResolvedValueOnce([]);
    api.setPluginEnabled.mockResolvedValue(undefined);
    render(BUNKR.urls);
    await cardsShown(1);

    button(cards()[0], "Enable").click();
    await cardsShown(0);
    expect(api.setPluginEnabled).toHaveBeenCalledWith("bunkr", true);
    expect(api.installPlugin).not.toHaveBeenCalled();
    expect(get(toasts).map((t) => [t.type, t.title])).toEqual([["success", "Bunkr enabled"]]);
  });

  it("AC-11 a failed refresh after an install still removes the installed card", async () => {
    const other = { ...BUNKR, id: "gofile", name: "Gofile", urls: ["https://gofile.io/d/x"] };
    api.suggestPlugins.mockResolvedValueOnce([BUNKR, other]).mockRejectedValueOnce(new Error("offline"));
    api.installPlugin.mockResolvedValue({ id: "bunkr", version: "1.0.0" });
    render([...BUNKR.urls, ...other.urls]);
    await cardsShown(2);

    button(cards()[0], "Install").click();
    await cardsShown(1);
    expect(cards()[0].textContent).toContain("Gofile");
  });

  it("AC-12 a failed install keeps the card and its button, with an error toast", async () => {
    api.suggestPlugins.mockResolvedValue([BUNKR]);
    api.installPlugin.mockRejectedValue(new Error("checksum mismatch"));
    render(BUNKR.urls);
    await cardsShown(1);

    button(cards()[0], "Install").click();
    await vi.waitFor(() => {
      flushSync();
      expect(get(toasts)).toHaveLength(1);
    });
    expect(get(toasts)[0]).toMatchObject({ type: "error", title: "Could not install Bunkr", message: "checksum mismatch" });
    flushSync();
    expect(cards()).toHaveLength(1);
    expect(button(cards()[0], "Install").disabled).toBe(false);
  });
});

describe("failed enable and German texts", () => {
  it("AC-12 a failed enable keeps the card, its button and the field's links", async () => {
    api.suggestPlugins.mockResolvedValue([{ ...BUNKR, action: "enable" }]);
    api.setPluginEnabled.mockRejectedValue(new Error("plugin not found"));
    app = mount(AddPanel, { target: document.body });
    flushSync();
    const field = document.querySelector("textarea") as HTMLTextAreaElement;
    field.value = BUNKR.urls.join("\n");
    field.dispatchEvent(new Event("input", { bubbles: true }));
    await cardsShown(1);

    button(cards()[0], "Enable").click();
    await vi.waitFor(() => {
      flushSync();
      expect(get(toasts)).toHaveLength(1);
    });
    expect(get(toasts)[0]).toMatchObject({ type: "error", title: "Could not enable Bunkr", message: "plugin not found" });
    flushSync();
    expect(cards()).toHaveLength(1);
    expect(button(cards()[0], "Enable").disabled).toBe(false);
    expect(field.value).toBe(BUNKR.urls.join("\n"));
  });

  it("AC-15 cards, buttons and toasts read German", async () => {
    locale.set("de");
    api.suggestPlugins
      .mockResolvedValueOnce([BUNKR, { ...BUNKR, id: "old", name: "Old", action: "enable", domains: null }])
      .mockResolvedValueOnce([BUNKR])
      .mockResolvedValue([]);
    api.installPlugin.mockRejectedValueOnce(new Error("x")).mockResolvedValueOnce({});
    api.setPluginEnabled.mockRejectedValueOnce(new Error("y")).mockResolvedValueOnce(undefined);
    render([...BUNKR.urls, "https://old.test/1"]);
    await cardsShown(2);
    const [bunkr, old] = cards();
    expect(bunkr.textContent).toContain("Bunkr kann 2 dieser Links laden");
    expect(bunkr.textContent).toContain("Darf verbinden mit:");
    expect(button(bunkr, "Alle zeigen (5)")).toBeTruthy();
    expect(old.textContent).toContain("jedem öffentlichen Host");

    const titles = async (n: number) =>
      vi.waitFor(() => {
        flushSync();
        expect(get(toasts)).toHaveLength(n);
      });
    // Both fail first, then enable succeeds (the refresh keeps Bunkr), then install.
    button(bunkr, "Installieren").click();
    await titles(1);
    button(old, "Aktivieren").click();
    await titles(2);
    button(old, "Aktivieren").click();
    await titles(3);
    await cardsShown(1);
    button(cards()[0], "Installieren").click();
    await titles(4);
    expect(get(toasts).map((t) => t.title).sort()).toEqual(
      ["Bunkr konnte nicht installiert werden", "Old konnte nicht aktiviert werden", "Bunkr installiert", "Old aktiviert"].sort(),
    );
  });
});

describe("add panel (AC-13)", () => {
  it("queues the links while a lookup is still running", async () => {
    api.suggestPlugins.mockReturnValue(new Promise(() => {}));
    api.addBatch.mockResolvedValue({ ids: ["1"], errors: [] });
    app = mount(AddPanel, { target: document.body });
    flushSync();
    const field = document.querySelector("textarea") as HTMLTextAreaElement;
    field.value = "https://bunkr.cr/a/1";
    field.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
    await vi.waitFor(() => expect(api.suggestPlugins).toHaveBeenCalled());

    const add = [...document.querySelectorAll("button")].find((b) => b.textContent?.trim() === "Add Download") as HTMLButtonElement;
    expect(add.disabled).toBe(false);
    add.click();
    await vi.waitFor(() => expect(api.addBatch).toHaveBeenCalledWith(["https://bunkr.cr/a/1"]));
  });
});
