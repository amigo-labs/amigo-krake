//! Packages a resolver returns: every entry queued, multi-file packages in a
//! folder, entry headers and cookies on every transfer request, one level of
//! re-resolution (`docs/specs/plugin-packages.md`, AC-01 to AC-08).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use krake_core::config::Config;
use krake_core::coordinator::{Coordinator, DownloadEvent};
use krake_core::protocol::{Protocol, ResolvedDownload, ResolvedPackage, UrlResolver};
use krake_core::storage::Storage;
use tokio::sync::broadcast::Receiver;
use tokio::time::timeout;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// A resolver backed by a fixed table: `packages` maps a URL to what it
/// resolves to (`None` = the plugin fails), `claims` decides which resolver
/// id claims a URL by prefix.
#[derive(Default)]
struct FakeResolver {
    packages: HashMap<String, Option<ResolvedPackage>>,
    claims: Vec<(String, String)>,
    calls: AtomicUsize,
    resolved: std::sync::Mutex<Vec<String>>,
}

#[async_trait::async_trait]
impl UrlResolver for FakeResolver {
    async fn resolve(&self, url: &str) -> Option<ResolvedPackage> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.resolved.lock().unwrap().push(url.to_string());
        self.packages.get(url).cloned().flatten()
    }

    async fn claimant(&self, url: &str) -> Option<String> {
        self.claims
            .iter()
            .find(|(prefix, _)| url.starts_with(prefix.as_str()))
            .map(|(_, id)| id.clone())
    }
}

fn entry(url: &str, filename: Option<&str>, size: Option<u64>) -> ResolvedDownload {
    ResolvedDownload {
        url: url.to_string(),
        filename: filename.map(str::to_string),
        filesize: size,
        protocol: Protocol::Http,
        headers: HashMap::new(),
    }
}

fn package(resolver: &str, name: &str, downloads: Vec<ResolvedDownload>) -> ResolvedPackage {
    ResolvedPackage {
        resolver: resolver.to_string(),
        name: name.to_string(),
        downloads,
    }
}

fn config_for(dir: &Path, max_concurrent: u32) -> Config {
    let mut config = Config {
        download_dir: dir.display().to_string(),
        max_concurrent_downloads: max_concurrent,
        ..Config::default()
    };
    config.retry.max_retries = 3;
    config.retry.base_delay_secs = 0.05;
    config.retry.max_delay_secs = 0.2;
    config
}

fn coordinator_with(
    resolver: FakeResolver,
    storage: Storage,
    config: Config,
) -> (Arc<FakeResolver>, Arc<Coordinator>) {
    let resolver = Arc::new(resolver);
    let mut coord = Coordinator::new(config, storage);
    coord.set_resolvers(vec![resolver.clone()]);
    let coord = Arc::new(coord);
    coord.spawn_queue_advance_loop();
    (resolver, coord)
}

fn memory_storage(dir: &Path) -> Storage {
    let mut storage = Storage::open_memory().expect("open in-memory storage");
    storage.download_dir = dir.to_path_buf();
    storage.temp_dir = dir.join(".tmp");
    std::fs::create_dir_all(&storage.temp_dir).unwrap();
    storage
}

async fn mount_file(mock: &MockServer, route: &str, body: &[u8]) {
    Mock::given(method("HEAD"))
        .and(path(route))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-length", body.len().to_string())
                .insert_header("accept-ranges", "none"),
        )
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path(route))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body.to_vec()))
        .mount(mock)
        .await;
}

async fn wait_terminal(rx: &mut Receiver<DownloadEvent>, ids: &[String]) -> HashMap<String, bool> {
    let mut done = HashMap::new();
    timeout(Duration::from_secs(20), async {
        while done.len() < ids.len() {
            match rx.recv().await {
                Ok(DownloadEvent::Completed { id }) if ids.contains(&id) => {
                    done.insert(id, true);
                }
                Ok(DownloadEvent::Failed { id, .. }) if ids.contains(&id) => {
                    done.insert(id, false);
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
    })
    .await
    .expect("downloads did not finish in time");
    done
}

async fn row_dir(coord: &Coordinator, id: &str) -> PathBuf {
    let row = coord.storage().get_download(id).await.unwrap().unwrap();
    PathBuf::from(row.download_dir.unwrap())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac01_ac02_every_entry_is_queued_in_order_inside_a_package_folder() {
    let mock = MockServer::start().await;
    for (route, body) in [
        ("/f1", b"one".as_slice()),
        ("/f2", b"two"),
        ("/f3", b"three"),
    ] {
        mount_file(&mock, route, body).await;
    }
    let base = mock.uri();
    let added = "https://album.test/a/1";
    let mut resolver = FakeResolver::default();
    resolver.packages.insert(
        added.into(),
        Some(package(
            "album",
            "My Album",
            vec![
                entry(&format!("{base}/f1"), Some("one.txt"), Some(3)),
                entry(&format!("{base}/f2"), Some("two.txt"), Some(3)),
                entry(&format!("{base}/f3"), Some("three.txt"), Some(5)),
            ],
        )),
    );
    let tmp = tempfile::tempdir().unwrap();
    let (_r, coord) = coordinator_with(
        resolver,
        memory_storage(tmp.path()),
        config_for(tmp.path(), 4),
    );
    let mut rx = coord.subscribe();

    let ids = coord
        .add_download_all(added, None, None, 0, None)
        .await
        .expect("add");
    assert_eq!(ids.len(), 3);

    let mut names = Vec::new();
    for id in &ids {
        let row = coord.storage().get_download(id).await.unwrap().unwrap();
        names.push((row.filename.clone().unwrap(), row.filesize));
        assert_eq!(row_dir(&coord, id).await, tmp.path().join("My Album"));
    }
    assert_eq!(
        names,
        vec![
            ("one.txt".to_string(), Some(3)),
            ("two.txt".to_string(), Some(3)),
            ("three.txt".to_string(), Some(5)),
        ]
    );

    let done = wait_terminal(&mut rx, &ids).await;
    assert!(done.values().all(|ok| *ok), "{done:?}");
    assert_eq!(
        std::fs::read(tmp.path().join("My Album").join("three.txt")).unwrap(),
        b"three"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac02_one_entry_gets_no_folder_and_unsafe_names_stay_inside() {
    let added_one = "https://album.test/f/1";
    let added_many = "https://album.test/a/evil";
    let mut resolver = FakeResolver::default();
    resolver.packages.insert(
        added_one.into(),
        Some(package(
            "album",
            "Single",
            vec![entry("https://cdn.test/1", Some("a.bin"), None)],
        )),
    );
    resolver.packages.insert(
        added_many.into(),
        Some(package(
            "album",
            "../../etc/pa:ss<>wd",
            vec![
                entry("https://cdn.test/2", Some("b.bin"), None),
                entry("https://cdn.test/3", Some("c.bin"), None),
            ],
        )),
    );
    let tmp = tempfile::tempdir().unwrap();
    // Nothing starts: these files are never fetched.
    let (_r, coord) = coordinator_with(
        resolver,
        memory_storage(tmp.path()),
        config_for(tmp.path(), 0),
    );

    let one = coord
        .add_download_all(added_one, None, None, 0, None)
        .await
        .unwrap();
    assert_eq!(row_dir(&coord, &one[0]).await, tmp.path());

    let many = coord
        .add_download_all(added_many, None, None, 0, None)
        .await
        .unwrap();
    let dir = row_dir(&coord, &many[0]).await;
    assert_eq!(dir.parent().unwrap(), tmp.path(), "folder escaped: {dir:?}");
    let folder = dir.file_name().unwrap().to_str().unwrap();
    assert!(!folder.contains(['/', '\\', ':', '<', '>']), "{folder}");
    assert!(!folder.starts_with('.'), "{folder}");

    // An empty name falls back to the resolver id.
    let mut resolver = FakeResolver::default();
    resolver.packages.insert(
        added_many.into(),
        Some(package(
            "album",
            "  ",
            vec![
                entry("https://cdn.test/4", None, None),
                entry("https://cdn.test/5", None, None),
            ],
        )),
    );
    let tmp2 = tempfile::tempdir().unwrap();
    let (_r, coord) = coordinator_with(
        resolver,
        memory_storage(tmp2.path()),
        config_for(tmp2.path(), 0),
    );
    let ids = coord
        .add_download_all(added_many, None, None, 0, None)
        .await
        .unwrap();
    assert_eq!(row_dir(&coord, &ids[0]).await, tmp2.path().join("album"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac02_category_folder_holds_the_package_folder() {
    let added = "https://album.test/a/2";
    let mut resolver = FakeResolver::default();
    resolver.packages.insert(
        added.into(),
        Some(package(
            "album",
            "Pics",
            vec![
                entry("https://cdn.test/6", None, None),
                entry("https://cdn.test/7", None, None),
            ],
        )),
    );
    let tmp = tempfile::tempdir().unwrap();
    let (_r, coord) = coordinator_with(
        resolver,
        memory_storage(tmp.path()),
        config_for(tmp.path(), 0),
    );
    let ids = coord
        .add_download_all(added, None, Some("photos".into()), 0, None)
        .await
        .unwrap();
    assert_eq!(
        row_dir(&coord, &ids[1]).await,
        tmp.path().join("photos").join("Pics")
    );
}

fn with_secrets(mut e: ResolvedDownload) -> ResolvedDownload {
    e.headers
        .insert("Referer".into(), "https://album.test/".into());
    e.headers
        .insert("Cookie".into(), "accountToken=SECRET-TOKEN-123".into());
    e
}

/// Mount `route` so it only answers when both entry headers arrive; the
/// first GET fails with 503 so the transfer needs a retry.
async fn mount_guarded_file(mock: &MockServer, route: &str, body: &[u8], fail_first: bool) {
    let guarded = |m: wiremock::MockBuilder| {
        m.and(path(route))
            .and(header("referer", "https://album.test/"))
            .and(header("cookie", "accountToken=SECRET-TOKEN-123"))
    };
    guarded(Mock::given(method("HEAD")))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-length", body.len().to_string())
                .insert_header("accept-ranges", "none"),
        )
        .mount(mock)
        .await;
    if fail_first {
        guarded(Mock::given(method("GET")))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(1)
            .with_priority(1)
            .mount(mock)
            .await;
    }
    guarded(Mock::given(method("GET")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body.to_vec()))
        .with_priority(2)
        .mount(mock)
        .await;
    // Without the headers the host refuses the file.
    Mock::given(path(route))
        .respond_with(ResponseTemplate::new(403))
        .with_priority(10)
        .mount(mock)
        .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac03_ac04_ac05_entry_headers_reach_every_attempt_and_never_the_history() {
    let mock = MockServer::start().await;
    mount_guarded_file(&mock, "/secret", b"payload", true).await;
    let added = "https://album.test/f/secret";
    let mut resolver = FakeResolver::default();
    resolver.packages.insert(
        added.into(),
        Some(package(
            "album",
            "x",
            vec![with_secrets(entry(
                &format!("{}/secret", mock.uri()),
                Some("s.bin"),
                None,
            ))],
        )),
    );
    let tmp = tempfile::tempdir().unwrap();
    let (_r, coord) = coordinator_with(
        resolver,
        memory_storage(tmp.path()),
        config_for(tmp.path(), 4),
    );
    let mut rx = coord.subscribe();
    let mut events = coord.subscribe();

    let ids = coord
        .add_download_all(added, None, None, 0, None)
        .await
        .unwrap();
    let done = wait_terminal(&mut rx, &ids).await;
    assert_eq!(
        done.get(&ids[0]),
        Some(&true),
        "transfer failed: headers missing?"
    );

    let gets = mock
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .filter(|r| r.method.as_str() == "GET")
        .count();
    assert!(gets >= 2, "expected a retry, saw {gets} GETs");

    // AC-05: not in the history entry, not in any event.
    let history = coord
        .storage()
        .get_history_metadata(&ids[0])
        .await
        .unwrap()
        .unwrap_or_default();
    assert!(!history.contains("SECRET-TOKEN-123"), "history: {history}");
    while let Ok(ev) = events.try_recv() {
        let json = serde_json::to_string(&ev).unwrap();
        assert!(!json.contains("SECRET-TOKEN-123"), "event: {json}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac03_headers_survive_a_restart() {
    let mock = MockServer::start().await;
    mount_guarded_file(&mock, "/later", b"after restart", false).await;
    let added = "https://album.test/f/later";
    let packages = || {
        let mut resolver = FakeResolver::default();
        resolver.packages.insert(
            added.into(),
            Some(package(
                "album",
                "x",
                vec![with_secrets(entry(
                    &format!("{}/later", mock.uri()),
                    Some("l.bin"),
                    None,
                ))],
            )),
        );
        resolver
    };
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("krake.db");
    let open = || {
        Storage::open(
            db.clone(),
            tmp.path().to_path_buf(),
            tmp.path().join(".tmp"),
        )
        .unwrap()
    };

    // First run: queued, nothing starts (no free slot).
    let id = {
        let (_r, coord) = coordinator_with(packages(), open(), config_for(tmp.path(), 0));
        coord
            .add_download_all(added, None, None, 0, None)
            .await
            .unwrap()[0]
            .clone()
    };

    // Second run, new coordinator on the same database, no resolver.
    let (_r, coord) = coordinator_with(FakeResolver::default(), open(), config_for(tmp.path(), 4));
    let mut rx = coord.subscribe();
    coord.resume(&id).await.unwrap();
    let done = wait_terminal(&mut rx, std::slice::from_ref(&id)).await;
    assert_eq!(
        done.get(&id),
        Some(&true),
        "headers lost across the restart"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac06_ac07_entries_the_same_plugin_claims_are_resolved_one_level() {
    let added = "https://album.test/a/root";
    let mut resolver = FakeResolver {
        claims: vec![
            ("https://album.test/".into(), "album".into()),
            // Every other URL is claimed by a generic plugin.
            ("https://".into(), "generic-http".into()),
        ],
        ..Default::default()
    };
    resolver.packages.insert(
        added.into(),
        Some(package(
            "album",
            "Root",
            vec![
                entry("https://album.test/f/1", None, None),
                entry("https://cdn.test/direct", Some("direct.bin"), None),
                entry("https://album.test/f/broken", None, None),
                entry("https://album.test/f/nested", None, None),
            ],
        )),
    );
    resolver.packages.insert(
        "https://album.test/f/1".into(),
        Some(package(
            "album",
            "f1",
            vec![entry("https://cdn.test/one", Some("one.bin"), None)],
        )),
    );
    resolver
        .packages
        .insert("https://album.test/f/broken".into(), None);
    resolver.packages.insert(
        "https://album.test/f/nested".into(),
        Some(package(
            "album",
            "nested",
            vec![
                entry("https://album.test/a/deeper", None, None),
                entry("https://cdn.test/two", Some("two.bin"), None),
            ],
        )),
    );
    let tmp = tempfile::tempdir().unwrap();
    let (r, coord) = coordinator_with(
        resolver,
        memory_storage(tmp.path()),
        config_for(tmp.path(), 0),
    );

    let ids = coord
        .add_download_all(added, None, None, 0, None)
        .await
        .unwrap();
    let mut urls = Vec::new();
    for id in &ids {
        urls.push(coord.storage().get_download(id).await.unwrap().unwrap().url);
    }
    assert_eq!(
        urls,
        vec![
            "https://cdn.test/one",
            "https://cdn.test/direct",
            "https://cdn.test/two",
        ]
    );
    // The direct link (claimed only by the generic plugin) was not resolved
    // again, nor was the deeper album.
    let resolved = r.resolved.lock().unwrap().clone();
    assert!(
        !resolved.iter().any(|u| u == "https://cdn.test/direct"),
        "{resolved:?}"
    );
    assert!(
        !resolved.iter().any(|u| u == "https://album.test/a/deeper"),
        "{resolved:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac08_an_empty_package_or_nothing_queueable_fails_the_add() {
    let empty = "https://album.test/a/empty";
    let all_broken = "https://album.test/a/broken";
    let mut resolver = FakeResolver {
        claims: vec![("https://album.test/".into(), "album".into())],
        ..Default::default()
    };
    resolver
        .packages
        .insert(empty.into(), Some(package("album", "Empty", vec![])));
    resolver.packages.insert(
        all_broken.into(),
        Some(package(
            "album",
            "Broken",
            vec![entry("https://album.test/f/x", None, None)],
        )),
    );
    resolver
        .packages
        .insert("https://album.test/f/x".into(), None);
    let tmp = tempfile::tempdir().unwrap();
    let (_r, coord) = coordinator_with(
        resolver,
        memory_storage(tmp.path()),
        config_for(tmp.path(), 0),
    );

    for url in [empty, all_broken] {
        let err = coord
            .add_download_all(url, None, None, 0, None)
            .await
            .expect_err("add must fail");
        assert!(err.to_string().contains("album"), "{err}");
    }
    assert!(coord.storage().list_downloads().await.unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac09_already_queued_entries_report_the_existing_id() {
    let first = "https://album.test/a/one";
    let second = "https://album.test/a/two";
    let mut resolver = FakeResolver::default();
    resolver.packages.insert(
        first.into(),
        Some(package(
            "album",
            "One",
            vec![entry("https://cdn.test/shared", None, None)],
        )),
    );
    resolver.packages.insert(
        second.into(),
        Some(package(
            "album",
            "Two",
            vec![
                entry("https://cdn.test/new", None, None),
                entry("https://cdn.test/shared", None, None),
                entry("https://cdn.test/new", None, None),
            ],
        )),
    );
    let tmp = tempfile::tempdir().unwrap();
    let (_r, coord) = coordinator_with(
        resolver,
        memory_storage(tmp.path()),
        config_for(tmp.path(), 0),
    );

    let a = coord
        .add_download_all(first, None, None, 0, None)
        .await
        .unwrap();
    let b = coord
        .add_download_all(second, None, None, 0, None)
        .await
        .unwrap();
    assert_eq!(b.len(), 3);
    assert_eq!(b[1], a[0], "shared entry must report the existing id");
    assert_eq!(b[0], b[2], "the same URL twice is queued once");
    assert_eq!(coord.storage().list_downloads().await.unwrap().len(), 2);
}
