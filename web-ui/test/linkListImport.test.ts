import { describe, it, expect } from "vitest";
import {
  MAX_LIST_BYTES,
  chunkLinks,
  prepareLinkList,
  queueLinkList,
  resultToast,
  type ImportResult,
} from "../src/lib/linkListImport";
import { parseLinkList } from "../src/lib/linkList";

const none = { urls: new Set<string>(), ids: new Set<string>() };
// Echo translator: "key{a=1,b=2}" makes every count visible in assertions.
const t = (key: string, params?: Record<string, string | number>) =>
  params ? `${key}{${Object.entries(params).map(([k, v]) => `${k}=${v}`).join(",")}}` : key;

describe("prepareLinkList", () => {
  it("refuses more than 1 MiB of text (AC-10)", () => {
    expect(prepareLinkList("a".repeat(MAX_LIST_BYTES + 1)).kind).toBe("too_large");
  });

  it("measures text as UTF-8 bytes (AC-10)", () => {
    // 2 bytes per "ä": just over the limit in UTF-8, under it in UTF-16 units.
    expect(prepareLinkList("ä".repeat(MAX_LIST_BYTES / 2 + 1)).kind).toBe("too_large");
  });

  it("uses the file size when given (AC-10)", () => {
    expect(prepareLinkList("https://a/x", MAX_LIST_BYTES + 1).kind).toBe("too_large");
  });

  it("reports a list without links (AC-09)", () => {
    expect(prepareLinkList("# nothing\nhello\n")).toEqual({ kind: "no_links", notRecognized: 1 });
  });

  it("parses a list with links", () => {
    const p = prepareLinkList("https://a/x\n");
    expect(p.kind).toBe("ok");
  });
});

describe("chunkLinks", () => {
  it("keeps every JSON body at or below the limit and loses nothing (AC-11)", () => {
    const links = Array.from({ length: 5000 }, (_, i) => `https://host.example/${"x".repeat(200)}/${i}`);
    const chunks = chunkLinks(links, 64 * 1024);
    expect(chunks.length).toBeGreaterThan(1);
    expect(chunks.flat()).toEqual(links);
    for (const c of chunks) {
      expect(new TextEncoder().encode(JSON.stringify({ urls: c })).length).toBeLessThanOrEqual(64 * 1024);
    }
  });
});

describe("queueLinkList", () => {
  it("sends every link, in order, across several requests (AC-02, AC-11)", async () => {
    const links = Array.from({ length: 3000 }, (_, i) => `https://host.example/${"y".repeat(400)}/${i}`);
    const sent: string[] = [];
    let n = 0;
    const result = await queueLinkList({ links, notRecognized: 2 }, none, async (urls) => {
      sent.push(...urls);
      return { ids: urls.map(() => `id-${n++}`) };
    });
    expect(sent).toEqual(links);
    expect(result).toMatchObject({ queued: 3000, alreadyQueued: 0, notRecognized: 2, notSent: 0 });
  });

  it("does not send links of active downloads (AC-05)", async () => {
    const sent: string[] = [];
    const result = await queueLinkList(
      parseLinkList("https://a/1\nhttps://a/2\n"),
      { urls: new Set(["https://a/1"]), ids: new Set() },
      async (urls) => {
        sent.push(...urls);
        return { ids: urls.map((u) => `new-${u}`) };
      },
    );
    expect(sent).toEqual(["https://a/2"]);
    expect(result).toMatchObject({ queued: 1, alreadyQueued: 1 });
  });

  it("counts an id the server already had as already queued", async () => {
    const result = await queueLinkList(
      parseLinkList("https://a/1\n"),
      { urls: new Set(), ids: new Set(["old"]) },
      async () => ({ ids: ["old"] }),
    );
    expect(result).toMatchObject({ queued: 0, alreadyQueued: 1 });
  });

  it("keeps the other links when some are rejected (AC-08)", async () => {
    const result = await queueLinkList(
      parseLinkList("https://a/1\nhttp://127.0.0.1/x\nhttps://a/2\n"),
      none,
      async () => ({
        ids: ["i1", "i2"],
        errors: ["http://127.0.0.1/x: refusing private address 127.0.0.1"],
      }),
    );
    expect(result.queued).toBe(2);
    expect(result.rejected).toEqual([{ url: "http://127.0.0.1/x", reason: "refusing private address 127.0.0.1" }]);
  });

  it("stops at a request that fails as a whole and counts what was not sent (AC-11)", async () => {
    const links = Array.from({ length: 3000 }, (_, i) => `https://host.example/${"z".repeat(400)}/${i}`);
    let calls = 0;
    let sent = 0;
    const result = await queueLinkList({ links, notRecognized: 0 }, none, async (urls) => {
      calls += 1;
      if (calls === 2) throw new Error("HTTP 401");
      sent += urls.length;
      return { ids: urls.map((_, i) => `id-${calls}-${i}`) };
    });
    expect(calls).toBe(2);
    expect(result.queued).toBe(sent);
    expect(result.notSent).toBe(3000 - sent);
    expect(result.failure).toBe("HTTP 401");
  });
});

describe("resultToast", () => {
  const base: ImportResult = { queued: 0, alreadyQueued: 0, rejected: [], notRecognized: 0, notSent: 0 };

  it("names only the counts that are not zero (AC-05)", () => {
    const toast = resultToast({ ...base, queued: 3, notRecognized: 2 }, t);
    expect(toast.type).toBe("success");
    expect(toast.title).toContain("linklist.queued{count=3}");
    expect(toast.title).toContain("linklist.not_recognized{count=2}");
    expect(toast.title).not.toContain("already");
    expect(toast.title).not.toContain("rejected");
  });

  it("lists the first five rejected links, then 'and N more' (AC-08)", () => {
    const rejected = Array.from({ length: 7 }, (_, i) => ({ url: `https://r/${i}`, reason: `why ${i}` }));
    const toast = resultToast({ ...base, queued: 1, rejected }, t);
    for (let i = 0; i < 5; i++) expect(toast.message).toContain(`https://r/${i}`);
    expect(toast.message).toContain("why 0");
    expect(toast.message).not.toContain("https://r/5");
    expect(toast.message).toContain("linklist.more{count=2}");
  });

  it("states how many links were not sent after a failure (AC-11)", () => {
    const toast = resultToast({ ...base, queued: 10, notSent: 5, failure: "HTTP 500" }, t);
    expect(toast.type).toBe("error");
    expect(toast.message).toContain("linklist.not_sent{count=5,reason=HTTP 500}");
    expect(toast.message).toContain("HTTP 500");
  });

  it("is an error when nothing was queued", () => {
    const toast = resultToast({ ...base, rejected: [{ url: "https://r/1", reason: "bad" }] }, t);
    expect(toast.type).toBe("error");
  });
});

describe("link list strings (AC-12)", () => {
  it("every linklist.* key used by the import exists in English and German", async () => {
    const { readFileSync } = await import("node:fs");
    const { join } = await import("node:path");
    const { translations } = await import("../src/lib/i18n");
    const used = new Set<string>();
    for (const file of ["../src/lib/linkListImport.ts", "../src/lib/linkListActions.ts"]) {
      const text = readFileSync(join(__dirname, file), "utf8");
      for (const m of text.matchAll(/"(linklist\.[a-z_]+)"/g)) used.add(m[1]);
    }
    expect(used.size).toBeGreaterThanOrEqual(9);
    for (const key of used) {
      expect(translations.en[key], `en: ${key}`).toBeTruthy();
      expect(translations.de[key], `de: ${key}`).toBeTruthy();
    }
  });
});
