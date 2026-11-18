import { describe, it, expect } from "vitest";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { parseLinkList } from "../src/lib/linkList";

// The same fixtures the Rust side reads (crates/core/tests/link_list_fixtures.rs):
// both implementations must agree case by case (AC-03).
const dir = join(__dirname, "../../crates/core/tests/fixtures/link-lists");
const cases = readdirSync(dir)
  .filter((f) => f.endsWith(".txt"))
  .map((f) => f.slice(0, -4));

describe("link list rule (shared fixtures)", () => {
  it("finds the shared fixtures", () => {
    expect(cases.length).toBeGreaterThanOrEqual(16);
  });

  for (const name of cases) {
    it(name, () => {
      const bytes = new Uint8Array(readFileSync(join(dir, `${name}.txt`)));
      const expected = JSON.parse(readFileSync(join(dir, `${name}.expected.json`), "utf8")) as {
        links: string[];
        not_recognized: number;
      };
      const got = parseLinkList(bytes);
      expect(got.links).toEqual(expected.links);
      expect(got.notRecognized).toBe(expected.not_recognized);
    });
  }

  it("accepts already decoded text", () => {
    expect(parseLinkList("https://a/x  note\nfoo\n")).toEqual({ links: ["https://a/x"], notRecognized: 1 });
  });
});
