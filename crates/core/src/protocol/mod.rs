//! Protocol backends: HTTP, Usenet, HLS, DASH.

pub mod dash;
pub mod hls;
pub mod http;
pub mod usenet;

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use self::http::DownloadProgress;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Protocol {
    Http,
    Hls,
    Dash,
    Usenet,
}

/// Everything a protocol backend needs to perform a download.
#[derive(Debug, Clone)]
pub struct DownloadJob {
    pub url: String,
    pub download_id: String,
    pub download_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub filename: Option<String>,
    /// Extra headers (e.g. Referer, Authorization from plugin resolution).
    pub headers: HashMap<String, String>,
    /// HTTP-specific: max parallel chunks (0 = auto).
    pub max_chunks: u32,
    /// HTTP-specific: user agent string.
    pub user_agent: String,
}

/// Resolve once the cancellation flag flips to `true`.
///
/// Cancellation is carried over a `watch::<bool>` channel (instead of a
/// one-shot) so the same signal survives across every retry attempt — a
/// download stuck in the retry loop must still be cancellable. If the sender
/// is dropped (download finished, nobody can cancel any more) this future
/// stays pending forever so a `select!` never spuriously resolves it.
pub async fn wait_for_cancel(rx: &mut watch::Receiver<bool>) {
    if *rx.borrow() {
        return;
    }
    while rx.changed().await.is_ok() {
        if *rx.borrow() {
            return;
        }
    }
    std::future::pending::<()>().await;
}

/// Unified trait for all protocol backends.
#[async_trait::async_trait]
pub trait ProtocolBackend: Send + Sync {
    /// Which protocol this backend handles.
    fn protocol(&self) -> Protocol;

    /// Execute the download. Returns (bytes_downloaded, output_path).
    async fn download(
        &self,
        job: &DownloadJob,
        progress_tx: watch::Sender<DownloadProgress>,
        cancel_rx: watch::Receiver<bool>,
    ) -> Result<(u64, PathBuf), crate::Error>;
}

/// One file of a resolved URL.
#[derive(Debug, Clone)]
pub struct ResolvedDownload {
    /// The actual download URL (may differ from the original page URL).
    pub url: String,
    pub filename: Option<String>,
    pub filesize: Option<u64>,
    pub protocol: Protocol,
    /// Extra HTTP headers to send with every transfer request (e.g.
    /// Referer, Authorization). Cookies arrive merged into `Cookie`.
    pub headers: HashMap<String, String>,
}

/// Result of resolving a URL through plugins or extractors: every file the
/// URL stands for (one for a file link, many for a folder or album).
#[derive(Debug, Clone)]
pub struct ResolvedPackage {
    /// Who resolved it (the plugin id); also names a package folder whose
    /// own name is empty.
    pub resolver: String,
    /// Package name, e.g. the folder or album title.
    pub name: String,
    pub downloads: Vec<ResolvedDownload>,
}

/// Trait for URL resolution — plugins, extractors, or custom resolvers.
///
/// Defined in core to avoid circular dependencies. Implemented in the server
/// crate by wrapping PluginLoader and Extractor registry.
#[async_trait::async_trait]
pub trait UrlResolver: Send + Sync {
    /// Resolve `url`; `None` when no resolver claims it or resolution failed.
    async fn resolve(&self, url: &str) -> Option<ResolvedPackage>;

    /// The resolver (plugin id) that would handle `url`, without resolving it.
    async fn claimant(&self, _url: &str) -> Option<String> {
        None
    }
}

/// Headers Krake sets itself on transfer requests; a resolver's values for
/// them are dropped.
pub const RESERVED_TRANSFER_HEADERS: &[&str] = &[
    "host",
    "content-length",
    "range",
    "transfer-encoding",
    "connection",
];

/// Merge a resolver's headers and cookies into the headers a transfer
/// sends: reserved headers are dropped, and cookies join an existing
/// `Cookie` header into one.
pub fn transfer_headers(
    headers: &HashMap<String, String>,
    cookies: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut out: HashMap<String, String> = HashMap::new();
    let mut cookie_parts: Vec<String> = Vec::new();
    let mut names: Vec<&String> = headers.keys().collect();
    names.sort();
    for name in names {
        let lower = name.to_ascii_lowercase();
        if RESERVED_TRANSFER_HEADERS.contains(&lower.as_str()) {
            continue;
        }
        if lower == "cookie" {
            cookie_parts.push(headers[name].clone());
        } else {
            out.insert(name.clone(), headers[name].clone());
        }
    }
    let mut cookie_names: Vec<&String> = cookies.keys().collect();
    cookie_names.sort();
    for name in cookie_names {
        cookie_parts.push(format!("{name}={}", cookies[name]));
    }
    if !cookie_parts.is_empty() {
        out.insert("Cookie".to_string(), cookie_parts.join("; "));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn ac04_cookies_join_an_existing_cookie_header() {
        let out = transfer_headers(
            &map(&[("Cookie", "a=1"), ("Referer", "https://x/")]),
            &map(&[("accountToken", "t"), ("b", "2")]),
        );
        assert_eq!(out["Cookie"], "a=1; accountToken=t; b=2");
        assert_eq!(out["Referer"], "https://x/");
    }

    #[test]
    fn ac03_reserved_headers_are_dropped() {
        let out = transfer_headers(
            &map(&[
                ("Host", "evil"),
                ("range", "bytes=0-1"),
                ("Content-Length", "1"),
                ("Transfer-Encoding", "chunked"),
                ("Connection", "close"),
                ("X-Keep", "yes"),
            ]),
            &HashMap::new(),
        );
        assert_eq!(out, map(&[("X-Keep", "yes")]));
    }

    #[tokio::test]
    async fn wait_for_cancel_resolves_when_flag_already_true() {
        let (tx, mut rx) = watch::channel(false);
        tx.send_replace(true);
        // Already cancelled: must resolve immediately, not hang.
        tokio::time::timeout(std::time::Duration::from_secs(1), wait_for_cancel(&mut rx))
            .await
            .expect("should resolve immediately when flag is already true");
    }

    #[tokio::test]
    async fn wait_for_cancel_resolves_when_flag_flips() {
        let (tx, mut rx) = watch::channel(false);
        let waiter = tokio::spawn(async move { wait_for_cancel(&mut rx).await });
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        tx.send_replace(true);
        tokio::time::timeout(std::time::Duration::from_secs(1), waiter)
            .await
            .expect("should resolve once flag flips")
            .expect("task should not panic");
    }

    #[tokio::test]
    async fn wait_for_cancel_survives_across_cloned_receivers() {
        // Each retry attempt clones a fresh receiver from the same sender;
        // a single cancel must be observable by a receiver cloned afterwards.
        let (tx, rx) = watch::channel(false);
        tx.send_replace(true);
        let mut attempt_rx = rx.clone();
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            wait_for_cancel(&mut attempt_rx),
        )
        .await
        .expect("cloned receiver must observe the cancellation");
    }

    #[tokio::test]
    async fn wait_for_cancel_stays_pending_without_cancel() {
        let (_tx, mut rx) = watch::channel(false);
        // No cancel sent: the future must not resolve.
        let res = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            wait_for_cancel(&mut rx),
        )
        .await;
        assert!(res.is_err(), "should still be pending when not cancelled");
    }
}
