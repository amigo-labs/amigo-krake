//! Integration tests for core download coordinator lifecycle.

use std::path::PathBuf;

use krake_core::config::Config;
use krake_core::coordinator::{Coordinator, Dupe, DupeMode, NzbAdded, NzbOptions};
use krake_core::queue::QueueStatus;
use krake_core::storage::Storage;

fn test_coordinator() -> Coordinator {
    let storage = Storage::open_memory().expect("Failed to open in-memory storage");
    // Set max_concurrent_downloads to 0 to prevent auto-start in tests
    // (avoids network calls and filesystem side effects)
    let config = Config {
        max_concurrent_downloads: 0,
        ..Config::default()
    };
    Coordinator::new(config, storage)
}

#[tokio::test]
async fn test_add_download_creates_entry() {
    let coord = test_coordinator();
    let id = coord
        .add_download("https://example.com/file.zip", None)
        .await
        .unwrap();

    let row = coord.storage().get_download(&id).await.unwrap().unwrap();
    assert_eq!(row.url, "https://example.com/file.zip");
    assert_eq!(row.protocol, "http");
    assert_eq!(row.status, "queued");
}

#[tokio::test]
async fn test_owner_is_attributed_and_resolvable() {
    let coord = test_coordinator();

    // A download created with an owner resolves to that owner (from cache and
    // from a fresh storage lookup).
    let owned = coord
        .add_download_with_options(
            "https://example.com/owned.zip",
            None,
            None,
            0,
            Some("alice".to_string()),
        )
        .await
        .unwrap();
    assert_eq!(coord.download_owner(&owned).await.as_deref(), Some("alice"));
    assert_eq!(
        coord
            .storage()
            .get_download(&owned)
            .await
            .unwrap()
            .unwrap()
            .owner
            .as_deref(),
        Some("alice")
    );

    // A plain add_download is unowned.
    let unowned = coord
        .add_download("https://example.com/unowned.zip", None)
        .await
        .unwrap();
    assert_eq!(coord.download_owner(&unowned).await, None);
}

#[tokio::test]
async fn test_subject_download_id_classifies_events() {
    use krake_core::coordinator::DownloadEvent;
    // Download-scoped events expose their download id.
    assert_eq!(
        DownloadEvent::Added {
            id: "d1".into(),
            url: "u".into()
        }
        .subject_download_id(),
        Some("d1")
    );
    assert_eq!(
        DownloadEvent::CaptchaChallenge {
            id: "c1".into(),
            plugin_id: "p".into(),
            download_id: "d2".into(),
            image_url: "img".into(),
            captcha_type: "image".into(),
        }
        .subject_download_id(),
        Some("d2")
    );
    // Global events are not download-scoped.
    assert_eq!(DownloadEvent::QueueEmpty.subject_download_id(), None);
    assert_eq!(
        DownloadEvent::PluginNotification {
            plugin_id: "p".into(),
            title: "t".into(),
            message: "m".into(),
        }
        .subject_download_id(),
        None
    );
}

#[tokio::test]
async fn test_add_download_deduplication() {
    let coord = test_coordinator();
    let id1 = coord
        .add_download("https://example.com/file.zip", None)
        .await
        .unwrap();
    let id2 = coord
        .add_download("https://example.com/file.zip", None)
        .await
        .unwrap();

    // Same URL should return the same ID (duplicate detection)
    assert_eq!(id1, id2);
}

#[tokio::test]
async fn test_add_download_dedup_race() {
    // Concurrent add_download calls for the same URL must collapse to a single
    // row — the partial-unique index closes the TOCTOU window between the
    // pre-check and the insert (issue #53).
    let coord = test_coordinator();
    let url = "https://example.com/race.zip";
    let (a, b) = tokio::join!(coord.add_download(url, None), coord.add_download(url, None));
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(a, b, "concurrent add_download should return the same id");

    let rows = coord.storage().list_downloads().await.unwrap();
    let count = rows.iter().filter(|d| d.url == url).count();
    assert_eq!(count, 1, "exactly one row should exist for {url}");
}

#[tokio::test]
async fn test_add_download_with_filename() {
    let coord = test_coordinator();
    let id = coord
        .add_download("https://example.com/dl?id=123", Some("custom.zip".into()))
        .await
        .unwrap();

    let row = coord.storage().get_download(&id).await.unwrap().unwrap();
    assert_eq!(row.filename, Some("custom.zip".to_string()));
}

#[tokio::test]
async fn test_add_download_with_priority() {
    let coord = test_coordinator();
    let id = coord
        .add_download_with_options("https://example.com/movie.mp4", None, None, 5, None)
        .await
        .unwrap();

    let row = coord.storage().get_download(&id).await.unwrap().unwrap();
    assert_eq!(row.priority, 5);
}

#[tokio::test]
async fn test_protocol_detection_http() {
    let coord = test_coordinator();
    let id = coord
        .add_download("https://example.com/file.zip", None)
        .await
        .unwrap();
    let row = coord.storage().get_download(&id).await.unwrap().unwrap();
    assert_eq!(row.protocol, "http");
}

#[tokio::test]
async fn test_protocol_detection_hls() {
    let coord = test_coordinator();
    let id = coord
        .add_download("https://stream.example.com/live.m3u8", None)
        .await
        .unwrap();
    let row = coord.storage().get_download(&id).await.unwrap().unwrap();
    assert_eq!(row.protocol, "hls");
}

#[tokio::test]
async fn test_protocol_detection_dash() {
    let coord = test_coordinator();
    let id = coord
        .add_download("https://cdn.example.com/video.mpd", None)
        .await
        .unwrap();
    let row = coord.storage().get_download(&id).await.unwrap().unwrap();
    assert_eq!(row.protocol, "dash");
}

#[tokio::test]
async fn test_protocol_detection_usenet() {
    let coord = test_coordinator();
    let id = coord
        .add_download("https://nzbindex.com/file.nzb", None)
        .await
        .unwrap();
    let row = coord.storage().get_download(&id).await.unwrap().unwrap();
    assert_eq!(row.protocol, "usenet");
}

#[tokio::test]
async fn test_cancel_removes_download() {
    let coord = test_coordinator();
    let id = coord
        .add_download("https://example.com/file.zip", None)
        .await
        .unwrap();

    coord.cancel(&id).await.unwrap();
    let row = coord.storage().get_download(&id).await.unwrap();
    assert!(row.is_none());
}

#[tokio::test]
async fn test_pause_and_resume() {
    let coord = test_coordinator();
    let id = coord
        .add_download("https://example.com/file.zip", None)
        .await
        .unwrap();

    coord.pause(&id).await.unwrap();
    let row = coord.storage().get_download(&id).await.unwrap().unwrap();
    assert_eq!(row.status, "paused");

    coord.resume(&id).await.unwrap();
    let row = coord.storage().get_download(&id).await.unwrap().unwrap();
    // After resume, status goes back to queued (waiting for a slot)
    assert_eq!(row.status, "queued");
}

#[tokio::test]
async fn test_retry_clears_error_and_requeues() {
    let coord = test_coordinator();
    let id = coord
        .add_download("https://example.com/file.zip", None)
        .await
        .unwrap();

    coord
        .storage()
        .update_download_status(&id, QueueStatus::Failed)
        .await
        .unwrap();
    coord
        .storage()
        .update_download_error(&id, "connection reset", 0)
        .await
        .unwrap();

    coord.retry(&id).await.unwrap();

    let row = coord.storage().get_download(&id).await.unwrap().unwrap();
    assert_eq!(row.status, "queued");
    // The error must be cleared: the UI keys its "report this failure" button
    // off it, and the attempt is being made again.
    assert_eq!(row.error_message, None);
    assert_eq!(row.retry_count, 1);
}

#[tokio::test]
async fn test_retry_is_safe_on_a_download_that_is_not_failed() {
    // `retry` is reachable for any id through the API and the bulk endpoint,
    // so it must not corrupt state when applied to a queued or paused row.
    let coord = test_coordinator();
    let id = coord
        .add_download("https://example.com/file.zip", None)
        .await
        .unwrap();

    coord.pause(&id).await.unwrap();
    coord.retry(&id).await.unwrap();

    let row = coord.storage().get_download(&id).await.unwrap().unwrap();
    assert_eq!(row.status, "queued");
    assert_eq!(row.error_message, None);
    assert_eq!(coord.active_count().await, 0);
}

#[tokio::test]
async fn test_event_subscription() {
    let coord = test_coordinator();
    let mut rx = coord.subscribe();

    let _id = coord
        .add_download("https://example.com/file.zip", None)
        .await
        .unwrap();

    // Should receive an "Added" event
    let event = rx.recv().await.unwrap();
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["type"], "added");
}

#[tokio::test]
async fn test_recover_downloads_requeues_interrupted() {
    let coord = test_coordinator();
    let id = coord
        .add_download("https://example.com/file.zip", None)
        .await
        .unwrap();

    // Manually set status to "downloading" (simulating crash mid-download)
    coord
        .storage()
        .update_download_status(&id, QueueStatus::Downloading)
        .await
        .unwrap();

    let recovered = coord.recover_downloads().await.unwrap();
    assert_eq!(recovered, 1);

    let row = coord.storage().get_download(&id).await.unwrap().unwrap();
    assert_eq!(row.status, "queued");
}

#[tokio::test]
async fn test_chunk_storage_roundtrip() {
    let storage = Storage::open_memory().unwrap();

    // Create a download first
    let row = krake_core::storage::DownloadRow {
        id: "test-dl".to_string(),
        url: "https://example.com/big.zip".to_string(),
        protocol: "http".to_string(),
        filename: Some("big.zip".to_string()),
        filesize: Some(10_000_000),
        status: "queued".to_string(),
        priority: 0,
        package_id: None,
        plugin_id: None,
        download_dir: Some("downloads".to_string()),
        bytes_downloaded: 0,
        speed_current: 0,
        error_message: None,
        retry_count: 0,
        created_at: String::new(),
        started_at: None,
        completed_at: None,
        owner: None,
    };
    storage.insert_download(&row).await.unwrap();

    // Save chunks
    let chunks = krake_core::chunk::ChunkPlan::split(10_000_000, 4);
    let temp_dir = PathBuf::from("/tmp/test-chunks");
    storage
        .save_chunks("test-dl", &chunks.chunks, &temp_dir)
        .await
        .unwrap();

    // Load them back
    let loaded = storage.load_chunks("test-dl").await.unwrap();
    assert_eq!(loaded.len(), 4);
    assert_eq!(loaded[0].start_byte, 0);
    assert_eq!(loaded[3].end_byte, 9_999_999);

    // Update progress
    storage
        .update_chunk_progress("test-dl", 0, 2_500_000, "completed")
        .await
        .unwrap();

    let reloaded = storage.load_chunks("test-dl").await.unwrap();
    assert_eq!(reloaded[0].bytes_downloaded, 2_500_000);
    assert_eq!(
        reloaded[0].status,
        krake_core::chunk::ChunkStatus::Completed
    );

    // Delete
    storage.delete_chunks("test-dl").await.unwrap();
    let empty = storage.load_chunks("test-dl").await.unwrap();
    assert!(empty.is_empty());
}

#[tokio::test]
async fn test_config_validation() {
    let mut config = Config::default();
    assert!(config.validate().is_empty());

    config.max_concurrent_downloads = 0;
    let errors = config.validate();
    assert!(!errors.is_empty());
    assert!(errors[0].contains("max_concurrent_downloads"));
}

#[tokio::test]
async fn a_download_with_a_category_is_stored_with_it() {
    // The category is not a package: before, it went into `package_id`,
    // whose foreign key refused every category no package was named after.
    let coord = test_coordinator();
    let id = coord
        .add_download_with_options(
            "https://example.com/file.zip",
            None,
            Some("movies".into()),
            0,
            None,
        )
        .await
        .expect("a category must not need a package");

    let row = coord.storage().get_download(&id).await.unwrap().unwrap();
    assert_eq!(row.package_id, None);
    let dir = PathBuf::from(row.download_dir.unwrap());
    assert_eq!(dir, coord.storage().download_dir.join("movies"));
    let metadata: serde_json::Value = serde_json::from_str(
        &coord
            .storage()
            .get_download_metadata(&id)
            .await
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(metadata["category"], "movies");
}

const SMALL_NZB: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<nzb xmlns="http://www.newzbin.com/DTD/2003/nzb">
  <file poster="p" date="1" subject="&quot;a.mkv&quot; yEnc (1/1)">
    <groups><group>alt.binaries.test</group></groups>
    <segments><segment bytes="100" number="1">a1@test</segment></segments>
  </file>
</nzb>"#;

fn dupe(key: &str, score: i64, mode: DupeMode) -> NzbOptions {
    NzbOptions {
        dupe: Some(Dupe {
            key: key.into(),
            score,
            mode,
        }),
        ..NzbOptions::default()
    }
}

async fn add(coord: &Coordinator, name: &str, options: &NzbOptions) -> NzbAdded {
    coord
        .add_nzb_download_with(name, SMALL_NZB, options)
        .await
        .expect("add NZB")
}

#[tokio::test]
async fn an_nzb_is_queued_with_its_priority_and_paused() {
    let coord = test_coordinator();
    let added = add(
        &coord,
        "Paused.Release",
        &NzbOptions {
            priority: 100,
            paused: true,
            ..NzbOptions::default()
        },
    )
    .await;
    assert!(!added.dupe);
    let row = coord
        .storage()
        .get_download(&added.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.priority, 100);
    assert_eq!(row.status, "paused");
}

#[tokio::test]
async fn dupe_mode_score_skips_an_nzb_whose_key_is_queued_with_a_higher_or_equal_score() {
    let coord = test_coordinator();
    let first = add(&coord, "First", &dupe("show-s01e01", 100, DupeMode::Score)).await;
    assert!(!first.dupe);

    for score in [100, 50] {
        let second = add(
            &coord,
            "Second",
            &dupe("show-s01e01", score, DupeMode::Score),
        )
        .await;
        assert!(second.dupe, "score {score} must be a dupe");
        // Straight to the history, never queued.
        assert!(
            coord
                .storage()
                .get_download(&second.id)
                .await
                .unwrap()
                .is_none()
        );
        let history = coord.storage().get_history().await.unwrap();
        let entry = history
            .iter()
            .find(|h| h.id == second.id)
            .expect("in history");
        assert_eq!(entry.status, "failed");
        let metadata: serde_json::Value = serde_json::from_str(
            &coord
                .storage()
                .get_history_metadata(&second.id)
                .await
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(metadata["failure"], "dupe");
        assert_eq!(metadata["dupe_key"], "show-s01e01");
        assert_eq!(metadata["dupe_score"], score);
        assert!(metadata.get("nzb_data").is_none());
    }

    // A higher score, another key, no key, or FORCE is downloaded.
    for options in [
        dupe("show-s01e01", 101, DupeMode::Score),
        dupe("show-s01e02", 0, DupeMode::Score),
        dupe("", 0, DupeMode::Score),
        dupe("show-s01e01", 0, DupeMode::Force),
    ] {
        let added = add(&coord, "Other", &options).await;
        assert!(!added.dupe, "{options:?}");
        assert!(
            coord
                .storage()
                .get_download(&added.id)
                .await
                .unwrap()
                .is_some()
        );
    }
}

#[tokio::test]
async fn dupe_mode_score_checks_successful_downloads_but_not_failed_ones() {
    let coord = test_coordinator();
    let good = add(&coord, "Good", &dupe("movie-1", 10, DupeMode::Score)).await;
    let bad = add(&coord, "Bad", &dupe("movie-2", 10, DupeMode::Score)).await;
    for (id, status) in [
        (&good.id, QueueStatus::Completed),
        (&bad.id, QueueStatus::Failed),
    ] {
        coord
            .storage()
            .update_download_status(id, status)
            .await
            .unwrap();
        coord.storage().record_history(id).await.unwrap();
        coord.storage().delete_download(id).await.unwrap();
    }
    // Only the archived history is left of either.
    assert!(
        add(&coord, "Again", &dupe("movie-1", 10, DupeMode::Score))
            .await
            .dupe
    );
    assert!(
        !add(&coord, "Retry", &dupe("movie-2", 10, DupeMode::Score))
            .await
            .dupe
    );
}

#[tokio::test]
async fn dupe_mode_all_skips_only_what_was_downloaded_successfully() {
    let coord = test_coordinator();
    let queued = add(&coord, "Queued", &dupe("movie-1", 10, DupeMode::Score)).await;
    // Queued items do not count for ALL.
    assert!(
        !add(&coord, "All", &dupe("movie-1", 0, DupeMode::All))
            .await
            .dupe
    );

    coord
        .storage()
        .update_download_status(&queued.id, QueueStatus::Completed)
        .await
        .unwrap();
    // A successful download counts whatever the score.
    assert!(
        add(&coord, "All", &dupe("movie-1", 1000, DupeMode::All))
            .await
            .dupe
    );
}

#[test]
fn dupe_modes_parse_nzbget_names() {
    assert_eq!(DupeMode::parse("score"), DupeMode::Score);
    assert_eq!(DupeMode::parse("ALL"), DupeMode::All);
    assert_eq!(DupeMode::parse("Force"), DupeMode::Force);
    assert_eq!(DupeMode::parse(""), DupeMode::Score);
}
