// The theme choice: Dark, Light or System (docs/specs/amigo-theme.md).
//
// The palettes themselves are CSS: `@amigo-labs/krake-ui` loads the shared
// amigo theme, which is dark on `:root`, light under `data-theme="light"`, and
// follows the system while the root has no `data-theme`. This module only
// decides which of the three the root carries, remembers the choice, and
// keeps `<meta name="theme-color">` on the active background.
import { writable, type Readable } from "svelte/store";

/** What the user picked in Settings ▸ Appearance. */
export type ThemeChoice = "dark" | "light" | "system";
/** What a choice resolves to: the palette on screen. */
export type Palette = "dark" | "light";

/** The choices in the order Settings shows them; the first is the default. */
export const THEME_CHOICES: readonly ThemeChoice[] = ["dark", "light", "system"];

const STORAGE_KEY = "theme";
/** The glow strength an earlier version stored; deleted at start. */
// ui-lint-disable-next-line no-neon-name
const LEGACY_INTENSITY_KEY = "neon-intensity";

/** A stored value as a choice; anything unknown, or nothing, is Dark. */
export function parseThemeChoice(raw: string | null | undefined): ThemeChoice {
  return THEME_CHOICES.find((c) => c === raw) ?? "dark";
}

/** The palette a choice shows, given what the system prefers. */
export function resolvePalette(choice: ThemeChoice, systemPrefersLight: boolean): Palette {
  if (choice === "system") return systemPrefersLight ? "light" : "dark";
  return choice;
}

/** The choice the header button makes: the palette opposite to the active one. */
export function toggledChoice(active: Palette): ThemeChoice {
  return active === "dark" ? "light" : "dark";
}

/** What the store needs from the page; tests pass fakes. */
export interface ThemeEnv {
  /** `localStorage`, or null where it cannot be reached. Calls may throw. */
  storage: Pick<Storage, "getItem" | "setItem" | "removeItem"> | null;
  /** `matchMedia("(prefers-color-scheme: light)")`, or null. */
  media: { readonly matches: boolean; addEventListener(type: "change", listener: () => void): void } | null;
  /** The `<html>` element, or null outside a document. */
  root: { setAttribute(name: string, value: string): void; removeAttribute(name: string): void } | null;
  /** The background the stylesheet gives the root right now (`--am-bg`), or "". */
  readBackground(): string;
  /** Writes `<meta name="theme-color">`. */
  setThemeColor(color: string): void;
}

export interface ThemeStore extends Readable<ThemeChoice> {
  /** The palette on screen. */
  palette: Readable<Palette>;
  /** Applies and stores a choice. */
  set(choice: ThemeChoice): void;
  /** The header button: Light while dark is shown, Dark while light is shown. */
  toggle(): void;
}

export function createThemeStore(env: ThemeEnv): ThemeStore {
  let stored: string | null = null;
  try {
    stored = env.storage?.getItem(STORAGE_KEY) ?? null;
  } catch {
    // Storage is blocked: start with the default.
  }
  try {
    env.storage?.removeItem(LEGACY_INTENSITY_KEY);
  } catch {
    // Nothing to clean up where storage cannot be written.
  }

  let choice = parseThemeChoice(stored);
  const prefersLight = () => env.media?.matches ?? false;
  let active = resolvePalette(choice, prefersLight());

  const choiceStore = writable<ThemeChoice>(choice);
  const paletteStore = writable<Palette>(active);

  function apply() {
    active = resolvePalette(choice, prefersLight());
    // Dark and Light pin the palette; System leaves it to the stylesheet's
    // media query, so the page follows the system without this script.
    if (choice === "system") env.root?.removeAttribute("data-theme");
    else env.root?.setAttribute("data-theme", choice);
    const background = env.readBackground().trim();
    if (background) env.setThemeColor(background);
    paletteStore.set(active);
  }

  function set(next: ThemeChoice) {
    choice = next;
    try {
      env.storage?.setItem(STORAGE_KEY, next);
    } catch {
      // The pick still holds for the open page.
    }
    apply();
    choiceStore.set(next);
  }

  env.media?.addEventListener("change", () => {
    if (choice === "system") apply();
  });
  apply();

  return {
    subscribe: choiceStore.subscribe,
    palette: { subscribe: paletteStore.subscribe },
    set,
    toggle: () => set(toggledChoice(active)),
  };
}

/** The page's real storage, media query and root; all null outside a browser. */
export function browserThemeEnv(): ThemeEnv {
  const doc = typeof document !== "undefined" ? document : null;
  let storage: ThemeEnv["storage"] = null;
  try {
    storage = typeof localStorage !== "undefined" ? localStorage : null;
  } catch {
    // Reading the property itself throws where site data is blocked.
  }
  const media =
    typeof window !== "undefined" && typeof window.matchMedia === "function"
      ? window.matchMedia("(prefers-color-scheme: light)")
      : null;
  return {
    storage,
    media: media && typeof media.addEventListener === "function" ? media : null,
    root: doc?.documentElement ?? null,
    readBackground: () =>
      doc ? getComputedStyle(doc.documentElement).getPropertyValue("--am-bg") : "",
    setThemeColor: (color) =>
      doc?.querySelector('meta[name="theme-color"]')?.setAttribute("content", color),
  };
}

export const theme = createThemeStore(browserThemeEnv());
