//! Which requests are let in without a login (spec `local-auth-bypass`,
//! AC-02, AC-03, AC-05). Pure functions over the peer address and the
//! request headers; the auth middleware and the WebSocket call them.

use std::net::IpAddr;

use axum::http::{HeaderMap, Uri, header};
use krake_core::config::AuthMode;

/// Whether `ip` is in a private range: loopback, RFC 1918, link-local,
/// IPv6 unique-local and link-local, or an IPv4-mapped private address.
pub fn is_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_private(IpAddr::V4(v4));
            }
            let first = v6.segments()[0];
            v6.is_loopback() || (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80
        }
    }
}

/// The single `Host` of a request (or the HTTP/2 authority), if it has
/// exactly one.
fn host_value<'a>(headers: &'a HeaderMap, uri: &'a Uri) -> Option<&'a str> {
    let mut hosts = headers.get_all(header::HOST).iter();
    match (hosts.next(), hosts.next()) {
        (Some(one), None) => one.to_str().ok(),
        (None, _) => uri.authority().map(|a| a.as_str()),
        _ => None,
    }
}

/// Split `host[:port]` (IPv6 in brackets) into a lowercase host and a port.
fn split_host_port(value: &str) -> Option<(String, Option<u16>)> {
    let value = value.trim();
    if let Some(rest) = value.strip_prefix('[') {
        let (host, after) = rest.split_once(']')?;
        let port = match after.strip_prefix(':') {
            Some(p) => Some(p.parse().ok()?),
            None if after.is_empty() => None,
            None => return None,
        };
        return Some((host.to_ascii_lowercase(), port));
    }
    match value.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => {
            Some((host.to_ascii_lowercase(), Some(port.parse().ok()?)))
        }
        _ => Some((value.to_ascii_lowercase(), None)),
    }
}

/// Whether the request names this server by a local host name: an IP
/// literal, `localhost`, a name without a dot, or `.local`, `.lan`,
/// `.home.arpa`, `.internal`. Guards against DNS rebinding.
pub fn has_local_host_name(headers: &HeaderMap, uri: &Uri) -> bool {
    let Some((host, _)) = host_value(headers, uri).and_then(split_host_port) else {
        return false;
    };
    if host.is_empty() {
        return false;
    }
    host.parse::<IpAddr>().is_ok()
        || host == "localhost"
        || !host.contains('.')
        || [".local", ".lan", ".home.arpa", ".internal"]
            .iter()
            .any(|suffix| host.ends_with(suffix))
}

/// Whether the request comes from the server's own origin: no `Origin`, or
/// one matching `Host`, and `Sec-Fetch-Site`, when sent, is `same-origin` or
/// `none`.
pub fn is_same_origin(headers: &HeaderMap, uri: &Uri) -> bool {
    let mut sites = headers.get_all("sec-fetch-site").iter();
    match (sites.next(), sites.next()) {
        (None, _) => {}
        (Some(site), None) => {
            let site = site.to_str().unwrap_or_default();
            if !site.eq_ignore_ascii_case("same-origin") && !site.eq_ignore_ascii_case("none") {
                return false;
            }
        }
        _ => return false,
    }

    let mut origins = headers.get_all(header::ORIGIN).iter();
    let origin = match (origins.next(), origins.next()) {
        (None, _) => return true,
        (Some(one), None) => one.to_str().unwrap_or_default(),
        _ => return false,
    };
    let Some((scheme, rest)) = origin.split_once("://") else {
        return false;
    };
    let default_port = match scheme.to_ascii_lowercase().as_str() {
        "http" => 80,
        "https" => 443,
        _ => return false,
    };
    let (Some((origin_host, origin_port)), Some((host, host_port))) = (
        split_host_port(rest),
        host_value(headers, uri).and_then(split_host_port),
    ) else {
        return false;
    };
    origin_host == host && origin_port.unwrap_or(default_port) == host_port.unwrap_or(default_port)
}

/// Whether the request carries forwarding headers.
fn is_forwarded(headers: &HeaderMap) -> bool {
    headers.contains_key("x-forwarded-for") || headers.contains_key(header::FORWARDED)
}

/// The forwarded address list: every `X-Forwarded-For` line as one list,
/// else the `for=` entries of `Forwarded`. `None` when a line is not text:
/// dropping it could hide the entry the proxy appended (AC-05).
fn forwarded_list(headers: &HeaderMap) -> Option<Vec<String>> {
    let lines = |name: &str| -> Option<String> {
        let parts = headers
            .get_all(name)
            .iter()
            .map(|v| v.to_str().ok())
            .collect::<Option<Vec<_>>>()?;
        Some(parts.join(","))
    };
    if headers.contains_key("x-forwarded-for") {
        return Some(
            lines("x-forwarded-for")?
                .split(',')
                .map(|e| e.trim().to_string())
                .filter(|e| !e.is_empty())
                .collect(),
        );
    }
    Some(
        lines(header::FORWARDED.as_str())?
            .split(',')
            .filter_map(|element| {
                element.split(';').find_map(|pair| {
                    let (key, value) = pair.trim().split_once('=')?;
                    key.eq_ignore_ascii_case("for")
                        .then(|| value.trim().trim_matches('"').to_string())
                })
            })
            .collect(),
    )
}

/// Parse a forwarded entry: an IP, optionally with a port (`[v6]:port`).
fn parse_forwarded_ip(entry: &str) -> Option<IpAddr> {
    if let Ok(ip) = entry.parse() {
        return Some(ip);
    }
    let (host, _) = split_host_port(entry)?;
    host.parse().ok()
}

/// The client address used for the private check, or `None` when the
/// request must not be let in without login (untrusted or unreadable
/// forwarding headers).
pub fn client_address(peer: IpAddr, headers: &HeaderMap, trust_proxy: bool) -> Option<IpAddr> {
    if !is_forwarded(headers) {
        return Some(peer);
    }
    if !trust_proxy || !is_private(peer) {
        return None;
    }
    let list = forwarded_list(headers)?;
    for entry in list.iter().rev() {
        let ip = parse_forwarded_ip(entry)?;
        if !is_private(ip) {
            return Some(ip);
        }
    }
    list.first().and_then(|e| parse_forwarded_ip(e))
}

/// Whether a request without a valid login is let in under `mode`.
pub fn lets_in(
    mode: AuthMode,
    peer: IpAddr,
    headers: &HeaderMap,
    uri: &Uri,
    trust_proxy: bool,
) -> bool {
    let private_client = || client_address(peer, headers, trust_proxy).is_some_and(is_private);
    let local_request = || has_local_host_name(headers, uri) && is_same_origin(headers, uri);
    match mode {
        AuthMode::Always => false,
        AuthMode::Private => private_client() && local_request(),
        AuthMode::Never => local_request() && (!is_forwarded(headers) || private_client()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.append(
                axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                HeaderValue::from_str(v).unwrap(),
            );
        }
        h
    }

    fn uri() -> Uri {
        Uri::from_static("/api/v1/downloads")
    }

    #[test]
    fn private_ranges() {
        for a in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.1.20",
            "169.254.3.4",
            "::1",
            "fd12::1",
            "fc00::5",
            "fe80::1",
            "::ffff:192.168.1.5",
        ] {
            assert!(is_private(ip(a)), "{a} is private");
        }
        for a in [
            "8.8.8.8",
            "172.32.0.1",
            "172.15.255.255",
            "203.0.113.7",
            "2001:db8::5",
            "::ffff:8.8.8.8",
            "100.64.0.1",
        ] {
            assert!(!is_private(ip(a)), "{a} is not private");
        }
    }

    #[test]
    fn local_host_names() {
        for host in [
            "192.168.1.10:1516",
            "192.168.1.10",
            "[::1]:1516",
            "localhost:1516",
            "casa",
            "casa:1517",
            "nas.local",
            "krake.LAN:80",
            "box.home.arpa",
            "srv.internal",
        ] {
            assert!(
                has_local_host_name(&headers(&[("host", host)]), &uri()),
                "{host} is local"
            );
        }
        for host in [
            "krake.example.com",
            "evil.example",
            "casa.de:1516",
            "local",
            "x.localhost.com",
        ] {
            let local = host == "local";
            assert_eq!(
                has_local_host_name(&headers(&[("host", host)]), &uri()),
                local,
                "{host}"
            );
        }
        // Two Host headers, or none and no authority: not local.
        assert!(!has_local_host_name(
            &headers(&[("host", "casa"), ("host", "casa")]),
            &uri()
        ));
        assert!(!has_local_host_name(&headers(&[]), &uri()));
        // HTTP/2 carries the host in the authority.
        assert!(has_local_host_name(
            &headers(&[]),
            &Uri::from_static("http://casa:1516/api")
        ));
    }

    #[test]
    fn same_origin_rules() {
        let host = ("host", "casa:1517");
        assert!(is_same_origin(&headers(&[host]), &uri()), "no Origin");
        assert!(is_same_origin(
            &headers(&[host, ("origin", "http://casa:1517")]),
            &uri()
        ));
        assert!(is_same_origin(
            &headers(&[host, ("origin", "http://CASA:1517")]),
            &uri()
        ));
        assert!(!is_same_origin(
            &headers(&[host, ("origin", "http://casa:1516")]),
            &uri()
        ));
        assert!(!is_same_origin(
            &headers(&[host, ("origin", "https://evil.example")]),
            &uri()
        ));
        assert!(!is_same_origin(
            &headers(&[host, ("origin", "null")]),
            &uri()
        ));
        // A missing port means the default port of the Origin's scheme.
        assert!(is_same_origin(
            &headers(&[("host", "casa"), ("origin", "http://casa")]),
            &uri()
        ));
        assert!(is_same_origin(
            &headers(&[("host", "casa"), ("origin", "https://casa")]),
            &uri()
        ));
        assert!(is_same_origin(
            &headers(&[("host", "casa:443"), ("origin", "https://casa")]),
            &uri()
        ));
        assert!(!is_same_origin(
            &headers(&[("host", "casa:80"), ("origin", "https://casa")]),
            &uri()
        ));
        // Sec-Fetch-Site.
        for site in ["same-origin", "none"] {
            assert!(
                is_same_origin(&headers(&[host, ("sec-fetch-site", site)]), &uri()),
                "{site}"
            );
        }
        for site in ["cross-site", "same-site"] {
            assert!(
                !is_same_origin(&headers(&[host, ("sec-fetch-site", site)]), &uri()),
                "{site}"
            );
        }
        assert!(!is_same_origin(
            &headers(&[
                host,
                ("origin", "http://casa:1517"),
                ("origin", "http://casa:1517")
            ]),
            &uri()
        ));
    }

    #[test]
    fn client_address_without_forwarding_is_the_peer() {
        assert_eq!(
            client_address(ip("192.168.1.20"), &headers(&[]), false),
            Some(ip("192.168.1.20"))
        );
        assert_eq!(
            client_address(ip("8.8.8.8"), &headers(&[]), true),
            Some(ip("8.8.8.8"))
        );
    }

    #[test]
    fn forwarded_requests_need_trust_proxy_and_a_private_peer() {
        let xff = headers(&[("x-forwarded-for", "192.168.1.5")]);
        assert_eq!(
            client_address(ip("127.0.0.1"), &xff, false),
            None,
            "trust_proxy off"
        );
        assert_eq!(
            client_address(ip("203.0.113.9"), &xff, true),
            None,
            "public peer"
        );
        assert_eq!(
            client_address(ip("127.0.0.1"), &xff, true),
            Some(ip("192.168.1.5"))
        );
        let fwd = headers(&[("forwarded", "for=192.168.1.5")]);
        assert_eq!(client_address(ip("127.0.0.1"), &fwd, false), None);
    }

    #[test]
    fn forwarded_list_is_read_from_the_end_skipping_private_hops() {
        let peer = ip("172.18.0.3");
        // Client forged a private entry; the proxy appended the real address.
        let forged = headers(&[("x-forwarded-for", "192.168.1.5, 203.0.113.7")]);
        assert_eq!(client_address(peer, &forged, true), Some(ip("203.0.113.7")));
        // cloudflared -> nginx chain: the public client is found behind the private hop.
        let chain = headers(&[("x-forwarded-for", "198.51.100.4, 172.18.0.3")]);
        assert_eq!(client_address(peer, &chain, true), Some(ip("198.51.100.4")));
        // All private: the first entry.
        let lan = headers(&[("x-forwarded-for", "192.168.1.5, 172.18.0.2")]);
        assert_eq!(client_address(peer, &lan, true), Some(ip("192.168.1.5")));
        // Several header lines form one list.
        let lines = headers(&[
            ("x-forwarded-for", "192.168.1.5"),
            ("x-forwarded-for", "8.8.8.8"),
        ]);
        assert_eq!(client_address(peer, &lines, true), Some(ip("8.8.8.8")));
        // Ports are removed.
        let ports = headers(&[("x-forwarded-for", "192.168.1.5:5555, [fd00::7]:80")]);
        assert_eq!(client_address(peer, &ports, true), Some(ip("192.168.1.5")));
        // Forwarded, quoted IPv6 with port.
        let fwd = headers(&[(
            "forwarded",
            "for=\"[2001:db8::1]:4711\";proto=https, for=10.0.0.2",
        )]);
        assert_eq!(client_address(peer, &fwd, true), Some(ip("2001:db8::1")));
    }

    #[test]
    fn unreadable_forwarded_entries_are_never_let_in() {
        let peer = ip("127.0.0.1");
        for v in ["unknown", "_hidden", "garbage, 192.168.1.5", ""] {
            let h = headers(&[("x-forwarded-for", v)]);
            assert_eq!(client_address(peer, &h, true), None, "{v:?}");
        }
        // A Forwarded line that is not text is never dropped silently: a
        // forged private entry on another line must not become the client.
        let mut h = headers(&[("forwarded", "for=192.168.1.5")]);
        h.append(
            header::FORWARDED,
            HeaderValue::from_bytes(b"for=203.0.113.7\xff").unwrap(),
        );
        assert_eq!(
            client_address(peer, &h, true),
            None,
            "Forwarded with invalid bytes"
        );
        let mut h = headers(&[("x-forwarded-for", "192.168.1.5")]);
        h.append(
            "x-forwarded-for",
            HeaderValue::from_bytes(b"8.8.8.8\xff").unwrap(),
        );
        assert_eq!(
            client_address(peer, &h, true),
            None,
            "X-Forwarded-For with invalid bytes"
        );
        // An unreadable entry *before* the client (further left) is not read.
        let h = headers(&[("x-forwarded-for", "unknown, 8.8.8.8")]);
        assert_eq!(client_address(peer, &h, true), Some(ip("8.8.8.8")));
    }

    #[test]
    fn modes() {
        let local = headers(&[("host", "casa:1517")]);
        let lan = ip("192.168.1.20");
        let public = ip("203.0.113.7");
        // always: never let in.
        assert!(!lets_in(AuthMode::Always, lan, &local, &uri(), false));
        // private: private client, local host, same origin.
        assert!(lets_in(AuthMode::Private, lan, &local, &uri(), false));
        assert!(!lets_in(AuthMode::Private, public, &local, &uri(), false));
        let foreign = headers(&[("host", "krake.example.com")]);
        assert!(!lets_in(AuthMode::Private, lan, &foreign, &uri(), false));
        let cross = headers(&[("host", "casa:1517"), ("sec-fetch-site", "cross-site")]);
        assert!(!lets_in(AuthMode::Private, lan, &cross, &uri(), false));
        // never: any direct request with a local host and same origin…
        let loopback = ip("127.0.0.1");
        assert!(lets_in(AuthMode::Never, loopback, &local, &uri(), false));
        assert!(!lets_in(AuthMode::Never, loopback, &foreign, &uri(), false));
        // …but a forwarded one is checked as in private.
        let proxied = headers(&[
            ("host", "localhost:1516"),
            ("x-forwarded-for", "203.0.113.7"),
        ]);
        assert!(!lets_in(AuthMode::Never, loopback, &proxied, &uri(), true));
        assert!(!lets_in(AuthMode::Never, loopback, &proxied, &uri(), false));
        let proxied_lan = headers(&[
            ("host", "localhost:1516"),
            ("x-forwarded-for", "192.168.1.5"),
        ]);
        assert!(lets_in(
            AuthMode::Never,
            loopback,
            &proxied_lan,
            &uri(),
            true
        ));
    }
}
