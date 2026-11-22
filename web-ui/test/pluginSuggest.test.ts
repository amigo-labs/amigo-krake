import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  createSuggestionLookup,
  domainView,
  SUGGEST_DELAY_MS,
  type LinkSuggestion,
} from "../src/lib/pluginSuggest";

// Spec plugin-suggest-on-add: the add panel's suggestion lookup.

function suggestion(id: string, urls: string[]): LinkSuggestion {
  return { id, name: id.toUpperCase(), version: "1.0.0", domains: [`${id}.test`], urls, action: "install" };
}

/** A fetch whose answers the test resolves by hand, in any order. */
function manualFetch() {
  const calls: { urls: string[]; resolve: (s: LinkSuggestion[]) => void; reject: (e: Error) => void }[] = [];
  const fetch = vi.fn(
    (urls: string[]) =>
      new Promise<LinkSuggestion[]>((resolve, reject) => {
        calls.push({ urls, resolve, reject });
      }),
  );
  return { fetch, calls };
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("suggestion lookup", () => {
  it("AC-10 asks once for all links after 500 ms without change", async () => {
    const { fetch, calls } = manualFetch();
    const onResult = vi.fn();
    const lookup = createSuggestionLookup({ fetch, onResult });

    lookup.update(["https://a.test/1"]);
    await vi.advanceTimersByTimeAsync(SUGGEST_DELAY_MS - 100);
    lookup.update(["https://a.test/1", "https://b.test/1"]);
    await vi.advanceTimersByTimeAsync(SUGGEST_DELAY_MS - 1);
    expect(fetch).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);
    expect(fetch).toHaveBeenCalledTimes(1);
    expect(calls[0].urls).toEqual(["https://a.test/1", "https://b.test/1"]);

    calls[0].resolve([suggestion("a", ["https://a.test/1"])]);
    await vi.advanceTimersByTimeAsync(0);
    expect(onResult).toHaveBeenLastCalledWith([suggestion("a", ["https://a.test/1"])]);
    expect(SUGGEST_DELAY_MS).toBe(500);
  });

  it("AC-14 drops an answer to an earlier lookup that arrives later", async () => {
    const { fetch, calls } = manualFetch();
    const onResult = vi.fn();
    const lookup = createSuggestionLookup({ fetch, onResult });

    lookup.update(["https://a.test/1"]);
    await vi.advanceTimersByTimeAsync(SUGGEST_DELAY_MS);
    lookup.update(["https://b.test/1"]);
    await vi.advanceTimersByTimeAsync(SUGGEST_DELAY_MS);
    expect(calls).toHaveLength(2);

    calls[1].resolve([suggestion("b", ["https://b.test/1"])]);
    await vi.advanceTimersByTimeAsync(0);
    calls[0].resolve([suggestion("a", ["https://a.test/1"])]);
    await vi.advanceTimersByTimeAsync(0);
    expect(onResult).toHaveBeenCalledTimes(1);
    expect(onResult).toHaveBeenLastCalledWith([suggestion("b", ["https://b.test/1"])]);
  });

  it("AC-14 a field without links shows no cards at once and drops the pending answer", async () => {
    const { fetch, calls } = manualFetch();
    const onResult = vi.fn();
    const lookup = createSuggestionLookup({ fetch, onResult });

    lookup.update(["https://a.test/1"]);
    await vi.advanceTimersByTimeAsync(SUGGEST_DELAY_MS);
    lookup.update([]);
    expect(onResult).toHaveBeenLastCalledWith([]);
    calls[0].resolve([suggestion("a", ["https://a.test/1"])]);
    await vi.advanceTimersByTimeAsync(SUGGEST_DELAY_MS);
    expect(onResult).toHaveBeenLastCalledWith([]);
    expect(fetch).toHaveBeenCalledTimes(1);
  });

  it("AC-13 a failed lookup reports nothing and throws nothing", async () => {
    const { fetch, calls } = manualFetch();
    const onResult = vi.fn();
    const lookup = createSuggestionLookup({ fetch, onResult });

    lookup.update(["https://a.test/1"]);
    await vi.advanceTimersByTimeAsync(SUGGEST_DELAY_MS);
    calls[0].reject(new Error("HTTP 400"));
    await vi.advanceTimersByTimeAsync(0);
    // At most "no cards"; the error itself goes nowhere.
    for (const [result] of onResult.mock.calls) expect(result).toEqual([]);
  });

  it("AC-14 a failed lookup for changed links clears the cards of the old links", async () => {
    const { fetch, calls } = manualFetch();
    const onResult = vi.fn();
    const lookup = createSuggestionLookup({ fetch, onResult });

    lookup.update(["https://a.test/1"]);
    await vi.advanceTimersByTimeAsync(SUGGEST_DELAY_MS);
    calls[0].resolve([suggestion("a", ["https://a.test/1"])]);
    await vi.advanceTimersByTimeAsync(0);
    lookup.update(["https://b.test/1"]);
    await vi.advanceTimersByTimeAsync(SUGGEST_DELAY_MS);
    calls[1].reject(new Error("offline"));
    await vi.advanceTimersByTimeAsync(0);
    expect(onResult).toHaveBeenLastCalledWith([]);
  });

  it("AC-11 refresh asks again for the current links at once", async () => {
    const { fetch, calls } = manualFetch();
    const lookup = createSuggestionLookup({ fetch, onResult: vi.fn() });

    lookup.update(["https://a.test/1"]);
    await vi.advanceTimersByTimeAsync(SUGGEST_DELAY_MS);
    lookup.refresh();
    expect(calls).toHaveLength(2);
    expect(calls[1].urls).toEqual(["https://a.test/1"]);
  });
});

describe("domain line (AC-10)", () => {
  it("names unscoped and empty lists and folds more than three", () => {
    expect(domainView(null)).toEqual({ kind: "any" });
    expect(domainView([])).toEqual({ kind: "none" });
    expect(domainView(["a", "b", "c"])).toEqual({ kind: "list", shown: ["a", "b", "c"], hidden: [] });
    expect(domainView(["a", "b", "c", "d", "e"])).toEqual({
      kind: "list",
      shown: ["a", "b", "c"],
      hidden: ["d", "e"],
    });
  });
});
