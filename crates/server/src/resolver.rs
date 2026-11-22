//! URL resolver that bridges plugins to the core download pipeline.

use std::sync::Arc;

use krake_core::protocol::{
    Protocol, ResolvedDownload, ResolvedPackage, UrlResolver, transfer_headers,
};
use krake_plugin_runtime::loader::PluginLoader;
use krake_plugin_runtime::types::{DownloadInfo, DownloadProtocol};

/// Wraps `PluginLoader` to implement the core `UrlResolver` trait.
pub struct PluginUrlResolver {
    loader: Arc<PluginLoader>,
}

impl PluginUrlResolver {
    pub fn new(loader: Arc<PluginLoader>) -> Self {
        Self { loader }
    }
}

#[async_trait::async_trait]
impl UrlResolver for PluginUrlResolver {
    async fn resolve(&self, url: &str) -> Option<ResolvedPackage> {
        let plugin_meta = self.loader.match_url(url).await?;

        tracing::debug!("Plugin '{}' matched URL: {url}", plugin_meta.name);

        let package = match self.loader.resolve(&plugin_meta.id, url).await {
            Ok(pkg) => pkg,
            Err(e) => {
                tracing::warn!("Plugin '{}' failed to resolve {url}: {e}", plugin_meta.name);
                return None;
            }
        };

        Some(ResolvedPackage {
            resolver: plugin_meta.id,
            name: package.name,
            downloads: package.downloads.into_iter().map(to_resolved).collect(),
        })
    }

    async fn claimant(&self, url: &str) -> Option<String> {
        self.loader.match_url(url).await.map(|m| m.id)
    }
}

/// One plugin entry as the core queues it: headers and cookies merged into
/// the headers every transfer request sends.
fn to_resolved(info: DownloadInfo) -> ResolvedDownload {
    let protocol = match info.protocol {
        DownloadProtocol::Hls => Protocol::Hls,
        DownloadProtocol::Dash => Protocol::Dash,
        DownloadProtocol::Http => Protocol::Http,
    };
    ResolvedDownload {
        url: info.url,
        filename: info.filename,
        filesize: info.filesize,
        protocol,
        headers: transfer_headers(
            &info.headers.unwrap_or_default(),
            &info.cookies.unwrap_or_default(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn ac03_ac04_plugin_headers_and_cookies_become_transfer_headers() {
        let info = DownloadInfo {
            url: "https://cdn.example/f".into(),
            filename: Some("f.bin".into()),
            filesize: Some(7),
            chunks_supported: true,
            max_chunks: None,
            headers: Some(HashMap::from([
                ("Referer".to_string(), "https://host.example/".to_string()),
                ("Cookie".to_string(), "a=1".to_string()),
            ])),
            cookies: Some(HashMap::from([("token".to_string(), "t".to_string())])),
            wait_seconds: None,
            mirrors: vec![],
            protocol: DownloadProtocol::Http,
        };
        let resolved = to_resolved(info);
        assert_eq!(resolved.headers["Referer"], "https://host.example/");
        assert_eq!(resolved.headers["Cookie"], "a=1; token=t");
        assert_eq!(resolved.filename.as_deref(), Some("f.bin"));
        assert_eq!(resolved.filesize, Some(7));
    }
}
