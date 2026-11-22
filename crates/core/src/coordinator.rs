//! Download Coordinator — orchestrates downloads across all protocols.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::{Mutex, broadcast, watch};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::bandwidth::BandwidthLimiter;
use crate::config::Config;
use crate::postprocess;
use crate::protocol::dash::DashDownloader;
use crate::protocol::hls::HlsDownloader;
use crate::protocol::http::{DownloadProgress, HttpDownloader};
use crate::protocol::usenet::UsenetConfig;
use crate::protocol::usenet::UsenetDownloader;
use crate::protocol::usenet::nntp::NntpServerConfig;
use crate::protocol::{Protocol, ResolvedDownload, ResolvedPackage, UrlResolver};
use crate::queue::QueueStatus;
use crate::retry::{RetryOutcome, RetryPolicy, retry_with_policy};
use crate::storage::{DownloadRow, Storage};

/// Events broadcast to subscribers (WebSocket clients, webhooks, etc.)
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DownloadEvent {
    Added {
        id: String,
        url: String,
    },
    Progress {
        id: String,
        progress: DownloadProgress,
    },
    StatusChanged {
        id: String,
        status: String,
    },
    Completed {
        id: String,
    },
    Failed {
        id: String,
        error: String,
    },
    Removed {
        id: String,
    },
    /// Plugin-triggered notification for the UI.
    PluginNotification {
        plugin_id: String,
        title: String,
        message: String,
    },
    /// Captcha challenge that needs manual solving via the UI.
    CaptchaChallenge {
        id: String,
        plugin_id: String,
        download_id: String,
        image_url: String,
        captcha_type: String,
    },
    /// Captcha was solved by the user.
    CaptchaSolved {
        id: String,
    },
    /// Captcha timed out without being solved.
    CaptchaTimeout {
        id: String,
    },
    /// All queued downloads are complete (queue empty).
    QueueEmpty,
}

impl DownloadEvent {
    /// The download this event is about, if any. Events tied to a specific
    /// download are scoped per-owner in the event stream; events returning
    /// `None` here (queue/plugin/captcha-resolution notifications) are global
    /// and delivered to every authenticated client.
    pub fn subject_download_id(&self) -> Option<&str> {
        match self {
            DownloadEvent::Added { id, .. }
            | DownloadEvent::Progress { id, .. }
            | DownloadEvent::StatusChanged { id, .. }
            | DownloadEvent::Completed { id }
            | DownloadEvent::Failed { id, .. }
            | DownloadEvent::Removed { id } => Some(id),
            DownloadEvent::CaptchaChallenge { download_id, .. } => Some(download_id),
            DownloadEvent::PluginNotification { .. }
            | DownloadEvent::CaptchaSolved { .. }
            | DownloadEvent::CaptchaTimeout { .. }
            | DownloadEvent::QueueEmpty => None,
        }
    }
}

/// Tracks an active download task.
struct ActiveDownload {
    /// Cancellation signal. A `watch::<bool>` (rather than a one-shot) so the
    /// flag survives across retry attempts — flipping it to `true` cancels a
    /// download even while it is mid-retry.
    cancel_tx: watch::Sender<bool>,
    progress_rx: watch::Receiver<DownloadProgress>,
}

pub struct Coordinator {
    config: Arc<Mutex<Config>>,
    storage: Storage,
    bandwidth: BandwidthLimiter,
    retry_policy: Arc<Mutex<RetryPolicy>>,
    active: Arc<Mutex<HashMap<String, ActiveDownload>>>,
    event_tx: broadcast::Sender<DownloadEvent>,
    /// Cache of download id → owning principal, used to scope real-time events
    /// per user without a DB round-trip on every (frequent) progress event.
    /// Populated at creation and lazily from storage on a miss. LRU-bounded so
    /// a long-running process can't accumulate one entry per download ever
    /// seen; an evicted entry reloads from storage on the next miss, and the
    /// recency touch keeps active/just-touched downloads resident so a
    /// `Removed` event still resolves to its owner after the row is gone.
    owners: Arc<Mutex<OwnerCache>>,
    resolvers: Vec<Arc<dyn UrlResolver>>,
    /// Signaled when a download completes/fails so the queue can advance.
    queue_notify: Arc<tokio::sync::Notify>,
    /// Serializes `try_start_next` so two concurrent callers (an `add_download`
    /// and the queue-advance loop) can't both observe the same free slot and
    /// double-start the same queued row.
    start_lock: Arc<Mutex<()>>,
}

/// Capacity of the per-coordinator broadcast channel that fans
/// `DownloadEvent`s out to every subscriber. Sized to absorb the bursts
/// produced at startup recovery (50+ status changes in flight) and during
/// large parallel-download progress fan-out.
const EVENT_CHANNEL_CAPACITY: usize = 4096;

/// Upper bound on the number of download→owner mappings retained in memory.
/// Generous enough to comfortably hold any realistic active+recent working
/// set; entries beyond it are LRU-evicted and reload from storage on demand.
const OWNER_CACHE_CAP: usize = 8192;

/// LRU-bounded cache of download id → owning principal.
///
/// The owner lookup happens on the hot path (every download-scoped event, for
/// every session-scoped WebSocket client), so it must avoid a DB round-trip in
/// the common case — but it must also not grow without bound over the life of
/// a long-running server. Entries carry a monotonically increasing "last use"
/// tick; when the map is full, inserting a new id evicts the least-recently
/// used entry. Eviction is safe: `download_owner` reloads a missing entry from
/// storage, and because a hit refreshes recency, a download that is active or
/// was just touched (e.g. one being cancelled, whose row is about to be
/// deleted) is never the eviction victim, so its `Removed` event still
/// resolves to the right owner.
struct OwnerCache {
    map: HashMap<String, (Option<String>, u64)>,
    tick: u64,
    cap: usize,
}

impl OwnerCache {
    fn new(cap: usize) -> Self {
        debug_assert!(cap > 0, "OwnerCache capacity must be non-zero");
        Self {
            map: HashMap::new(),
            tick: 0,
            cap,
        }
    }

    /// Look up an id, refreshing its recency on a hit. Returns the cached owner
    /// (the outer `Option` distinguishes a hit from a miss; the inner is the
    /// possibly-`None` owner).
    fn get(&mut self, id: &str) -> Option<Option<String>> {
        self.tick += 1;
        let tick = self.tick;
        let entry = self.map.get_mut(id)?;
        entry.1 = tick;
        Some(entry.0.clone())
    }

    /// Insert (or overwrite) an id's owner, evicting the least-recently-used
    /// entry first when the cache is full and the id is new.
    fn insert(&mut self, id: String, owner: Option<String>) {
        self.tick += 1;
        // Evict the least-recently-used entry first. This runs only on inserts
        // past capacity, so the O(n) scan is off the steady-state hot path.
        if !self.map.contains_key(&id)
            && self.map.len() >= self.cap
            && let Some(lru_id) = self
                .map
                .iter()
                .min_by_key(|(_, (_, t))| *t)
                .map(|(k, _)| k.clone())
        {
            self.map.remove(&lru_id);
        }
        self.map.insert(id, (owner, self.tick));
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.map.len()
    }

    #[cfg(test)]
    fn contains(&self, id: &str) -> bool {
        self.map.contains_key(id)
    }
}

impl Coordinator {
    pub fn new(config: Config, storage: Storage) -> Self {
        let bandwidth = BandwidthLimiter::new(config.bandwidth.clone());
        // Broadcast buffer holds DownloadEvent values for every subscriber
        // (WebSocket clients, webhook dispatcher, plugin notifications, …).
        // The previous capacity of 256 was easy to overflow at startup —
        // recovering 50 downloads or running a webhook dispatcher with a
        // slow receiver triggered Lagged() and dropped progress events. A
        // larger buffer absorbs spiky bursts; subscribers that fall behind
        // still see Lagged but with much more headroom.
        let (event_tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);

        let retry_policy = RetryPolicy::from(config.retry.clone());

        Self {
            config: Arc::new(Mutex::new(config)),
            storage,
            bandwidth,
            retry_policy: Arc::new(Mutex::new(retry_policy)),
            active: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
            owners: Arc::new(Mutex::new(OwnerCache::new(OWNER_CACHE_CAP))),
            resolvers: Vec::new(),
            queue_notify: Arc::new(tokio::sync::Notify::new()),
            start_lock: Arc::new(Mutex::new(())),
        }
    }

    /// Register URL resolvers (plugins, extractors). Called after construction.
    /// Resolvers are tried in order; first match wins.
    pub fn set_resolvers(&mut self, resolvers: Vec<Arc<dyn UrlResolver>>) {
        self.resolvers = resolvers;
    }

    /// Spawn a background loop that advances the queue when downloads complete.
    /// Must be called after wrapping the Coordinator in `Arc`.
    pub fn spawn_queue_advance_loop(self: &Arc<Self>) {
        let coordinator = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                coordinator.queue_notify.notified().await;
                if let Err(e) = coordinator.try_start_next().await {
                    warn!("Queue advance failed: {e}");
                }
            }
        });
    }

    /// Get a reference to the bandwidth limiter.
    pub fn bandwidth(&self) -> &BandwidthLimiter {
        &self.bandwidth
    }

    /// Subscribe to download events.
    pub fn subscribe(&self) -> broadcast::Receiver<DownloadEvent> {
        self.event_tx.subscribe()
    }

    /// Resolve the owning principal of a download, for per-user event scoping.
    /// Reads the in-memory cache first (populated at creation) and falls back
    /// to storage once for downloads not yet cached (e.g. recovered at startup
    /// or created before ownership existed), caching the result.
    pub async fn download_owner(&self, id: &str) -> Option<String> {
        // A hit also refreshes recency so active/just-touched downloads stay
        // resident in the LRU (see `OwnerCache`).
        if let Some(owner) = self.owners.lock().await.get(id) {
            return owner;
        }
        let owner = self
            .storage
            .get_download(id)
            .await
            .ok()
            .flatten()
            .and_then(|row| row.owner);
        self.owners
            .lock()
            .await
            .insert(id.to_string(), owner.clone());
        owner
    }

    /// Get the event sender (for wiring captcha manager, notifications, etc.)
    pub fn event_sender(&self) -> broadcast::Sender<DownloadEvent> {
        self.event_tx.clone()
    }

    /// Get a reference to storage.
    pub fn storage(&self) -> &Storage {
        &self.storage
    }

    /// Get a snapshot of the current config.
    pub async fn config(&self) -> Config {
        self.config.lock().await.clone()
    }

    /// Update the config at runtime and propagate to subsystems.
    pub async fn update_config(&self, new_config: Config) {
        self.bandwidth
            .update_config(new_config.bandwidth.clone())
            .await;
        *self.retry_policy.lock().await = RetryPolicy::from(new_config.retry.clone());
        *self.config.lock().await = new_config;
    }

    /// Add a new download and start it if slots are available. The download is
    /// unowned (visible only to admin clients in the real-time event stream);
    /// use [`add_download_with_options`](Self::add_download_with_options) to
    /// attribute it to a specific principal.
    pub async fn add_download(
        &self,
        url: &str,
        filename: Option<String>,
    ) -> Result<String, crate::Error> {
        self.add_download_with_options(url, filename, None, 0, None)
            .await
    }

    /// Add a download with category, priority, and owning principal. `owner`
    /// (e.g. a login username) scopes the download's real-time events to that
    /// principal; `None` leaves it unowned (admin-only in the event stream).
    ///
    /// Returns the id of the first download the URL stands for; see
    /// [`add_download_all`](Self::add_download_all) for folders and albums.
    pub async fn add_download_with_options(
        &self,
        url: &str,
        filename: Option<String>,
        category: Option<String>,
        priority: i32,
        owner: Option<String>,
    ) -> Result<String, crate::Error> {
        let ids = self
            .add_download_all(url, filename, category, priority, owner)
            .await?;
        ids.into_iter()
            .next()
            .ok_or_else(|| crate::Error::Other(format!("nothing was queued for {url}")))
    }

    /// Add every download `url` stands for: one for a file link, one per
    /// file for a folder or album a plugin resolves it to. Returns their
    /// ids in package order; an entry whose URL is already queued reports
    /// the existing download's id.
    pub async fn add_download_all(
        &self,
        url: &str,
        filename: Option<String>,
        category: Option<String>,
        priority: i32,
        owner: Option<String>,
    ) -> Result<Vec<String>, crate::Error> {
        // Duplicate detection: check if same URL is already queued/downloading.
        // Indexed point lookup rather than scanning every download.
        if let Some(dup_id) = self.storage.find_active_download_by_url(url).await? {
            info!("Duplicate download skipped: {} (existing: {})", url, dup_id);
            return Ok(vec![dup_id]);
        }

        // Determine download directory (category-based subdirectory if set).
        //
        // The category arrives untrusted from the REST API; sanitize_category
        // rejects traversal, absolute paths, and platform-invalid characters
        // so a caller can't escape the configured download root.
        let base_dir = self.storage.download_dir.to_string_lossy().to_string();
        let sanitized_category = category.as_deref().and_then(crate::sanitize_category);
        if let Some(raw) = category.as_deref()
            && !raw.is_empty()
            && sanitized_category.is_none()
        {
            return Err(crate::Error::Other(format!(
                "invalid category {raw:?}: must not contain path traversal, absolute paths, or NUL bytes"
            )));
        }
        let download_dir = match sanitized_category.as_deref() {
            Some(cat) => format!("{base_dir}/{cat}"),
            None => base_dir,
        };

        // Try URL resolution: plugins → extractors → raw URL fallback
        let (entries, download_dir) = match self.resolve_url(url).await {
            Some(package) => {
                let mut entries = self.expand_entries(url, &package).await;
                if entries.is_empty() {
                    return Err(crate::Error::Other(format!(
                        "plugin '{}' returned no downloads for {url}",
                        package.resolver
                    )));
                }
                if entries.len() == 1 {
                    let entry = &mut entries[0];
                    entry.filename = entry.filename.take().or(filename);
                    (entries, download_dir)
                } else {
                    let folder = package_folder_name(&package);
                    (entries, format!("{download_dir}/{folder}"))
                }
            }
            None => (
                vec![ResolvedDownload {
                    url: url.to_string(),
                    filename,
                    filesize: None,
                    protocol: detect_protocol(url),
                    headers: HashMap::new(),
                }],
                download_dir,
            ),
        };

        let mut ids = Vec::with_capacity(entries.len());
        for entry in entries {
            let id = self
                .insert_entry(
                    url,
                    entry,
                    &download_dir,
                    sanitized_category.as_deref(),
                    priority,
                    owner.clone(),
                )
                .await?;
            ids.push(id);
        }
        Ok(ids)
    }

    /// Queue one resolved file; returns its id, or the id of the download
    /// already queued under its URL.
    async fn insert_entry(
        &self,
        added_url: &str,
        entry: ResolvedDownload,
        download_dir: &str,
        category: Option<&str>,
        priority: i32,
        owner: Option<String>,
    ) -> Result<String, crate::Error> {
        let id = Uuid::new_v4().to_string();
        let row = DownloadRow {
            id: id.clone(),
            url: entry.url.clone(),
            protocol: entry.protocol.as_str().to_string(),
            filename: entry.filename,
            filesize: entry.filesize,
            status: QueueStatus::Queued.as_str().to_string(),
            priority,
            // A category is not a package: `package_id` must name a row of
            // `packages`. The category lives in the metadata.
            package_id: None,
            plugin_id: None,
            download_dir: Some(download_dir.to_string()),
            bytes_downloaded: 0,
            speed_current: 0,
            error_message: None,
            retry_count: 0,
            created_at: String::new(),
            started_at: None,
            completed_at: None,
            owner: owner.clone(),
        };

        // The partial-unique index in the schema is the authoritative
        // race-safe duplicate guard: a URL already queued returns its id.
        match self.storage.insert_download(&row).await {
            Ok(()) => {}
            Err(crate::Error::DuplicateUrl(existing_id)) => {
                info!(
                    "Duplicate download skipped: {} -> existing {}",
                    entry.url, existing_id
                );
                return Ok(existing_id);
            }
            Err(e) => return Err(e),
        }
        let mut metadata = serde_json::Map::new();
        if let Some(category) = category {
            metadata.insert("category".into(), serde_json::json!(category));
        }
        if !entry.headers.is_empty() {
            metadata.insert(REQUEST_HEADERS_KEY.into(), serde_json::json!(entry.headers));
        }
        if !metadata.is_empty() {
            self.storage
                .merge_download_metadata(&id, &serde_json::Value::Object(metadata))
                .await?;
        }
        let announced = if entry.url == added_url {
            added_url
        } else {
            entry.url.as_str()
        };
        self.announce_and_start(&id, announced, owner).await?;
        Ok(id)
    }

    /// The files a resolved package stands for. An entry the same resolver
    /// claims (a file page of an album) is resolved once more; what that
    /// returns is taken as is, except entries the resolver claims again,
    /// which are deeper than one level and skipped.
    async fn expand_entries(
        &self,
        added_url: &str,
        package: &ResolvedPackage,
    ) -> Vec<ResolvedDownload> {
        let mut out = Vec::with_capacity(package.downloads.len());
        for entry in &package.downloads {
            if entry.url == added_url || !self.claimed_by(&entry.url, &package.resolver).await {
                out.push(entry.clone());
                continue;
            }
            let Some(sub) = self.resolve_url(&entry.url).await else {
                warn!(
                    "Could not resolve entry {} of {added_url}; skipped",
                    entry.url
                );
                continue;
            };
            for inner in sub.downloads {
                if inner.url != entry.url && self.claimed_by(&inner.url, &package.resolver).await {
                    warn!(
                        "Entry {} of {added_url} is nested deeper than one level; skipped",
                        inner.url
                    );
                    continue;
                }
                out.push(inner);
            }
        }
        out
    }

    /// Whether `resolver` is the one that would handle `url`.
    async fn claimed_by(&self, url: &str, resolver: &str) -> bool {
        for r in &self.resolvers {
            if let Some(id) = r.claimant(url).await {
                return id == resolver;
            }
        }
        false
    }

    /// Add an NZB as a Usenet download named `name` (its release name).
    ///
    /// The NZB is stored with the row before the queue can start it, so the
    /// transfer always finds its segments. Every NZB is a download of its
    /// own, and its files go to a folder of their own,
    /// `<download dir>/<category>/<name>`, where post-processing also unpacks
    /// them. `category` is kept in the download's metadata.
    pub async fn add_nzb_download(
        &self,
        name: &str,
        nzb_data: &str,
        category: Option<&str>,
    ) -> Result<String, crate::Error> {
        let options = NzbOptions {
            category: category.map(str::to_string),
            ..NzbOptions::default()
        };
        Ok(self
            .add_nzb_download_with(name, nzb_data, &options)
            .await?
            .id)
    }

    /// Queue an NZB as [`Self::add_nzb_download`] does, with what NZBGet's
    /// `append` passes beyond the NZB: priority, paused, duplicate check
    /// and the media check.
    pub async fn add_nzb_download_with(
        &self,
        name: &str,
        nzb_data: &str,
        options: &NzbOptions,
    ) -> Result<NzbAdded, crate::Error> {
        let category = options.category.as_deref();
        let nzb = crate::protocol::usenet::nzb::parse_nzb(nzb_data)?;
        let file_count = nzb.files.len();
        let total_bytes: u64 = nzb.files.iter().map(|f| f.total_bytes()).sum();

        let category = category.filter(|c| !c.trim().is_empty());
        let mut parent = self.storage.download_dir.clone();
        if let Some(raw) = category {
            let sanitized = crate::sanitize_category(raw).ok_or_else(|| {
                crate::Error::Other(format!(
                    "invalid category {raw:?}: must not contain path traversal, absolute paths, or NUL bytes"
                ))
            })?;
            parent.push(sanitized);
        }

        let id = Uuid::new_v4().to_string();
        let folder = crate::sanitize_filename(name);
        let mut download_dir = parent.join(&folder);
        // A folder left by an earlier download keeps its files apart.
        if tokio::fs::try_exists(&download_dir).await.unwrap_or(false) {
            download_dir = parent.join(format!("{folder}.{}", &id[..8]));
        }

        let dupe = match &options.dupe {
            Some(d) => self.is_dupe(d).await?,
            None => false,
        };
        let status = if dupe {
            // Never queued: it goes straight to the history below.
            QueueStatus::Failed
        } else if options.paused {
            QueueStatus::Paused
        } else {
            QueueStatus::Queued
        };

        let url = format!("nzb://{id}");
        let row = DownloadRow {
            id: id.clone(),
            url: url.clone(),
            protocol: Protocol::Usenet.as_str().to_string(),
            filename: Some(name.to_string()),
            filesize: Some(total_bytes),
            status: status.as_str().to_string(),
            priority: options.priority,
            package_id: None,
            plugin_id: None,
            download_dir: Some(download_dir.to_string_lossy().into_owned()),
            bytes_downloaded: 0,
            speed_current: 0,
            error_message: None,
            retry_count: 0,
            created_at: String::new(),
            started_at: None,
            completed_at: None,
            owner: None,
        };
        self.storage.insert_download(&row).await?;

        let mut metadata = serde_json::json!({
            "file_count": file_count,
            "total_bytes": total_bytes,
            "nzb_data": nzb_data,
            "category": category.unwrap_or_default(),
            "media_check": options.media_check,
        });
        if let Some(d) = &options.dupe {
            metadata["dupe_key"] = d.key.clone().into();
            metadata["dupe_score"] = d.score.into();
            metadata["dupe_mode"] = d.mode.as_str().into();
        }
        if dupe {
            metadata["failure"] = "dupe".into();
        }
        if let Err(e) = self
            .storage
            .update_download_metadata(&id, &metadata.to_string())
            .await
        {
            // Without its NZB the row could never download.
            let _ = self.storage.delete_download(&id).await;
            return Err(e);
        }

        if dupe {
            // NZBGet keeps a skipped duplicate in its history only.
            self.storage.record_history(&id).await?;
            self.storage.delete_download(&id).await?;
            info!("NZB skipped as duplicate: {id} — {name}");
            return Ok(NzbAdded { id, dupe: true });
        }

        info!("NZB added: {id} — {name} ({file_count} files, {total_bytes} bytes)");
        self.announce_and_start(&id, &url, None).await?;
        Ok(NzbAdded { id, dupe: false })
    }

    /// NZBGet's duplicate check: whether an NZB with `dupe` is skipped
    /// because of a listed or archived item with the same key.
    async fn is_dupe(&self, dupe: &Dupe) -> Result<bool, crate::Error> {
        if dupe.key.is_empty() || dupe.mode == DupeMode::Force {
            return Ok(false);
        }
        let others = self.storage.dupes_of(&dupe.key).await?;
        Ok(others.iter().any(|(status, score)| {
            let succeeded = status == QueueStatus::Completed.as_str();
            match dupe.mode {
                DupeMode::Score => {
                    let pending = [
                        QueueStatus::Queued,
                        QueueStatus::Downloading,
                        QueueStatus::Paused,
                    ]
                    .iter()
                    .any(|s| s.as_str() == status);
                    (succeeded || pending) && *score >= dupe.score
                }
                DupeMode::All => succeeded,
                DupeMode::Force => false,
            }
        }))
    }

    /// Announce a stored download and start it if a slot is free.
    async fn announce_and_start(
        &self,
        id: &str,
        url: &str,
        owner: Option<String>,
    ) -> Result<(), crate::Error> {
        // Cache the owner so event filtering doesn't hit the DB per event.
        self.owners.lock().await.insert(id.to_string(), owner);
        let _ = self.event_tx.send(DownloadEvent::Added {
            id: id.to_string(),
            url: url.to_string(),
        });

        info!("Download added: {id} — {url}");

        // Auto-start if slots available
        self.try_start_next().await
    }

    /// Start queued downloads until either the concurrency limit is reached or
    /// the queue is empty.
    ///
    /// Holding `start_lock` for the whole pass serializes concurrent callers so
    /// they cannot both see the same free slot and double-start a row (TOCTOU);
    /// looping fills *every* free slot in one pass so recovery (which re-queues
    /// N downloads and calls this once) ramps up to the configured concurrency
    /// instead of running strictly serially.
    async fn try_start_next(&self) -> Result<(), crate::Error> {
        let _guard = self.start_lock.lock().await;

        let max = self.config.lock().await.max_concurrent_downloads;
        loop {
            let active_count = self.active.lock().await.len() as u32;
            if active_count >= max {
                break;
            }
            let queued = self
                .storage
                .list_downloads_by_status(QueueStatus::Queued)
                .await?;
            // start_download flips the row to Downloading and inserts it into
            // `active`, so the next iteration's query no longer returns it.
            let Some(next) = queued.into_iter().next() else {
                break;
            };
            self.start_download(&next.id).await?;
        }

        Ok(())
    }

    /// Start downloading a specific item.
    async fn start_download(&self, id: &str) -> Result<(), crate::Error> {
        let row = self
            .storage
            .get_download(id)
            .await?
            .ok_or_else(|| crate::Error::Other(format!("Download not found: {id}")))?;
        let request_headers =
            request_headers_of(self.storage.get_download_metadata(id).await?.as_deref());

        self.storage
            .update_download_status(id, QueueStatus::Downloading)
            .await?;
        let _ = self.event_tx.send(DownloadEvent::StatusChanged {
            id: id.to_string(),
            status: "downloading".to_string(),
        });

        let (cancel_tx, cancel_rx) = watch::channel(false);
        let (progress_tx, progress_rx) = watch::channel(DownloadProgress {
            bytes_downloaded: 0,
            total_bytes: None,
            speed_bytes_per_sec: 0,
        });

        {
            let mut active = self.active.lock().await;
            active.insert(
                id.to_string(),
                ActiveDownload {
                    cancel_tx,
                    progress_rx: progress_rx.clone(),
                },
            );
        }

        // Spawn a task to forward progress from watch channel to broadcast events
        {
            let mut progress_rx = progress_rx.clone();
            let event_tx = self.event_tx.clone();
            let id = id.to_string();
            let storage = self.storage.clone();
            tokio::spawn(async move {
                let mut size_recorded = false;
                while progress_rx.changed().await.is_ok() {
                    let p = progress_rx.borrow().clone();
                    // Persist the size once it is known, so a list fetched
                    // over REST can show a percentage, not only the socket.
                    if !size_recorded && let Some(total) = p.total_bytes {
                        size_recorded = true;
                        let _ = storage
                            .update_download_file_info(&id, None, Some(total))
                            .await;
                    }
                    let _ = event_tx.send(DownloadEvent::Progress {
                        id: id.clone(),
                        progress: p,
                    });
                }
            });
        }

        // Spawn the download task with retry support
        let storage = self.storage.clone();
        let event_tx = self.event_tx.clone();
        let download_id = id.to_string();
        let url = row.url.clone();
        let protocol = row.protocol.clone();
        let download_dir = PathBuf::from(row.download_dir.as_deref().unwrap_or("downloads"));
        let temp_dir = self.storage.temp_dir.clone();
        let config = self.config.lock().await.clone();
        let user_agent = config.http.user_agent.clone();
        let max_chunks = config.http.max_chunks_per_download;
        let active = self.active.clone();
        let retry_policy = self.retry_policy.lock().await.clone();
        let bandwidth = self.bandwidth.clone();
        let pp_config = config.postprocessing.clone();
        let queue_notify = self.queue_notify.clone();

        tokio::spawn(async move {
            // The progress watch channel lives across all retries. The
            // forwarder spawned earlier reads `progress_rx`; if we created
            // a fresh `(tx, _rx)` per retry the forwarder kept its old rx
            // and the UI silently froze at "0%" through every retry. We
            // clone the same sender into each attempt so every byte of
            // progress flows through the original watch.
            let result = retry_with_policy(&retry_policy, |attempt| {
                let ptx = progress_tx.clone();

                // The cancel signal is a `watch::<bool>` shared across every
                // attempt: each retry gets its own receiver clone, so flipping
                // the flag to `true` cancels the download even mid-retry.
                let crx = cancel_rx.clone();

                let storage = &storage;
                let event_tx = &event_tx;
                let max_retries = retry_policy.max_retries;
                let download_id = &download_id;
                let protocol = &protocol;
                let url = &url;
                let download_dir = &download_dir;
                let temp_dir = &temp_dir;
                let user_agent = &user_agent;
                let bandwidth = &bandwidth;
                let row_filename = &row.filename;
                let request_headers = &request_headers;

                async move {
                    if attempt > 0 {
                        // The wait is over: this attempt is a transfer again.
                        let _ = storage.clear_retrying_mark(download_id).await;
                        let _ = event_tx.send(DownloadEvent::StatusChanged {
                            id: download_id.to_string(),
                            status: "downloading".to_string(),
                        });
                    }

                    let job = crate::protocol::DownloadJob {
                        url: url.to_string(),
                        download_id: download_id.to_string(),
                        download_dir: download_dir.clone(),
                        temp_dir: temp_dir.clone(),
                        filename: row_filename.clone(),
                        headers: request_headers.clone(),
                        max_chunks,
                        user_agent: user_agent.to_string(),
                    };

                    let result = if protocol == "usenet" {
                        // Usenet is special: needs Storage for server configs + NZB data
                        run_usenet_download(
                            storage,
                            download_id,
                            download_dir,
                            row_filename.as_deref(),
                            ptx,
                        )
                        .await
                    } else {
                        // HTTP, HLS, DASH — use ProtocolBackend trait dispatch
                        let backend: Box<dyn crate::protocol::ProtocolBackend> =
                            match protocol.as_str() {
                                "hls" => Box::new(HlsDownloader::new(user_agent, 8)),
                                "dash" => Box::new(DashDownloader::new(user_agent, 8)),
                                _ => Box::new(HttpDownloader::new_with_headers(
                                    user_agent,
                                    &job.headers,
                                    bandwidth.clone(),
                                )),
                            };
                        backend.download(&job, ptx, crx).await
                    };

                    match result {
                        Ok(value) => RetryOutcome::Success(value),
                        Err(e @ crate::Error::Cancelled) => RetryOutcome::Abort(e),
                        Err(e) if is_permanent_failure(&e) => {
                            warn!("Download failed permanently: {download_id} — {e}");
                            RetryOutcome::Abort(e)
                        }
                        Err(e) => {
                            warn!("Download attempt failed: {download_id} — {e}");
                            // Marked before the backoff sleep, so the row
                            // reads "retrying" while it actually waits.
                            if attempt < max_retries {
                                let _ = storage
                                    .update_download_error(download_id, "retrying", attempt + 1)
                                    .await;
                                let _ = event_tx.send(DownloadEvent::StatusChanged {
                                    id: download_id.to_string(),
                                    status: "downloading".to_string(),
                                });
                            }
                            RetryOutcome::Retry(e)
                        }
                    }
                }
            })
            .await;

            match result {
                Ok((bytes, actual_path)) => {
                    let _ = storage
                        .update_download_progress(&download_id, bytes, 0)
                        .await;
                    // A Usenet job returns the first of several files; its
                    // row keeps the NZB's name.
                    let saved_as = (protocol != "usenet")
                        .then(|| actual_path.file_name().and_then(|n| n.to_str()))
                        .flatten();
                    let _ = storage
                        .update_download_file_info(&download_id, saved_as, Some(bytes))
                        .await;

                    // Run post-processing. A Usenet release that cannot be
                    // completed fails here, with its reason in the metadata.
                    let verdict = if protocol == "usenet" {
                        finish_usenet(&storage, &download_id, &download_dir, &pp_config).await
                    } else {
                        if let Err(e) = postprocess::run_pipeline(&actual_path, &pp_config).await {
                            warn!("Post-processing failed for {download_id}: {e}");
                        }
                        Ok(())
                    };

                    match verdict {
                        Ok(()) => {
                            let _ = storage
                                .update_download_status(&download_id, QueueStatus::Completed)
                                .await;
                            let _ = storage.record_history(&download_id).await;
                            let _ = event_tx.send(DownloadEvent::Completed {
                                id: download_id.clone(),
                            });
                            info!("Download completed: {download_id}");
                        }
                        Err(error_msg) => {
                            error!("Download failed: {download_id} — {error_msg}");
                            let _ = storage
                                .update_download_status(&download_id, QueueStatus::Failed)
                                .await;
                            let _ = storage
                                .update_download_error(&download_id, &error_msg, 0)
                                .await;
                            let _ = storage.record_history(&download_id).await;
                            let _ = event_tx.send(DownloadEvent::Failed {
                                id: download_id.clone(),
                                error: error_msg,
                            });
                        }
                    }
                }
                Err(crate::Error::Cancelled) => {
                    // The transfer was stopped by the user, not by a failure:
                    // pause() has already set the row to Paused and cancel() has
                    // deleted it. Overwriting the status with Failed here (the
                    // old behaviour) made a paused download show up as failed.
                    info!("Download stopped (paused/cancelled): {download_id}");
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    error!("Download failed: {download_id} — {error_msg}");
                    let _ = storage
                        .update_download_status(&download_id, QueueStatus::Failed)
                        .await;
                    let _ = storage
                        .update_download_error(&download_id, &error_msg, retry_policy.max_retries)
                        .await;
                    let _ = storage.record_history(&download_id).await;
                    let _ = event_tx.send(DownloadEvent::Failed {
                        id: download_id.clone(),
                        error: error_msg,
                    });
                }
            }

            active.lock().await.remove(&download_id);
            // Signal the queue to advance and start the next queued download
            queue_notify.notify_one();
        });

        Ok(())
    }

    /// Pause an active download.
    pub async fn pause(&self, id: &str) -> Result<(), crate::Error> {
        let mut active = self.active.lock().await;
        if let Some(dl) = active.remove(id) {
            // Progress lives in memory while a transfer runs and is only
            // written down when it ends. Pausing ends it here, so keep what
            // it had reached; otherwise the paused row reads 0 B.
            let reached = dl.progress_rx.borrow().bytes_downloaded;
            dl.cancel_tx.send_replace(true);
            self.storage
                .update_download_progress(id, reached, 0)
                .await?;
        }
        self.storage
            .update_download_status(id, QueueStatus::Paused)
            .await?;
        let _ = self.event_tx.send(DownloadEvent::StatusChanged {
            id: id.to_string(),
            status: "paused".to_string(),
        });
        Ok(())
    }

    /// Resume a paused download.
    pub async fn resume(&self, id: &str) -> Result<(), crate::Error> {
        self.storage
            .update_download_status(id, QueueStatus::Queued)
            .await?;
        self.try_start_next().await?;
        Ok(())
    }

    /// Re-queue a download, cancelling any attempt already in flight.
    ///
    /// Distinct from `resume` in two ways. It clears the recorded error, so
    /// the UI stops offering "report this failure" for an attempt that is
    /// being made again. And it cancels an active task first: `resume` is only
    /// ever reached from a paused download, which by definition is not active,
    /// but `retry` is exposed on the API and reachable for any id — including
    /// via the bulk endpoint over a mixed selection. Without the cancel, the
    /// old task would keep writing while the row flipped back to queued and
    /// `try_start_next` started a second task for the same download.
    pub async fn retry(&self, id: &str) -> Result<(), crate::Error> {
        let mut active = self.active.lock().await;
        if let Some(dl) = active.remove(id) {
            dl.cancel_tx.send_replace(true);
        }
        drop(active);

        self.storage.clear_download_error(id).await?;
        self.storage
            .update_download_status(id, QueueStatus::Queued)
            .await?;
        let _ = self.event_tx.send(DownloadEvent::StatusChanged {
            id: id.to_string(),
            status: "queued".to_string(),
        });
        self.try_start_next().await?;
        Ok(())
    }

    /// Cancel and remove a download.
    pub async fn cancel(&self, id: &str) -> Result<(), crate::Error> {
        let mut active = self.active.lock().await;
        if let Some(dl) = active.remove(id) {
            dl.cancel_tx.send_replace(true);
        }
        drop(active);
        self.storage.delete_download(id).await?;
        // Partial chunks and segments of a removed download are never
        // resumed again; leaving them would keep their bytes on disk for good.
        for prefix in ["http", "hls", "dash"] {
            let dir = self.storage.temp_dir.join(format!("{prefix}-{id}"));
            let _ = tokio::fs::remove_dir_all(dir).await;
        }
        let _ = self
            .event_tx
            .send(DownloadEvent::Removed { id: id.to_string() });
        Ok(())
    }

    /// Overlay the live progress of running transfers onto rows read from
    /// storage. Bytes and speed are only written to the database when a
    /// transfer ends, so without this a list fetched over REST shows every
    /// running download at 0 B.
    pub async fn overlay_live_progress(&self, rows: &mut [DownloadRow]) {
        let active = self.active.lock().await;
        for row in rows {
            if let Some(dl) = active.get(&row.id) {
                apply_live_progress(row, &dl.progress_rx.borrow());
            }
        }
    }

    /// Get progress for an active download.
    pub async fn get_progress(&self, id: &str) -> Option<DownloadProgress> {
        let active = self.active.lock().await;
        active.get(id).map(|dl| dl.progress_rx.borrow().clone())
    }

    /// Get count of active downloads.
    pub async fn active_count(&self) -> usize {
        self.active.lock().await.len()
    }

    /// Get current aggregate speed.
    pub async fn total_speed(&self) -> u64 {
        let active = self.active.lock().await;
        active
            .values()
            .map(|dl| dl.progress_rx.borrow().speed_bytes_per_sec)
            .sum()
    }

    /// Update download priority (for queue reordering).
    pub async fn set_priority(&self, id: &str, priority: i32) -> Result<(), crate::Error> {
        self.storage.set_download_priority(id, priority).await
    }

    /// Recover downloads that were in-progress when the app crashed.
    /// Resets "downloading" status to "queued" so they can be restarted.
    pub async fn recover_downloads(&self) -> Result<u32, crate::Error> {
        let downloading = self
            .storage
            .list_downloads_by_status(QueueStatus::Downloading)
            .await?;

        let count = downloading.len() as u32;
        for d in &downloading {
            info!("Recovering interrupted download: {} ({})", d.id, d.url);
            self.storage
                .update_download_status(&d.id, QueueStatus::Queued)
                .await?;
        }

        if count > 0 {
            info!("Recovered {count} interrupted downloads — re-queued");
            self.try_start_next().await?;
        }

        Ok(count)
    }

    /// Try each registered URL resolver in order. Returns None if no resolver matches.
    async fn resolve_url(&self, url: &str) -> Option<ResolvedPackage> {
        for resolver in &self.resolvers {
            match resolver.resolve(url).await {
                Some(resolved) => {
                    info!(
                        "URL resolved: {url} → {} download(s) by {}",
                        resolved.downloads.len(),
                        resolved.resolver
                    );
                    return Some(resolved);
                }
                None => continue,
            }
        }
        None
    }
}

/// Metadata key of the headers (cookies merged) a download's transfer sends.
pub const REQUEST_HEADERS_KEY: &str = "request_headers";

/// The folder a multi-file package is saved in: the package name made safe
/// as one path component, or the resolver's id when the name is empty.
fn package_folder_name(package: &ResolvedPackage) -> String {
    let name = package.name.replace(['/', '\\'], "_");
    if name
        .trim_matches(|c: char| c == '.' || c.is_whitespace())
        .is_empty()
    {
        crate::sanitize_filename(&package.resolver)
    } else {
        crate::sanitize_filename(&name)
    }
}

/// The headers stored with a download for its transfer requests.
fn request_headers_of(metadata: Option<&str>) -> HashMap<String, String> {
    metadata
        .and_then(|m| serde_json::from_str::<serde_json::Value>(m).ok())
        .and_then(|v| serde_json::from_value(v[REQUEST_HEADERS_KEY].clone()).ok())
        .unwrap_or_default()
}

/// Copy a running transfer's progress into its row.
fn apply_live_progress(row: &mut DownloadRow, progress: &DownloadProgress) {
    row.bytes_downloaded = progress.bytes_downloaded;
    row.speed_current = progress.speed_bytes_per_sec;
    if let Some(total) = progress.total_bytes {
        row.filesize = Some(total);
    }
}

/// Whether an error will repeat on every attempt, so retrying only delays
/// the failure: an HTTP client error other than timeout / too-early /
/// rate-limit.
fn is_permanent_failure(error: &crate::Error) -> bool {
    match error {
        crate::Error::Http(e) => e.status().is_some_and(|s| {
            s.is_client_error()
                && !matches!(
                    s,
                    reqwest::StatusCode::REQUEST_TIMEOUT
                        | reqwest::StatusCode::TOO_EARLY
                        | reqwest::StatusCode::TOO_MANY_REQUESTS
                )
        }),
        _ => false,
    }
}

/// Run a Usenet download: load server configs from DB, download NZB segments via NNTP.
async fn run_usenet_download(
    storage: &Storage,
    download_id: &str,
    download_dir: &PathBuf,
    _filename: Option<&str>,
    progress_tx: watch::Sender<DownloadProgress>,
) -> Result<(u64, PathBuf), crate::Error> {
    // Load NNTP servers from database
    let server_rows = storage.list_usenet_servers().await?;
    if server_rows.is_empty() {
        return Err(crate::Error::Other(
            "No Usenet servers configured. Add a server in Settings → Usenet Servers.".into(),
        ));
    }

    let servers: Vec<NntpServerConfig> = server_rows
        .into_iter()
        .map(|r| NntpServerConfig {
            name: r.name,
            host: r.host,
            port: r.port,
            ssl: r.ssl,
            username: r.username,
            password: r.password,
            connections: r.connections,
            priority: r.priority,
        })
        .collect();

    let usenet = UsenetDownloader::new(UsenetConfig {
        servers,
        par2_repair: true,
        auto_unrar: true,
        delete_archives_after_extract: true,
    });

    tokio::fs::create_dir_all(download_dir).await?;

    // Load NZB data from download metadata
    let metadata = storage.get_download_metadata(download_id).await?;
    let nzb_data = metadata
        .as_deref()
        .and_then(|m| serde_json::from_str::<serde_json::Value>(m).ok())
        .and_then(|v| v.get("nzb_data")?.as_str().map(String::from));

    let Some(nzb_data) = nzb_data else {
        return Err(crate::Error::Other(
            "No NZB data found in download metadata".into(),
        ));
    };
    let critical_health = crate::protocol::usenet::critical_health(
        &crate::protocol::usenet::nzb::parse_nzb(&nzb_data)?,
    );
    // A new attempt has no verdict yet.
    storage
        .merge_download_metadata(download_id, &serde_json::json!({ "failure": null }))
        .await?;

    let _ = progress_tx.send(DownloadProgress {
        bytes_downloaded: 0,
        total_bytes: None,
        speed_bytes_per_sec: 0,
    });

    // Download all files from the NZB
    info!("Starting Usenet download to {:?}", download_dir);
    let results = usenet.download_nzb(&nzb_data, download_dir).await?;

    let total_bytes: u64 = results.iter().map(|r| r.bytes).sum();
    let total_ok: u32 = results.iter().map(|r| r.segments_ok).sum();
    let total_failed: u32 = results.iter().map(|r| r.segments_failed).sum();

    info!(
        "Usenet download complete: {} files, {} bytes, {}/{} segments OK",
        results.len(),
        total_bytes,
        total_ok,
        total_ok + total_failed
    );

    // Health as NZBGet reports it: the share of articles fetched, per mille.
    let total_articles = total_ok + total_failed;
    let health = if total_articles == 0 {
        1000
    } else {
        (u64::from(total_ok) * 1000 / u64::from(total_articles)) as u32
    };
    storage
        .merge_download_metadata(
            download_id,
            &serde_json::json!({
                "total_articles": total_articles,
                "success_articles": total_ok,
                "failed_articles": total_failed,
                "health": health,
                "critical_health": critical_health,
            }),
        )
        .await?;

    let _ = progress_tx.send(DownloadProgress {
        bytes_downloaded: total_bytes,
        total_bytes: Some(total_bytes),
        speed_bytes_per_sec: 0,
    });

    // Return the first file path or the directory
    let output_path = results
        .first()
        .map(|r| r.path.clone())
        .unwrap_or_else(|| download_dir.clone());

    Ok((total_bytes, output_path))
}

/// How an NZB is queued, beyond the NZB itself: what NZBGet's `append`
/// passes.
#[derive(Debug, Clone, Default)]
pub struct NzbOptions {
    /// Files go to `<download dir>/<category>/<name>`; kept in the metadata.
    pub category: Option<String>,
    /// Queue order: higher starts first, 0 is normal. NZBGet's priorities
    /// (-100 very low … 100 very high, 900 force) are used as they are.
    pub priority: i32,
    /// Add the download paused; it starts once resumed.
    pub paused: bool,
    /// NZBGet's duplicate check; `None` checks nothing.
    pub dupe: Option<Dupe>,
    /// Fail a release that yields no media file (see
    /// [`postprocess::contains_media`]) as a bad release.
    pub media_check: bool,
}

/// NZBGet's duplicate key, score and mode of an NZB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dupe {
    pub key: String,
    pub score: i64,
    pub mode: DupeMode,
}

/// NZBGet's `DupeMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DupeMode {
    /// Skip the NZB if an item with its key and an equal or higher score is
    /// queued or was downloaded successfully.
    Score,
    /// Skip the NZB only if an item with its key was downloaded
    /// successfully, whatever its score; the queue is not checked.
    All,
    /// Never skip the NZB.
    Force,
}

impl DupeMode {
    /// NZBGet's name for the mode; unknown names read as `SCORE`, NZBGet's
    /// default.
    pub fn parse(mode: &str) -> Self {
        match mode.to_ascii_uppercase().as_str() {
            "ALL" => DupeMode::All,
            "FORCE" => DupeMode::Force,
            _ => DupeMode::Score,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            DupeMode::Score => "SCORE",
            DupeMode::All => "ALL",
            DupeMode::Force => "FORCE",
        }
    }
}

/// What [`Coordinator::add_nzb_download_with`] did with an NZB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NzbAdded {
    pub id: String,
    /// The NZB was a duplicate: it went straight to the history (NZBGet's
    /// `DELETED/DUPE`) and is not downloaded.
    pub dupe: bool,
}

/// Why a finished Usenet transfer did not yield the release, as stored in
/// the download's metadata (`failure`) and reported by the NZBGet API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseFailure {
    /// More articles are missing than the PAR2 recovery data can make up.
    Health,
    /// par2 could not repair the release.
    Par,
    /// An archive of the release could not be unpacked.
    Unpack,
    /// The release unpacked but holds no media file; checked only where
    /// asked for ([`NzbOptions::media_check`]).
    NoMedia,
}

impl ReleaseFailure {
    pub fn as_str(self) -> &'static str {
        match self {
            ReleaseFailure::Health => "health",
            ReleaseFailure::Par => "par",
            ReleaseFailure::Unpack => "unpack",
            ReleaseFailure::NoMedia => "media",
        }
    }
}

/// Check and post-process a Usenet download whose articles were fetched,
/// and record the outcome in its metadata (`failure`, `par_status`,
/// `unpack_status`). `Err` carries the error message: a broken release
/// (with `failure` set) or a problem on this machine (`failure` null).
async fn finish_usenet(
    storage: &Storage,
    id: &str,
    dir: &std::path::Path,
    pp_config: &postprocess::PostProcessConfig,
) -> Result<(), String> {
    let metadata = storage
        .get_download_metadata(id)
        .await
        .ok()
        .flatten()
        .and_then(|m| serde_json::from_str::<serde_json::Value>(&m).ok())
        .unwrap_or_default();
    let health = metadata["health"].as_u64().unwrap_or(1000);
    let critical = metadata["critical_health"].as_u64().unwrap_or(1000);

    let record = |failure: Option<ReleaseFailure>, par: &str, unpack: &str| {
        let fields = serde_json::json!({
            "failure": failure.map(ReleaseFailure::as_str),
            "par_status": par,
            "unpack_status": unpack,
        });
        async move {
            if let Err(e) = storage.merge_download_metadata(id, &fields).await {
                warn!("Could not record the Usenet result of {id}: {e}");
            }
        }
    };

    if health < critical {
        // Not even PAR2 can make up for what is missing; NZBGet does not
        // try either.
        record(Some(ReleaseFailure::Health), "NONE", "NONE").await;
        return Err(format!(
            "Too many articles missing: health {health}‰ is below the critical {critical}‰"
        ));
    }

    let outcome = match postprocess::run_usenet_pipeline(dir, pp_config).await {
        Ok(outcome) => outcome,
        Err(e) => {
            warn!("Usenet post-processing failed for {id}: {e}");
            record(None, "NONE", "NONE").await;
            return Err(format!("Post-processing failed: {e}"));
        }
    };
    let (par, unpack) = (outcome.par.as_str(), outcome.unpack.as_str());
    if outcome.par == postprocess::StageStatus::Failure {
        record(Some(ReleaseFailure::Par), par, unpack).await;
        return Err("PAR2 repair failed".into());
    }
    if outcome.unpack == postprocess::StageStatus::Failure {
        record(Some(ReleaseFailure::Unpack), par, unpack).await;
        return Err("Unpacking failed".into());
    }
    if metadata["media_check"].as_bool() == Some(true) {
        let dir = dir.to_path_buf();
        let has_media = tokio::task::spawn_blocking(move || postprocess::contains_media(&dir))
            .await
            .unwrap_or(true);
        if !has_media {
            record(Some(ReleaseFailure::NoMedia), par, unpack).await;
            return Err("The release holds no media file".into());
        }
    }
    record(None, par, unpack).await;
    Ok(())
}

fn detect_protocol(url: &str) -> Protocol {
    let path = url.split('?').next().unwrap_or(url);
    if url.ends_with(".nzb") {
        Protocol::Usenet
    } else if path.ends_with(".m3u8") || path.ends_with(".m3u") {
        Protocol::Hls
    } else if path.ends_with(".mpd") {
        Protocol::Dash
    } else {
        Protocol::Http
    }
}

impl Protocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Protocol::Http => "http",
            Protocol::Hls => "hls",
            Protocol::Dash => "dash",
            Protocol::Usenet => "usenet",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::assert_matches;

    fn stored_row(filesize: Option<u64>) -> DownloadRow {
        DownloadRow {
            id: "d1".into(),
            url: "https://example.com/file.bin".into(),
            protocol: "http".into(),
            filename: None,
            filesize,
            status: "downloading".into(),
            priority: 0,
            package_id: None,
            plugin_id: None,
            download_dir: None,
            bytes_downloaded: 0,
            speed_current: 0,
            error_message: None,
            retry_count: 0,
            created_at: String::new(),
            started_at: None,
            completed_at: None,
            owner: None,
        }
    }

    #[test]
    fn apply_live_progress_running_transfer_replaces_stored_zeroes() {
        let mut row = stored_row(None);
        apply_live_progress(
            &mut row,
            &DownloadProgress {
                bytes_downloaded: 500,
                total_bytes: Some(2000),
                speed_bytes_per_sec: 250,
            },
        );
        assert_eq!(row.bytes_downloaded, 500);
        assert_eq!(row.speed_current, 250);
        assert_eq!(row.filesize, Some(2000));
    }

    #[test]
    fn apply_live_progress_unknown_total_keeps_stored_size() {
        let mut row = stored_row(Some(4000));
        apply_live_progress(
            &mut row,
            &DownloadProgress {
                bytes_downloaded: 10,
                total_bytes: None,
                speed_bytes_per_sec: 1,
            },
        );
        assert_eq!(row.filesize, Some(4000));
    }

    #[test]
    fn owner_cache_is_bounded_and_reflects_owner() {
        let mut cache = OwnerCache::new(4);
        for i in 0..4 {
            cache.insert(format!("dl-{i}"), Some(format!("user-{i}")));
        }
        assert_eq!(cache.len(), 4);
        assert_eq!(cache.get("dl-2"), Some(Some("user-2".to_string())));
        // A miss is distinct from a cached `None` owner.
        cache.insert("unowned".to_string(), None);
        // Inserting "unowned" (5th id into a cap-4 cache) evicted one entry.
        assert_eq!(cache.len(), 4);
        assert_eq!(cache.get("unowned"), Some(None));
        assert_eq!(cache.get("missing"), None);
    }

    #[test]
    fn owner_cache_evicts_least_recently_used() {
        let mut cache = OwnerCache::new(3);
        cache.insert("a".to_string(), Some("ua".to_string()));
        cache.insert("b".to_string(), Some("ub".to_string()));
        cache.insert("c".to_string(), Some("uc".to_string()));

        // Touch "a" so it is the most-recently-used; "b" is now the LRU.
        assert!(cache.get("a").is_some());

        // Inserting a fourth id must evict the LRU ("b"), not the just-touched
        // "a" — this is what keeps a soon-to-be-`Removed` download resolvable.
        cache.insert("d".to_string(), Some("ud".to_string()));
        assert_eq!(cache.len(), 3);
        assert!(cache.contains("a"), "recently-touched entry must survive");
        assert!(cache.contains("c"));
        assert!(cache.contains("d"));
        assert!(!cache.contains("b"), "LRU entry must be evicted");
    }

    /// Regression test for #52: a cancel issued *after* the first attempt must
    /// still abort the download. This reproduces `start_download`'s wiring — a
    /// single `watch::<bool>` cancel channel whose receiver is cloned per
    /// attempt and selected on inside each attempt (as `backend.download` does
    /// via `wait_for_cancel`). With the old `oneshot` receiver, `take()`
    /// consumed it on attempt 0 and every later attempt got a dead placeholder,
    /// so a mid-retry cancel was silently a no-op.
    #[tokio::test(start_paused = true)]
    async fn cancel_is_honored_after_the_first_retry_attempt() {
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::time::Duration;
        use tokio::sync::watch;

        let (cancel_tx, cancel_rx) = watch::channel(false);
        let policy = RetryPolicy {
            max_retries: 10,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(1),
        };
        let attempts = Arc::new(AtomicU32::new(0));

        // Flip the cancel flag well after attempt 0 has failed and the loop has
        // moved on to a later attempt.
        let ctx = cancel_tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(150)).await;
            ctx.send_replace(true);
        });

        let result: Result<(), _> = retry_with_policy(&policy, async |attempt| {
            let mut crx = cancel_rx.clone();
            attempts.fetch_add(1, Ordering::SeqCst);
            if attempt == 0 {
                // First attempt fails fast, forcing the loop to retry.
                return RetryOutcome::Retry(crate::Error::Other("boom".into()));
            }
            // Later attempts run until cancelled — the exact pattern used by
            // `HttpDownloader::download`. Without observing the cancel, this
            // would keep "failing" until the retry policy is exhausted.
            tokio::select! {
                _ = crate::protocol::wait_for_cancel(&mut crx) => {
                    RetryOutcome::Abort(crate::Error::Cancelled)
                }
                _ = tokio::time::sleep(Duration::from_secs(30)) => {
                    RetryOutcome::Retry(crate::Error::Other("still failing".into()))
                }
            }
        })
        .await;

        assert_matches!(
            result,
            Err(crate::Error::Cancelled),
            "cancel must abort the retry chain, got {result:?}"
        );
        assert!(
            attempts.load(Ordering::SeqCst) >= 2,
            "cancel must have been observed on an attempt after the first"
        );
    }
}
