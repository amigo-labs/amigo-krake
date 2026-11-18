import { describe, it, expect } from "vitest";
import { get } from "svelte/store";
import { addToast, toasts } from "../src/lib/toast";

describe("toast options", () => {
  it("keeps the multiline flag for the renderer (link list results, AC-08)", () => {
    addToast("info", "t", "line 1\nline 2", { multiline: true, duration: 0 });
    const last = get(toasts).at(-1)!;
    expect(last.multiline).toBe(true);
    expect(last.duration).toBe(0);
  });
});
