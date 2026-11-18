// Link lists: plain text with one download link per line (spec
// `link-list-import`, AC-01 to AC-04). The same rule lives in Rust
// (`crates/core/src/linklist.rs`); both are pinned by the fixtures in
// `crates/core/tests/fixtures/link-lists/`.

export interface LinkList {
  /** Links in order of first appearance, without duplicates. */
  links: string[];
  /** Lines that are neither a link, blank, nor a `#` comment. */
  notRecognized: number;
}

// Unicode White_Space, the set Rust's `char::is_whitespace` uses. JS `\s`
// differs: it also matches U+FEFF, which the rule keeps inside a link.
const WS =
  "\\u0009-\\u000D\\u0020\\u0085\\u00A0\\u1680\\u2000-\\u200A\\u2028\\u2029\\u202F\\u205F\\u3000";
const TRIM = new RegExp(`^[${WS}]+|[${WS}]+$`, "g");
const FIRST_WORD = new RegExp(`^[^${WS}]+`);
const SCHEME = /^https?:\/\//i;

/**
 * Parse a link list. Bytes that are not valid UTF-8 become U+FFFD and a
 * leading byte order mark is dropped. Lines end at LF, CRLF or CR. A line,
 * trimmed of Unicode whitespace, is a link when it starts with `http://` or
 * `https://` (any case) and has a host; the link ends at the first
 * whitespace. Blank lines and `#` comments are skipped; every other line
 * counts as not recognized.
 */
export function parseLinkList(input: Uint8Array | string): LinkList {
  // TextDecoder drops a leading BOM and replaces invalid bytes like Rust's
  // `String::from_utf8_lossy` (both follow the WHATWG/Unicode practice).
  let text = typeof input === "string" ? input : new TextDecoder("utf-8").decode(input);
  if (typeof input === "string" && text.startsWith("\uFEFF")) text = text.slice(1);

  const links: string[] = [];
  const seen = new Set<string>();
  let notRecognized = 0;
  for (const raw of text.split(/\r\n|\r|\n/)) {
    const line = raw.replace(TRIM, "");
    if (line === "" || line.startsWith("#")) continue;
    const link = linkOf(line);
    if (link === null) {
      notRecognized += 1;
    } else if (!seen.has(link)) {
      seen.add(link);
      links.push(link);
    }
  }
  return { links, notRecognized };
}

function linkOf(line: string): string | null {
  const scheme = SCHEME.exec(line);
  if (!scheme) return null;
  const link = FIRST_WORD.exec(line)?.[0] ?? "";
  const host = link.slice(scheme[0].length).split(/[/?#]/)[0];
  return host === "" ? null : link;
}
