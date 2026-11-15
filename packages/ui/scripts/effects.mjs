/**
 * Finds what is left of the former neon effects in a source file
 * (docs/specs/amigo-theme.md, AC-03). Shared by `check-theme.mjs` and the
 * repository's `scripts/ui-lint.mjs`.
 *
 * Two findings:
 *
 *   no-neon-name   one of the former neon names: a property, class, store or
 *                  keyframe containing `neon`, `glow`, `scanline`, `flicker`,
 *                  or the `applyIntensity` function.
 *   no-glow-shadow a `box-shadow`, `text-shadow` or `drop-shadow()` layer that
 *                  is blurred and coloured with an accent or status colour (or
 *                  `currentcolor`). The neutral elevations `--elev-1..3` and
 *                  unblurred shapes (the 3px selected-edge bar, a focus ring)
 *                  are not glow.
 *
 * Comments do not count: they describe, they do not style.
 */

const NEON_NAME = /neon|glow|scanline|flicker|applyIntensity/i;

/** A colour that makes a blurred shadow a glow. */
const GLOW_COLOUR =
  /--(?:am-)?(?:accent|success|warning|danger|focus|status|btn)[\w-]*|currentcolor/i;

/** A shadow given as one custom property: only the elevations are allowed. */
const SHADOW_VAR = /^var\(\s*(--[\w-]+)\s*(?:,[^)]*)?\)$/;

/** Blank out comments (CSS, JS line and block, HTML) but keep the lines. */
export function stripComments(text) {
  const blank = (c) => c.replace(/[^\n]/g, " ");
  return text
    .replace(/\/\*[\s\S]*?\*\//g, blank)
    .replace(/<!--[\s\S]*?-->/g, blank)
    .replace(/(^|[^:"'`\\])\/\/[^\n]*/g, (m, lead) => lead + blank(m.slice(lead.length)));
}

/** Split at top-level commas, respecting nested parentheses. */
function splitLayers(value) {
  const out = [];
  let depth = 0;
  let cur = "";
  for (const ch of value) {
    if (ch === "(") depth++;
    if (ch === ")") depth--;
    if (ch === "," && depth === 0) { out.push(cur.trim()); cur = ""; continue; }
    cur += ch;
  }
  if (cur.trim()) out.push(cur.trim());
  return out;
}

/** The lengths of one shadow layer that stand outside any function. */
function topLevelLengths(layer) {
  let depth = 0;
  let flat = "";
  for (const ch of layer) {
    if (ch === "(") { depth++; flat += " "; continue; }
    if (ch === ")") { depth--; flat += " "; continue; }
    flat += depth === 0 ? ch : " ";
  }
  return flat.split(/\s+/).filter((t) => /^-?(?:\d+\.?\d*|\.\d+)[a-z%]*$/i.test(t));
}

function isZero(length) {
  return parseFloat(length) === 0;
}

/** Is this layer a glow? `value` is one comma-separated layer of a shadow. */
export function isGlowLayer(layer) {
  const v = layer.trim();
  if (!v || v === "none" || v === "inherit" || v === "initial" || v === "unset") return false;
  const single = SHADOW_VAR.exec(v);
  if (single) return !/^--(?:am-)?elev-[0-3]$/.test(single[1]);
  if (!GLOW_COLOUR.test(v)) return false;
  const lengths = topLevelLengths(v);
  // offset-x offset-y blur [spread]: no third length means no blur. A blur
  // that is not a plain number (a var(), a template expression) counts as one.
  if (lengths.length < 2) return true;
  if (lengths.length === 2) return /var\(|calc\(|\{/.test(v.replace(GLOW_COLOUR, ""));
  return !isZero(lengths[2]);
}

function lineOf(text, index) {
  return text.slice(0, index).split("\n").length;
}

/**
 * @param {string} source file contents
 * @returns {{line: number, id: string, message: string, excerpt: string}[]}
 */
export function findNeonEffects(source) {
  const text = stripComments(source);
  const out = [];
  const lines = text.split("\n");

  lines.forEach((line, i) => {
    const m = NEON_NAME.exec(line);
    if (m) {
      out.push({
        line: i + 1,
        id: "no-neon-name",
        message:
          `"${m[0]}" is one of the former neon names. The web UI has no glow, scanlines, ` +
          "flicker or intensity setting (docs/specs/amigo-theme.md).",
        excerpt: line.trim(),
      });
    }
  });

  const glow = (index, excerpt) =>
    out.push({
      line: lineOf(text, index),
      id: "no-glow-shadow",
      message:
        "A blurred shadow in an accent or status colour is a glow. Use --elev-1..3 for depth " +
        "and a border or an unblurred edge bar for state.",
      excerpt: excerpt.replace(/\s+/g, " ").trim(),
    });

  // Declarations, also inside style="" attributes and across lines.
  const decl = /(?<![\w-])(box-shadow|text-shadow)\s*:\s*([^;{}"]+)/g;
  let m;
  while ((m = decl.exec(text)) !== null) {
    // `transition: box-shadow 0.2s` is not a declaration of the shadow.
    if (splitLayers(m[2]).some(isGlowLayer)) glow(m.index, m[0]);
  }

  // filter: drop-shadow(...)
  const drop = /drop-shadow\(/g;
  while ((m = drop.exec(text)) !== null) {
    let depth = 0;
    let end = -1;
    for (let i = m.index + "drop-shadow".length; i < text.length; i++) {
      if (text[i] === "(") depth++;
      else if (text[i] === ")") {
        depth--;
        if (depth === 0) { end = i; break; }
      }
    }
    if (end === -1) continue;
    const body = text.slice(m.index + "drop-shadow(".length, end);
    if (isGlowLayer(body)) glow(m.index, text.slice(m.index, end + 1));
  }

  return out.sort((a, b) => a.line - b.line);
}

/** `ui-lint-disable-next-line <id>` on the line before, as in ui-lint.mjs. */
export function isDisabled(sourceLines, line, id) {
  const prev = sourceLines[line - 2] ?? "";
  return /ui-lint-disable-next-line\s+([\w-]+)/.exec(prev)?.[1] === id;
}
