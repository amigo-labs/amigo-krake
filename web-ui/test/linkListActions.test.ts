import { describe, it, expect, vi, beforeEach } from "vitest";

// The API and the toasts are the edges: record what the actions send and show.
const addBatch = vi.fn(async (urls: string[]) => ({ ids: urls.map((_, i) => `id-${i}`) }));
const addToast = vi.fn();
vi.mock("../src/lib/api", () => ({ addBatch: (urls: string[]) => addBatch(urls) }));
vi.mock("../src/lib/toast", () => ({ addToast: (...args: unknown[]) => addToast(...args) }));

const { downloads, usenetDownloads } = await import("../src/lib/stores");
const { activeDownloads, importLinkList, importLinkListFile } = await import("../src/lib/linkListActions");
const { MAX_LIST_BYTES } = await import("../src/lib/linkListImport");

const row = (id: string, url: string, status: string) =>
  ({ id, url, status, protocol: "http", filename: null, filesize: null, priority: 0, bytes_downloaded: 0, speed: 0, error: null, created_at: "" });

beforeEach(() => {
  addBatch.mockClear();
  addToast.mockClear();
  downloads.set([]);
  usenetDownloads.set([]);
});

describe("activeDownloads (AC-05)", () => {
  it("counts only queued, downloading and paused downloads as active", () => {
    downloads.set([
      row("q", "https://a/q", "queued"),
      row("d", "https://a/d", "downloading"),
      row("p", "https://a/p", "paused"),
      row("c", "https://a/c", "completed"),
      row("f", "https://a/f", "failed"),
    ]);
    usenetDownloads.set([row("u", "nzb://u", "queued")]);
    const active = activeDownloads();
    expect([...active.urls].sort()).toEqual(["https://a/d", "https://a/p", "https://a/q", "nzb://u"]);
    expect([...active.ids].sort()).toEqual(["d", "p", "q", "u"]);
  });

  it("sends a link again whose download completed or failed", async () => {
    downloads.set([row("c", "https://a/c", "completed"), row("q", "https://a/q", "queued")]);
    await importLinkList("https://a/c\nhttps://a/q\n");
    expect(addBatch).toHaveBeenCalledWith(["https://a/c"]);
  });
});

describe("importLinkList (AC-05, AC-09, AC-10)", () => {
  it("queues the links and reports them in one toast", async () => {
    await importLinkList("https://a/1\nfoo\n");
    expect(addBatch).toHaveBeenCalledWith(["https://a/1"]);
    expect(addToast).toHaveBeenCalledTimes(1);
    expect(addToast.mock.calls[0][0]).toBe("success");
  });

  it("sends nothing and says so for a list without links (AC-09)", async () => {
    await importLinkList("hello\n", "a.txt");
    expect(addBatch).not.toHaveBeenCalled();
    expect(addToast).toHaveBeenCalledWith("error", "a.txt contains no links");
  });

  it("sends nothing and names the limit for text over 1 MiB (AC-10)", async () => {
    await importLinkList("https://a/1\n" + "x".repeat(MAX_LIST_BYTES));
    expect(addBatch).not.toHaveBeenCalled();
    expect(addToast.mock.calls[0][0]).toBe("error");
    expect(addToast.mock.calls[0][1]).toContain("1 MiB");
  });
});

describe("importLinkListFile (AC-05, AC-10)", () => {
  it("refuses a file over 1 MiB without reading it", async () => {
    const read = vi.fn();
    const file = { name: "big.txt", size: MAX_LIST_BYTES + 1, arrayBuffer: read } as unknown as File;
    await importLinkListFile(file);
    expect(read).not.toHaveBeenCalled();
    expect(addBatch).not.toHaveBeenCalled();
    expect(addToast.mock.calls[0][1]).toBe("big.txt is larger than 1 MiB, the limit for a link list");
  });

  it("queues the links of a dropped file", async () => {
    const file = new File(["https://a/1\nhttps://a/2\n"], "links.txt", { type: "text/plain" });
    await importLinkListFile(file);
    expect(addBatch).toHaveBeenCalledWith(["https://a/1", "https://a/2"]);
  });
});

describe("text drops and pastes (AC-07)", async () => {
  const { shouldImportTextDrop, pastedText } = await import("../src/lib/linkListImport");
  const field = { closest: () => ({}) };
  const elsewhere = { closest: () => null };

  it("imports a text drop only outside text fields and for outside drags", () => {
    expect(shouldImportTextDrop(["text/plain"], { files: false, internal: false }, elsewhere)).toBe(true);
    expect(shouldImportTextDrop(["text/plain"], { files: false, internal: false }, field)).toBe(false);
    expect(shouldImportTextDrop(["text/plain"], { files: true, internal: false }, elsewhere)).toBe(false);
    expect(shouldImportTextDrop(["text/plain"], { files: false, internal: true }, elsewhere)).toBe(false);
    expect(shouldImportTextDrop(["text/uri-list"], { files: false, internal: false }, elsewhere)).toBe(false);
  });

  it("takes the pasted text only when it has some and no field has focus", () => {
    expect(pastedText("https://a/1", elsewhere, null)).toBe("https://a/1");
    expect(pastedText("", elsewhere, null)).toBeNull();
    expect(pastedText(undefined, elsewhere, null)).toBeNull();
    expect(pastedText("https://a/1", elsewhere, field)).toBeNull();
  });
});
