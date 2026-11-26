// @vitest-environment happy-dom
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { get } from "svelte/store";
import { createDownloadActions } from "../src/lib/downloadActions.svelte";
import { toasts } from "../src/lib/toast";
import type { Download } from "../src/lib/stores";
import { inEffectRoot } from "./effectRoot.svelte";

// Over plain HTTP on a LAN host (e.g. `http://nas:1516`) the page is not a
// secure context and `navigator.clipboard` does not exist. Copying a
// download's URL must still work there, from the row button and from the
// context menu alike.

const URL_TO_COPY = "https://example.com/file.bin";
const download = { id: "d1", url: URL_TO_COPY, filename: "file.bin" } as Download;

let copied: string[];
let dispose: () => void = () => {};

function actions() {
  const root = inEffectRoot(() => createDownloadActions(() => download, () => "en"));
  dispose = root.dispose;
  return root.value;
}

function lastToast() {
  return get(toasts).at(-1);
}

beforeEach(() => {
  copied = [];
  toasts.set([]);
  // Not a secure context: no async clipboard API.
  Object.defineProperty(navigator, "clipboard", { value: undefined, configurable: true });
  // The legacy copy command copies the current selection.
  document.execCommand = vi.fn((command: string) => {
    if (command !== "copy") return false;
    const field = document.activeElement as HTMLTextAreaElement | null;
    copied.push(field?.value.slice(field.selectionStart, field.selectionEnd) ?? "");
    return true;
  }) as typeof document.execCommand;
});

afterEach(() => {
  dispose();
  document.body.innerHTML = "";
});

describe("copy a download's URL without navigator.clipboard", () => {
  it("copies from the row button and confirms it", async () => {
    await actions().copyUrl();
    expect(copied).toEqual([URL_TO_COPY]);
    expect(lastToast()?.type).toBe("info");
    expect(lastToast()?.title).toBe("URL copied");
  });

  it("copies from the context menu and confirms it", async () => {
    const copy = actions()
      .contextMenuItems()
      .find((item) => item.icon === "copy")!;
    expect(() => copy.action()).not.toThrow();
    await vi.waitFor(() => expect(lastToast()?.title).toBe("URL copied"));
    expect(copied).toEqual([URL_TO_COPY]);
  });

  it("reports a failure when the browser refuses to copy", async () => {
    document.execCommand = vi.fn(() => false) as typeof document.execCommand;
    await actions().copyUrl();
    expect(lastToast()?.type).toBe("error");
  });

  it("leaves no helper element behind", async () => {
    await actions().copyUrl();
    expect(document.querySelector("textarea")).toBeNull();
  });
});
