//! Background tasks: NZB watch folder monitoring, RSS feed polling, and
//! periodic plugin auto-updates.

use std::path::Path;
use std::sync::{Arc, LazyLock};

use krake_core::coordinator::Coordinator;
use krake_plugin_runtime::updater::PluginUpdater;
use regex::Regex;
use tokio::time::{Duration, interval};
use tracing::{debug, info, warn};

// RSS/Atom NZB-link patterns are static; compile once instead of on every
// feed poll cycle (runs every 10-60s per configured feed).
static NZB_ENCLOSURE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<enclosure[^>]+url=["']([^"']+\.nzb[^"']*)["']"#).unwrap());
static NZB_LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<link[^>]*>([^<]+\.nzb[^<]*)</link>"#).unwrap());
static NZB_HREF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"href=["']([^"']+\.nzb[^"']*)["']"#).unwrap());

/// Start all background tasks.
pub fn spawn_background_tasks(
    coordinator: Arc<Coordinator>,
    http_client: reqwest::Client,
    plugin_updater: Arc<PluginUpdater>,
) {
    spawn_plugin_auto_update(coordinator.clone(), plugin_updater);
    // NZB watch folder — check every 10 seconds (only when usenet feature enabled)
    let coord = coordinator.clone();
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(10));
        loop {
            ticker.tick().await;
            if !coord.config().await.features.usenet {
                continue;
            }
            if let Err(e) = check_nzb_watch_folder(&coord).await {
                debug!("NZB watch folder check: {e}");
            }
        }
    });

    // RSS feed poller — check every 60 seconds (individual feed intervals checked inside)
    let coord = coordinator.clone();
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(60));
        loop {
            ticker.tick().await;
            if let Err(e) = poll_rss_feeds(&coord, &http_client).await {
                debug!("RSS poll cycle: {e}");
            }
        }
    });
}

/// Periodically ask the plugin updater to install upgrades for every
/// installed plugin. Controlled by `update.auto_update_plugins` in config
/// (opt-in — defaults to off) with a lower bound of 1 hour between ticks
/// to stop misconfigured short intervals from hammering the registry.
fn spawn_plugin_auto_update(coordinator: Arc<Coordinator>, plugin_updater: Arc<PluginUpdater>) {
    tokio::spawn(async move {
        // Sleep once before the first tick so we don't collide with startup
        // plugin discovery. Re-read config each iteration so the user can
        // toggle `auto_update_plugins` without a restart.
        let mut ticker = interval(Duration::from_hours(1));
        ticker.tick().await; // fires immediately — swallow that
        loop {
            ticker.tick().await;
            let cfg = coordinator.config().await.update;
            if !cfg.auto_check || !cfg.auto_update_plugins {
                continue;
            }
            // Re-align the interval if the user changed `check_interval_hours`.
            let desired = Duration::from_hours(cfg.check_interval_hours.max(1));
            if ticker.period() != desired {
                ticker = interval(desired);
                ticker.tick().await;
            }
            match plugin_updater.update_all_plugins().await {
                Ok(updated) if !updated.is_empty() => {
                    let names: Vec<&str> = updated.iter().map(|m| m.id.as_str()).collect();
                    info!(
                        "Auto-updated {} plugin(s): {}",
                        updated.len(),
                        names.join(", ")
                    );
                }
                Ok(_) => debug!("Plugin auto-update: nothing to do"),
                Err(e) => warn!("Plugin auto-update failed: {e}"),
            }
        }
    });
}

/// Check the NZB watch folder for new .nzb files and import them.
async fn check_nzb_watch_folder(coordinator: &Arc<Coordinator>) -> Result<(), krake_core::Error> {
    let watch_dir = coordinator
        .storage()
        .get_update_state("nzb_watch_dir")
        .await?
        .unwrap_or_default();

    if watch_dir.is_empty() {
        return Ok(());
    }

    let dir = Path::new(&watch_dir);
    if !dir.exists() {
        return Ok(());
    }

    let mut entries = tokio::fs::read_dir(dir).await?;
    let processed_dir = dir.join(".processed");

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();

        if !name.ends_with(".nzb") {
            continue;
        }

        info!("NZB watch folder: importing {:?}", path);

        let nzb_data = match tokio::fs::read_to_string(&path).await {
            Ok(data) => data,
            Err(e) => {
                warn!("Failed to read NZB from watch folder: {e}");
                continue;
            }
        };
        let filename = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("nzb-import")
            .to_string();

        let id = match coordinator
            .add_nzb_download(&filename, &nzb_data, None)
            .await
        {
            Ok(id) => id,
            Err(e) => {
                warn!("Failed to add NZB from watch folder: {e}");
                continue;
            }
        };

        info!("NZB imported from watch folder: {filename} → {id}");

        // Move to processed subfolder
        tokio::fs::create_dir_all(&processed_dir).await.ok();
        let dest = processed_dir.join(entry.file_name());
        if let Err(e) = tokio::fs::rename(&path, &dest).await {
            // If rename fails (cross-device), copy + delete
            if tokio::fs::copy(&path, &dest).await.is_ok() {
                let _ = tokio::fs::remove_file(&path).await;
            } else {
                warn!("Failed to move processed NZB: {e}");
            }
        }
    }

    Ok(())
}

/// Poll all enabled RSS feeds and import new NZB links.
async fn poll_rss_feeds(
    coordinator: &Arc<Coordinator>,
    http_client: &reqwest::Client,
) -> Result<(), krake_core::Error> {
    // Check if RSS feature is enabled
    let flags = coordinator
        .storage()
        .get_update_state("feature_flags")
        .await?
        .unwrap_or_default();

    let rss_enabled = serde_json::from_str::<serde_json::Value>(&flags)
        .ok()
        .and_then(|v| v.get("rss_feeds")?.as_bool())
        .unwrap_or(false);

    if !rss_enabled {
        return Ok(());
    }

    let feeds = coordinator.storage().list_rss_feeds().await?;

    for feed in &feeds {
        if !feed.enabled {
            continue;
        }

        // Check if enough time has passed since last check
        if let Some(ref last_check) = feed.last_check
            && let Ok(last) = chrono::NaiveDateTime::parse_from_str(last_check, "%Y-%m-%d %H:%M:%S")
        {
            let now = chrono::Utc::now().naive_utc();
            let elapsed = now - last;
            if elapsed.num_minutes() < feed.interval_minutes as i64 {
                continue;
            }
        }

        debug!("Polling RSS feed: {} ({})", feed.name, feed.url);

        match fetch_and_process_feed(coordinator, http_client, feed).await {
            Ok(count) => {
                if count > 0 {
                    info!("RSS feed '{}': imported {count} new items", feed.name);
                }
                coordinator
                    .storage()
                    .update_rss_feed_status(&feed.id, None)
                    .await?;
            }
            Err(e) => {
                warn!("RSS feed '{}' error: {e}", feed.name);
                coordinator
                    .storage()
                    .update_rss_feed_status(&feed.id, Some(&e.to_string()))
                    .await?;
            }
        }
    }

    Ok(())
}

/// Fetch an RSS/Atom feed and process new items.
async fn fetch_and_process_feed(
    coordinator: &Arc<Coordinator>,
    http_client: &reqwest::Client,
    feed: &krake_core::storage::RssFeedRow,
) -> Result<u32, krake_core::Error> {
    // Re-validate the feed URL on every poll. Even though add_rss_feed
    // checked it at insert time, a DNS A-record can later flip to a private
    // IP — a textbook SSRF rebinding pattern.
    crate::net_guard::validate_outbound_url(&feed.url, false)
        .await
        .map_err(|e| krake_core::Error::Other(format!("RSS feed URL rejected: {e}")))?;

    let resp = http_client
        .get(&feed.url)
        .header("User-Agent", "krake/0.1.0")
        .send()
        .await?;

    let body = resp.text().await?;
    let mut imported = 0;

    // Simple RSS/Atom parsing: find <link> or <enclosure> with .nzb URLs
    // This is a lightweight approach; a full XML parser could be added later
    for url in extract_nzb_links(&body) {
        let guid = &url; // Use URL as GUID if no explicit GUID

        if coordinator
            .storage()
            .is_rss_item_seen(&feed.id, guid)
            .await?
        {
            continue;
        }

        let category = Some(feed.category.as_str()).filter(|c| !c.is_empty());
        let added = match fetch_nzb(http_client, &url).await {
            Ok(nzb_data) => add_rss_nzb(coordinator, &url, &nzb_data, category).await,
            Err(e) => Err(e),
        };
        match added {
            Ok(_id) => {
                coordinator
                    .storage()
                    .mark_rss_item_seen(&feed.id, guid, None)
                    .await?;
                imported += 1;
            }
            Err(e) => warn!("RSS: failed to add {url}: {e}"),
        }
    }

    Ok(imported)
}

/// Cap on an NZB fetched from a feed; real NZBs are a few MB at most.
const MAX_NZB_BYTES: usize = 64 * 1024 * 1024;

/// Fetch the NZB a feed item links to.
async fn fetch_nzb(http_client: &reqwest::Client, url: &str) -> Result<String, krake_core::Error> {
    // The item URL comes from the feed, so it gets the same SSRF guard as
    // the feed itself.
    crate::net_guard::validate_outbound_url(url, false)
        .await
        .map_err(|e| krake_core::Error::Other(format!("NZB URL rejected: {e}")))?;
    let resp = http_client
        .get(url)
        .header("User-Agent", "krake/0.1.0")
        .send()
        .await?
        .error_for_status()?;
    if resp
        .content_length()
        .is_some_and(|len| len > MAX_NZB_BYTES as u64)
    {
        return Err(krake_core::Error::Other(format!(
            "NZB at {url} is too large"
        )));
    }
    let body = resp.bytes().await?;
    if body.len() > MAX_NZB_BYTES {
        return Err(krake_core::Error::Other(format!(
            "NZB at {url} is too large"
        )));
    }
    String::from_utf8(body.to_vec())
        .map_err(|e| krake_core::Error::Other(format!("NZB at {url} is not UTF-8: {e}")))
}

/// Queue an NZB fetched from a feed as a Usenet download named after its
/// URL, in the feed's category: stored in its metadata, files in
/// `<download dir>/<category>/<name>`.
async fn add_rss_nzb(
    coordinator: &Arc<Coordinator>,
    url: &str,
    nzb_data: &str,
    category: Option<&str>,
) -> Result<String, krake_core::Error> {
    coordinator
        .add_nzb_download(&rss_nzb_name(url), nzb_data, category)
        .await
}

/// The release name of an NZB URL: its last path segment without `.nzb`.
fn rss_nzb_name(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let segment = path.rsplit('/').next().unwrap_or_default();
    let stem = match segment.len().checked_sub(4) {
        Some(n) if n > 0 && segment[n..].eq_ignore_ascii_case(".nzb") => &segment[..n],
        _ => segment,
    };
    if stem.is_empty() {
        "rss-import".to_string()
    } else {
        stem.to_string()
    }
}

/// Extract NZB download links from RSS/Atom XML content.
fn extract_nzb_links(xml: &str) -> Vec<String> {
    let mut links = Vec::new();

    // Find enclosure URLs with .nzb
    for cap in NZB_ENCLOSURE_RE.captures_iter(xml) {
        if let Some(url) = cap.get(1) {
            links.push(url.as_str().to_string());
        }
    }

    // Find <link> elements pointing to .nzb files
    for cap in NZB_LINK_RE.captures_iter(xml) {
        if let Some(url) = cap.get(1) {
            let url = url.as_str().trim();
            if url.starts_with("http") {
                links.push(url.to_string());
            }
        }
    }

    // Find href attributes pointing to .nzb
    for cap in NZB_HREF_RE.captures_iter(xml) {
        if let Some(url) = cap.get(1) {
            links.push(url.as_str().to_string());
        }
    }

    // Deduplicate
    links.sort();
    links.dedup();
    links
}

#[cfg(test)]
mod tests {
    use super::*;

    const NZB: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<nzb xmlns="http://www.newzbin.com/DTD/2003/nzb">
  <file poster="p@example.com" date="1700000000" subject="&quot;abc.part01.rar&quot; yEnc (1/1)">
    <groups><group>alt.binaries.test</group></groups>
    <segments><segment bytes="2048" number="1">a@example.com</segment></segments>
  </file>
</nzb>"#;

    /// A coordinator over real directories whose queue never starts a
    /// transfer.
    fn idle_coordinator(root: &Path) -> Arc<Coordinator> {
        let storage = krake_core::storage::Storage::open(
            root.join("krake.db"),
            root.join("downloads"),
            root.join("tmp"),
        )
        .expect("storage");
        let config = krake_core::config::Config {
            max_concurrent_downloads: 0,
            ..Default::default()
        };
        Arc::new(Coordinator::new(config, storage))
    }

    #[tokio::test]
    async fn watch_folder_nzbs_become_usenet_downloads_of_their_own() {
        let root = tempfile::tempdir().unwrap();
        let coordinator = idle_coordinator(root.path());
        let watch = root.path().join("watch");
        std::fs::create_dir_all(&watch).unwrap();
        std::fs::write(watch.join("First.Release.nzb"), NZB).unwrap();
        std::fs::write(watch.join("Second.Release.nzb"), NZB).unwrap();
        coordinator
            .storage()
            .set_update_state("nzb_watch_dir", &watch.to_string_lossy())
            .await
            .unwrap();

        check_nzb_watch_folder(&coordinator).await.unwrap();

        let mut rows = coordinator.storage().list_downloads().await.unwrap();
        rows.sort_by(|a, b| a.filename.cmp(&b.filename));
        assert_eq!(rows.len(), 2, "each NZB is a download of its own");
        for (row, name) in rows.iter().zip(["First.Release", "Second.Release"]) {
            assert_eq!(row.protocol, "usenet");
            assert_eq!(row.filename.as_deref(), Some(name));
            let folder = root.path().join("downloads").join(name);
            assert_eq!(row.download_dir.as_deref(), folder.to_str());
            let metadata = coordinator
                .storage()
                .get_download_metadata(&row.id)
                .await
                .unwrap()
                .expect("metadata");
            let metadata: serde_json::Value = serde_json::from_str(&metadata).unwrap();
            assert_eq!(metadata["nzb_data"], NZB);
        }
        assert!(watch.join(".processed/First.Release.nzb").exists());
    }

    #[tokio::test]
    async fn an_rss_nzb_with_a_category_lands_in_its_folder() {
        let root = tempfile::tempdir().unwrap();
        let coordinator = idle_coordinator(root.path());

        let id = add_rss_nzb(
            &coordinator,
            "https://indexer.example/get/Some.Movie.2024.nzb?apikey=x",
            NZB,
            Some("movies"),
        )
        .await
        .expect("a category must not need a package");

        let row = coordinator
            .storage()
            .get_download(&id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.protocol, "usenet");
        assert_eq!(row.package_id, None);
        assert_eq!(row.filename.as_deref(), Some("Some.Movie.2024"));
        let folder = root.path().join("downloads/movies/Some.Movie.2024");
        assert_eq!(row.download_dir.as_deref(), folder.to_str());
        let metadata: serde_json::Value = serde_json::from_str(
            &coordinator
                .storage()
                .get_download_metadata(&id)
                .await
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(metadata["category"], "movies");
        assert_eq!(metadata["nzb_data"], NZB);
    }

    #[test]
    fn rss_nzb_name_is_the_last_path_segment_without_extension() {
        assert_eq!(
            rss_nzb_name("https://x.example/a/Release.Name.nzb?k=1"),
            "Release.Name"
        );
        assert_eq!(
            rss_nzb_name("https://x.example/a/Release.Name.NZB"),
            "Release.Name"
        );
        assert_eq!(rss_nzb_name("https://x.example/"), "rss-import");
    }
}
