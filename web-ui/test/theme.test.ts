// The theme choice (docs/specs/amigo-theme.md, AC-04 to AC-10, AC-12, AC-13).
// Vitest runs in node: storage, the system preference and the document root
// are fakes handed to the store.
import { describe, it, expect } from "vitest";
import { get } from "svelte/store";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import {
  parseThemeChoice, resolvePalette, toggledChoice, createThemeStore,
  THEME_CHOICES, type ThemeEnv,
} from "../src/lib/theme";
import { tr } from "../src/lib/i18n";

const file = (path: string) => readFileSync(fileURLToPath(new URL(path, import.meta.url)), "utf8");

/** The `--am-bg` of the generated theme file, per palette. */
function themeBackgrounds(): { dark: string; light: string } {
  const css = file("../../packages/ui/src/theme/tokens.css");
  const dark = /:root\s*\{[^}]*?--am-bg:\s*([^;]+);/.exec(css)?.[1].trim();
  const light = /:root\[data-theme='light'\]\s*\{[^}]*?--am-bg:\s*([^;]+);/.exec(css)?.[1].trim();
  if (!dark || !light) throw new Error("theme/tokens.css has no --am-bg for both palettes");
  return { dark, light };
}

const BG = themeBackgrounds();

class FakeStorage {
  data = new Map<string, string>();
  constructor(initial: Record<string, string> = {}) {
    for (const [k, v] of Object.entries(initial)) this.data.set(k, v);
  }
  getItem(key: string): string | null {
    return this.data.get(key) ?? null;
  }
  setItem(key: string, value: string): void {
    this.data.set(key, value);
  }
  removeItem(key: string): void {
    this.data.delete(key);
  }
}

/** Storage as a browser with blocked site data has it: every call throws. */
class BrokenStorage {
  getItem(): string | null {
    throw new Error("SecurityError");
  }
  setItem(): void {
    throw new Error("QuotaExceededError");
  }
  removeItem(): void {
    throw new Error("SecurityError");
  }
}

class FakeMedia {
  listeners: (() => void)[] = [];
  constructor(public matches: boolean) {}
  addEventListener(_type: "change", listener: () => void): void {
    this.listeners.push(listener);
  }
  /** The system preference changes while the page is open. */
  change(prefersLight: boolean): void {
    this.matches = prefersLight;
    for (const l of this.listeners) l();
  }
}

/** A page: the root's `data-theme`, the theme-color meta, and the CSS that
 *  decides `--am-bg` from the attribute and the system preference. */
function page(opts: { stored?: Record<string, string>; prefersLight?: boolean; storage?: ThemeEnv["storage"] } = {}) {
  const storage = opts.storage !== undefined ? opts.storage : new FakeStorage(opts.stored);
  const media = new FakeMedia(opts.prefersLight ?? false);
  const state = { dataTheme: "dark" as string | null, themeColor: BG.dark };
  const env: ThemeEnv = {
    storage,
    media,
    root: {
      setAttribute: (_name, value) => { state.dataTheme = value; },
      removeAttribute: () => { state.dataTheme = null; },
    },
    readBackground: () =>
      state.dataTheme === "light" || (state.dataTheme === null && media.matches) ? BG.light : BG.dark,
    setThemeColor: (color) => { state.themeColor = color; },
  };
  return { env, media, state, storage };
}

describe("parseThemeChoice", () => {
  it("accepts the three choices", () => {
    expect(THEME_CHOICES).toEqual(["dark", "light", "system"]);
    for (const c of THEME_CHOICES) expect(parseThemeChoice(c)).toBe(c);
  });

  it("falls back to dark for nothing stored and for an unknown value (AC-05, AC-10)", () => {
    expect(parseThemeChoice(null)).toBe("dark");
    expect(parseThemeChoice(undefined)).toBe("dark");
    expect(parseThemeChoice("")).toBe("dark");
    expect(parseThemeChoice("Light")).toBe("dark");
    expect(parseThemeChoice("auto")).toBe("dark");
  });
});

describe("resolvePalette", () => {
  it("pins dark and light whatever the system prefers", () => {
    expect(resolvePalette("dark", true)).toBe("dark");
    expect(resolvePalette("light", false)).toBe("light");
  });

  it("follows the system under System (AC-08)", () => {
    expect(resolvePalette("system", true)).toBe("light");
    expect(resolvePalette("system", false)).toBe("dark");
  });
});

describe("toggledChoice", () => {
  it("is the palette opposite to the active one (AC-09)", () => {
    expect(toggledChoice("dark")).toBe("light");
    expect(toggledChoice("light")).toBe("dark");
  });
});

describe("theme store", () => {
  it("is dark with nothing stored, also on a light system (AC-05)", () => {
    const p = page({ prefersLight: true });
    const theme = createThemeStore(p.env);
    expect(get(theme)).toBe("dark");
    expect(get(theme.palette)).toBe("dark");
    expect(p.state.dataTheme).toBe("dark");
    expect(p.state.themeColor).toBe(BG.dark);
  });

  it("keeps a choice an earlier version stored (AC-06)", () => {
    const light = page({ stored: { theme: "light" } });
    const a = createThemeStore(light.env);
    expect(get(a)).toBe("light");
    expect(get(a.palette)).toBe("light");
    expect(light.state.dataTheme).toBe("light");

    const dark = page({ stored: { theme: "dark" }, prefersLight: true });
    const b = createThemeStore(dark.env);
    expect(get(b.palette)).toBe("dark");
    expect(dark.state.dataTheme).toBe("dark");
  });

  it("applies and stores a pick at once, and starts in it after a reload (AC-07)", () => {
    const p = page();
    const theme = createThemeStore(p.env);

    theme.set("light");
    expect(get(theme)).toBe("light");
    expect(get(theme.palette)).toBe("light");
    expect(p.state.dataTheme).toBe("light");
    expect(p.storage?.getItem("theme")).toBe("light");

    theme.set("system");
    expect(get(theme)).toBe("system");
    expect(p.state.dataTheme).toBeNull();
    expect(p.storage?.getItem("theme")).toBe("system");

    theme.set("dark");
    expect(p.state.dataTheme).toBe("dark");
    expect(p.storage?.getItem("theme")).toBe("dark");

    // Reload: a new page over the same storage.
    theme.set("light");
    const reloaded = page({ storage: p.storage });
    const again = createThemeStore(reloaded.env);
    expect(get(again)).toBe("light");
    expect(reloaded.state.dataTheme).toBe("light");
  });

  it("follows the system preference under System while the page is open (AC-08)", () => {
    const p = page({ stored: { theme: "system" }, prefersLight: true });
    const theme = createThemeStore(p.env);
    expect(get(theme)).toBe("system");
    expect(p.state.dataTheme).toBeNull();
    expect(get(theme.palette)).toBe("light");

    p.media.change(false);
    expect(get(theme.palette)).toBe("dark");
    expect(get(theme)).toBe("system");
    expect(p.state.dataTheme).toBeNull();

    p.media.change(true);
    expect(get(theme.palette)).toBe("light");
  });

  it("ignores the system preference under Dark and Light", () => {
    const p = page({ stored: { theme: "dark" } });
    const theme = createThemeStore(p.env);
    p.media.change(true);
    expect(get(theme.palette)).toBe("dark");
    expect(p.state.dataTheme).toBe("dark");
    expect(p.state.themeColor).toBe(BG.dark);
  });

  it("toggles to the palette opposite to the active one, also from System (AC-09)", () => {
    const p = page({ stored: { theme: "system" }, prefersLight: true });
    const theme = createThemeStore(p.env);

    theme.toggle(); // light is shown -> Dark
    expect(get(theme)).toBe("dark");
    expect(p.state.dataTheme).toBe("dark");
    expect(p.storage?.getItem("theme")).toBe("dark");

    theme.toggle();
    expect(get(theme)).toBe("light");
    expect(get(theme.palette)).toBe("light");

    // From System on a dark system: dark is shown -> Light.
    p.media.change(false);
    theme.set("system");
    theme.toggle();
    expect(get(theme)).toBe("light");
  });

  it("starts dark on an unknown stored value and keeps working (AC-10)", () => {
    const p = page({ stored: { theme: "sepia" }, prefersLight: true });
    const theme = createThemeStore(p.env);
    expect(get(theme)).toBe("dark");
    expect(p.state.dataTheme).toBe("dark");
    theme.set("light");
    expect(get(theme.palette)).toBe("light");
  });

  it("starts dark and still applies a pick when storage throws or is missing (AC-10)", () => {
    for (const storage of [new BrokenStorage(), null]) {
      const p = page({ storage, prefersLight: true });
      const theme = createThemeStore(p.env);
      expect(get(theme)).toBe("dark");
      expect(p.state.dataTheme).toBe("dark");

      expect(() => theme.set("light")).not.toThrow();
      expect(get(theme)).toBe("light");
      expect(p.state.dataTheme).toBe("light");
      expect(() => theme.toggle()).not.toThrow();
      expect(get(theme)).toBe("dark");
    }
  });

  it("works without a document, a media query or storage", () => {
    const theme = createThemeStore({
      storage: null, media: null, root: null,
      readBackground: () => "", setThemeColor: () => {},
    });
    expect(get(theme)).toBe("dark");
    expect(() => theme.set("system")).not.toThrow();
    expect(get(theme.palette)).toBe("dark");
  });

  it("deletes the Neon intensity an earlier version stored (AC-04)", () => {
    const key = "neon-intensity";
    const p = page({ stored: { [key]: "1", theme: "light" } });
    createThemeStore(p.env);
    expect(p.storage?.getItem(key)).toBeNull();
    expect(p.storage?.getItem("theme")).toBe("light");
    expect(p.state.dataTheme).toBe("light");
  });

  it("keeps theme-color on the active background (AC-12)", () => {
    const p = page();
    const theme = createThemeStore(p.env);
    expect(p.state.themeColor).toBe(BG.dark);

    theme.set("light");
    expect(p.state.themeColor).toBe(BG.light);

    theme.set("system");
    expect(p.state.themeColor).toBe(BG.dark);
    p.media.change(true);
    expect(p.state.themeColor).toBe(BG.light);
    p.media.change(false);
    expect(p.state.themeColor).toBe(BG.dark);
  });

  it("leaves theme-color alone while the stylesheet has no background yet", () => {
    const p = page();
    p.env.readBackground = () => "";
    const theme = createThemeStore(p.env);
    theme.set("light");
    expect(p.state.themeColor).toBe(BG.dark);
  });
});

describe("static page colours (AC-12)", () => {
  it("index.html starts in the dark palette with the dark background as theme-color", () => {
    const html = file("../index.html");
    expect(html).toMatch(/<html[^>]*\sdata-theme="dark"/);
    expect(/<meta name="theme-color" content="([^"]+)"/.exec(html)?.[1]).toBe(BG.dark);
  });

  it("the PWA manifest carries the dark background", () => {
    const manifest = JSON.parse(file("../public/manifest.json"));
    expect(manifest.theme_color).toBe(BG.dark);
    expect(manifest.background_color).toBe(BG.dark);
  });
});

describe("strings (AC-13, AC-04)", () => {
  it("names the three choices in English and German", () => {
    expect(["dark", "light", "system"].map((c) => tr("en", `theme.${c}`))).toEqual(["Dark", "Light", "System"]);
    expect(["dark", "light", "system"].map((c) => tr("de", `theme.${c}`))).toEqual(["Dunkel", "Hell", "System"]);
  });

  it("has no string of the removed intensity setting", () => {
    const source = file("../src/lib/i18n.ts");
    expect(source).not.toMatch(/intensity|Intensität/i);
    // A missing key comes back as the key itself.
    for (const lang of ["en", "de"] as const) {
      for (const key of ["settings.neon_intensity", "cmd.set_intensity", "intensity.off", "intensity.full"]) {
        expect(tr(lang, key)).toBe(key);
      }
    }
  });

  it("Settings ▸ Appearance offers the three choices and no intensity setting", () => {
    const source = file("../src/components/settings/SettingsAppearance.svelte");
    expect(source).not.toMatch(/intensity|type="range"/i);
    expect(source).toMatch(/THEME_CHOICES/);
  });
});
