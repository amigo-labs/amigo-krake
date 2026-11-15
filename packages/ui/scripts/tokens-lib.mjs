/**
 * Token file parsing shared by `check-contrast.mjs` and `check-theme.mjs`.
 *
 * The custom properties of the web UI come from two files, in import order:
 * the generated theme file `src/theme/tokens.css` (every shared token as
 * `--am-*`) and Krake's `src/tokens.css` (Krake's property names as aliases of
 * those tokens, plus the scales the shared theme does not have).
 *
 * The theme file is dark on `:root` and light under
 * `:root[data-theme='light']` and, inside a `prefers-color-scheme: light`
 * media query, `:root:not([data-theme='dark'])`. Both light selectors carry
 * the light palette.
 */

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const HERE = dirname(fileURLToPath(import.meta.url));
export const SRC = join(HERE, "..", "src");
export const THEME_TOKENS = join(SRC, "theme", "tokens.css");
export const KRAKE_TOKENS = join(SRC, "tokens.css");

export const THEMES = ["dark", "light"];

/** Replace every comment by blanks, keeping the line count. */
export function stripCssComments(source) {
  return source.replace(/\/\*[\s\S]*?\*\//g, (c) => c.replace(/[^\n]/g, " "));
}

/**
 * Extract every `:root...{ ... }` rule in source order.
 * The token files never nest custom properties inside another rule, so a
 * flat scan for brace-delimited blocks whose selector starts at `:root` is
 * exhaustive (a media query around such a rule is skipped over).
 */
export function parseRules(source) {
  // Strip comments first — an inline comment ahead of a declaration would
  // otherwise be glued onto the property name and silently drop the token.
  const css = stripCssComments(source);
  const rules = [];
  const re = /(^|[{}\s])(:root[^{}]*?)\{([^{}]*)\}/g;
  let m;
  while ((m = re.exec(css)) !== null) {
    const selector = m[2].trim();
    const decls = {};
    for (const d of m[3].split(";")) {
      const i = d.indexOf(":");
      if (i === -1) continue;
      const prop = d.slice(0, i).trim();
      if (!prop.startsWith("--")) continue;
      decls[prop] = d.slice(i + 1).trim().replace(/\s+/g, " ");
    }
    if (Object.keys(decls).length) rules.push({ selector, decls });
  }
  return rules;
}

const LIGHT_SELECTORS = new Set([":root[data-theme='light']", ":root:not([data-theme='dark'])"]);

/** Does this selector apply to the given palette? */
export function applies(selector, theme) {
  const s = selector.replace(/"/g, "'").replace(/\s+/g, "");
  if (s === ":root") return true;
  if (LIGHT_SELECTORS.has(s)) return theme === "light";
  return false; // unknown modifier — not part of the palette matrix
}

/** Flatten the cascade into one {prop: rawValue} map. */
export function resolveTheme(rules, theme) {
  const applicable = rules
    .filter((r) => applies(r.selector, theme))
    .map((r, i) => ({ ...r, spec: r.selector.trim() === ":root" ? 0 : 1, order: i }));

  applicable.sort((a, b) => (a.spec - b.spec) || (a.order - b.order));

  const out = {};
  for (const r of applicable) Object.assign(out, r.decls);
  return out;
}

/** The rules of the theme file followed by Krake's, as the page loads them. */
export function loadRules() {
  return [
    ...parseRules(readFileSync(THEME_TOKENS, "utf8")),
    ...parseRules(readFileSync(KRAKE_TOKENS, "utf8")),
  ];
}

/**
 * Substitute every `var(--x)` in a raw value until none is left, so two
 * properties can be compared as text. Throws on an undefined or circular
 * reference without a fallback.
 */
export function resolveRaw(value, vars, seen = new Set()) {
  let s = String(value);
  for (let guard = 0; guard < 64; guard++) {
    const start = s.indexOf("var(");
    if (start === -1) return s.replace(/\s+/g, " ").trim();
    let depth = 0;
    let end = -1;
    for (let i = start + 3; i < s.length; i++) {
      if (s[i] === "(") depth++;
      else if (s[i] === ")") {
        depth--;
        if (depth === 0) { end = i; break; }
      }
    }
    if (end === -1) throw new Error(`unbalanced var() in "${value}"`);
    const body = s.slice(start + 4, end);
    const comma = body.indexOf(",");
    const name = (comma === -1 ? body : body.slice(0, comma)).trim();
    const fallback = comma === -1 ? null : body.slice(comma + 1).trim();
    let replacement;
    if (vars[name] !== undefined && !seen.has(name)) {
      replacement = resolveRaw(vars[name], vars, new Set([...seen, name]));
    } else if (fallback !== null) {
      replacement = resolveRaw(fallback, vars, seen);
    } else {
      throw new Error(seen.has(name) ? `circular reference at ${name}` : `undefined token ${name}`);
    }
    s = s.slice(0, start) + replacement + s.slice(end + 1);
  }
  throw new Error(`var() nesting too deep in "${value}"`);
}
