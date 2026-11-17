import { describe, it, expect } from "vitest";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { translations } from "../src/lib/i18n";

function sourceFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return sourceFiles(path);
    return /\.(svelte|ts)$/.test(name) ? [path] : [];
  });
}

// Literal keys only: tr(locale, "a.b") and tr($locale, 'a.b', ...). Template
// literals with ${...} are built at runtime and cannot be checked here.
const CALL = /\btr\(\s*\$?locale\s*,\s*(["'])([^"'\n]+)\1/g;

describe("i18n keys", () => {
  it("every literal tr() key exists in the en and de tables", () => {
    const missing: string[] = [];
    for (const file of sourceFiles(join(__dirname, "../src"))) {
      const text = readFileSync(file, "utf8");
      for (const m of text.matchAll(CALL)) {
        const key = m[2];
        for (const lang of ["en", "de"] as const) {
          if (!(key in translations[lang])) missing.push(`${lang}: ${key} (${file})`);
        }
      }
    }
    expect(missing).toEqual([]);
  });
});
