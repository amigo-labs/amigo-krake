#!/usr/bin/env node
/**
 * Asserts that the web UI takes its look from the shared amigo theme
 * (docs/specs/amigo-theme.md):
 *
 *   AC-02  every Krake property listed below resolves, in the dark and in the
 *          light palette, to the value of its token in the generated theme
 *          file, and Krake's own stylesheets hold no colour literal;
 *   AC-03  no neon effect is left in `web-ui/src` and `packages/ui/src`
 *          (`effects.mjs`).
 *
 *   node scripts/check-theme.mjs
 */

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import {
  SRC, THEMES, KRAKE_TOKENS, loadRules, parseRules, resolveTheme, resolveRaw, stripCssComments,
} from "./tokens-lib.mjs";
import { findNeonEffects, isDisabled, isGlowLayer } from "./effects.mjs";

const REPO = join(SRC, "..", "..", "..");
const SCAN_DIRS = [join(REPO, "web-ui", "src"), join(REPO, "packages", "ui", "src")];
const THEME_DIR = join(SRC, "theme");

/** Krake property -> token of the theme files (without the `--am-` prefix). */
const ALIASES = {
  "--bg-deep": "bg",
  "--bg-surface": "surface",
  "--bg-surface-2": "surface-raised",
  "--hover-bg": "hover",
  "--text-primary": "text",
  "--text-secondary": "text-secondary",
  "--text-muted": "text-muted",
  "--border-color": "border",
  "--accent": "accent",
  "--accent-ink": "accent-ink",
  "--accent-solid": "accent-solid",
  "--accent-subtle": "accent-subtle",
  "--on-accent": "on-accent",
  "--success": "success",
  "--success-ink": "success-ink",
  "--warning": "warning",
  "--warning-ink": "warning-ink",
  "--danger": "danger",
  "--danger-ink": "danger-ink",
  "--status-online": "success",
  "--status-degraded": "warning",
  "--status-error": "danger",
  "--radius-xs": "radius-xs",
  "--radius-sm": "radius-sm",
  "--radius-md": "radius-md",
  "--radius-lg": "radius-lg",
  "--radius-full": "radius-full",
  "--elev-1": "elev-1",
  "--elev-2": "elev-2",
  "--elev-3": "elev-3",
  "--font-sans": "font-sans",
  "--font-mono": "font-mono",
};

const failures = [];

function walk(dir) {
  const out = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) out.push(...walk(full));
    else out.push(full);
  }
  return out;
}

const rel = (file) => relative(REPO, file).replaceAll("\\", "/");

/* ---- self-test: a scanner that finds nothing must not pass everything ---- */

function selfTest() {
  const glow = [
    "0 0 6px var(--accent)",
    "0 0 12px color-mix(in srgb, var(--status-online) 40%, transparent)",
    "var(--neon-glow-md)",
    "0 0 3px currentcolor",
    "0 0 var(--blur, 3px) currentcolor",
  ];
  const calm = [
    "var(--elev-2)",
    "inset 3px 0 0 var(--accent)",
    "0 0 0 2px color-mix(in srgb, var(--accent) 20%, transparent)",
    "0 1px 2px rgb(0 0 0 / 0.4)",
    "none",
  ];
  for (const v of glow) {
    if (!isGlowLayer(v)) { console.error(`theme self-test: "${v}" must count as glow`); process.exit(2); }
  }
  for (const v of calm) {
    if (isGlowLayer(v)) { console.error(`theme self-test: "${v}" must not count as glow`); process.exit(2); }
  }
  const sample = ".a { box-shadow: var(--elev-1),\n  0 0 8px var(--accent); }\n/* neon */\n.neon-card {}";
  const found = findNeonEffects(sample).map((f) => `${f.line}:${f.id}`).join(",");
  if (found !== "1:no-glow-shadow,4:no-neon-name") {
    console.error(`theme self-test: scan found "${found}"`);
    process.exit(2);
  }
}

/* ---- AC-02: aliases ------------------------------------------------------ */

function checkAliases() {
  const all = loadRules();
  const krake = parseRules(readFileSync(KRAKE_TOKENS, "utf8"));
  let checks = 0;

  for (const theme of THEMES) {
    const vars = resolveTheme(all, theme);
    for (const [prop, token] of Object.entries(ALIASES)) {
      checks++;
      const want = vars[`--am-${token}`];
      if (want === undefined) {
        failures.push(`${theme}: the theme file has no token --am-${token}`);
        continue;
      }
      let got;
      try {
        got = resolveRaw(`var(${prop})`, vars);
      } catch (e) {
        failures.push(`${theme}: ${prop} does not resolve — ${e.message}`);
        continue;
      }
      const expected = resolveRaw(want, vars);
      if (got !== expected) {
        failures.push(`${theme}: ${prop} is "${got}", the token ${token} is "${expected}"`);
      }
    }
  }

  // A palette of Krake's own would shadow the theme file's.
  for (const r of krake) {
    if (r.selector.replace(/\s+/g, "") !== ":root") {
      failures.push(`packages/ui/src/tokens.css: rule "${r.selector}" defines properties per palette; palettes come from the theme file`);
    }
  }
  return checks;
}

/* ---- AC-02: no colour literal in Krake's stylesheets ---------------------- */

const COLOUR_LITERAL =
  /#[0-9a-fA-F]{3,8}\b|\b(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch)\(|(?<![\w-])(?:white|black)(?![\w-])/;

function checkColourLiterals(files) {
  let n = 0;
  for (const file of files) {
    if (!file.endsWith(".css") || file.startsWith(THEME_DIR)) continue;
    n++;
    const lines = stripCssComments(readFileSync(file, "utf8")).split("\n");
    lines.forEach((line, i) => {
      const m = COLOUR_LITERAL.exec(line);
      if (m) {
        failures.push(`${rel(file)}:${i + 1}  colour literal "${m[0]}" — colours come from the theme files\n    ${line.trim()}`);
      }
    });
  }
  return n;
}

/* ---- AC-03: no neon effect ------------------------------------------------ */

function checkNeon(files) {
  let n = 0;
  for (const file of files) {
    if (!/\.(svelte|css|ts)$/.test(file) || file.endsWith(".d.ts")) continue;
    n++;
    const source = readFileSync(file, "utf8");
    const lines = source.split("\n");
    for (const f of findNeonEffects(source)) {
      if (isDisabled(lines, f.line, f.id)) continue;
      failures.push(`${rel(file)}:${f.line}  [${f.id}] ${f.message}\n    ${f.excerpt}`);
    }
  }
  return n;
}

selfTest();
const files = SCAN_DIRS.flatMap(walk).sort();
const aliasChecks = checkAliases();
const sheets = checkColourLiterals(files);
const scanned = checkNeon(files);

if (failures.length) {
  console.error(`\ntheme: ${failures.length} problem(s)\n`);
  for (const f of failures) console.error(`  ${f}`);
  console.error("");
  process.exit(1);
}

console.log(
  `theme: ${aliasChecks} alias checks passed (${THEMES.length} palettes), ` +
  `${sheets} stylesheets without a colour literal, ${scanned} files without a neon effect`,
);
