//! Links the desktop app is opened with (AC-11, AC-12): `krake://add?url=…`
//! (and the deprecated pre-rename `amigo://add?url=…`) add a download;
//! `magnet:` is declined.

/// What a launch argument asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeepLink {
    /// Queue this URL.
    Add(String),
    /// A torrent link: not supported, nothing is added.
    Magnet,
    /// A `krake:` / `amigo:` link that cannot be used; logged and ignored.
    Malformed(String),
}

/// Parse one launch argument. `None` for arguments that are no link of
/// ours (program path, flags).
pub fn parse(arg: &str) -> Option<DeepLink> {
    let lower = arg.to_ascii_lowercase();
    if lower.starts_with("magnet:") {
        return Some(DeepLink::Magnet);
    }
    let rest = ["krake://", "amigo://"]
        .iter()
        .find_map(|scheme| lower.starts_with(scheme).then(|| &arg[scheme.len()..]))?;

    let (action, query) = rest.split_once('?').unwrap_or((rest, ""));
    if !action.trim_end_matches('/').eq_ignore_ascii_case("add") {
        return Some(DeepLink::Malformed(format!("unknown action in {arg}")));
    }
    let url = query
        .split('&')
        .find_map(|pair| pair.strip_prefix("url="))
        .map(percent_decode);
    match url {
        Some(Some(url)) if is_web_url(&url) => Some(DeepLink::Add(url)),
        Some(Some(url)) => Some(DeepLink::Malformed(format!("not an http(s) URL: {url}"))),
        Some(None) => Some(DeepLink::Malformed(format!("bad encoding in {arg}"))),
        None => Some(DeepLink::Malformed(format!("no url in {arg}"))),
    }
}

/// Every link among `args` (e.g. a process's command line).
pub fn parse_all<'a>(args: impl IntoIterator<Item = &'a str>) -> Vec<DeepLink> {
    args.into_iter().filter_map(parse).collect()
}

fn is_web_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    (lower.starts_with("http://") || lower.starts_with("https://")) && url.len() > "https://".len()
}

/// Decode `%XX` escapes and `+`; `None` for broken escapes or non-UTF-8.
fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hex = s.get(i + 1..i + 3)?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ac11_add_link_with_an_encoded_url() {
        assert_eq!(
            parse("krake://add?url=https%3A%2F%2Fexample.com%2Ff%3Fa%3D1%26b%3D2"),
            Some(DeepLink::Add("https://example.com/f?a=1&b=2".into()))
        );
        assert_eq!(
            parse("krake://add/?url=https://example.com/file.zip"),
            Some(DeepLink::Add("https://example.com/file.zip".into()))
        );
    }

    #[test]
    fn ac11_the_pre_rename_scheme_works_the_same() {
        assert_eq!(
            parse("amigo://add?url=https%3A%2F%2Fexample.com%2Fx"),
            Some(DeepLink::Add("https://example.com/x".into()))
        );
    }

    #[test]
    fn ac11_malformed_links_are_reported_not_added() {
        for link in [
            "krake://add",
            "krake://add?foo=bar",
            "krake://add?url=%ZZ",
            "krake://add?url=ftp%3A%2F%2Fhost%2Fx",
            "krake://remove?url=https%3A%2F%2Fexample.com",
            "krake://add?url=javascript:alert(1)",
        ] {
            assert!(
                matches!(parse(link), Some(DeepLink::Malformed(_))),
                "{link}: {:?}",
                parse(link)
            );
        }
    }

    #[test]
    fn ac12_magnet_links_are_declined() {
        assert_eq!(parse("magnet:?xt=urn:btih:abc"), Some(DeepLink::Magnet));
        assert_eq!(parse("MAGNET:?xt=urn:btih:abc"), Some(DeepLink::Magnet));
    }

    #[test]
    fn other_arguments_are_not_links() {
        let links = parse_all([
            "/usr/bin/krake",
            "--flag",
            "krake://add?url=https%3A%2F%2Fa.b%2Fc",
        ]);
        assert_eq!(links, vec![DeepLink::Add("https://a.b/c".into())]);
    }
}
