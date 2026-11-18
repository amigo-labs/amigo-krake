//! Link lists: plain text files with one download link per line.
//!
//! The rule (spec `link-list-import`, AC-01 to AC-04) is shared with the web
//! UI (`web-ui/src/lib/linkList.ts`); both are pinned by the fixtures in
//! `crates/core/tests/fixtures/link-lists/`.

/// The links found in a link list, plus how many lines were not links.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkList {
    /// Links in order of first appearance, without duplicates.
    pub links: Vec<String>,
    /// Lines that are neither a link, blank, nor a `#` comment.
    pub not_recognized: usize,
}

/// Parse a link list from raw bytes.
///
/// Bytes that are not valid UTF-8 become U+FFFD and a leading byte order
/// mark is dropped. Lines end at LF, CRLF or CR. A line, trimmed of Unicode
/// whitespace, is a link when it starts with `http://` or `https://` (any
/// case) and has a host; the link ends at the first whitespace. Blank lines
/// and `#` comments are skipped; every other line counts as not recognized.
pub fn parse(bytes: &[u8]) -> LinkList {
    let text = String::from_utf8_lossy(bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let mut list = LinkList::default();
    let mut seen = std::collections::HashSet::new();
    for line in text.split("\r\n").flat_map(|l| l.split(['\r', '\n'])) {
        let line = line.trim_matches(char::is_whitespace);
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match link_of(line) {
            Some(link) => {
                if seen.insert(link) {
                    list.links.push(link.to_string());
                }
            }
            None => list.not_recognized += 1,
        }
    }
    list
}

/// The link of a trimmed line, or `None` when the line is not a link.
fn link_of(line: &str) -> Option<&str> {
    let rest_start = ["http://", "https://"]
        .iter()
        .find(|scheme| {
            line.get(..scheme.len())
                .is_some_and(|p| p.eq_ignore_ascii_case(scheme))
        })?
        .len();
    let link = line.split(char::is_whitespace).next()?;
    let host = link[rest_start..].split(['/', '?', '#']).next()?;
    (!host.is_empty()).then_some(link)
}
