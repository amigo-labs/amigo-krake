//! Usenet downloads against a fake NNTP server: a release that cannot be
//! completed must end as failed, with the reason kept on the download, while
//! a problem on our side must not be blamed on the release.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use krake_core::config::Config;
use krake_core::coordinator::{Coordinator, DownloadEvent};
use krake_core::storage::{Storage, UsenetServerRow};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::broadcast::Receiver;
use tokio::time::timeout;

/// How the fake server answers `BODY` for an article.
#[derive(Clone)]
enum Article {
    /// 222 with this yEnc body.
    Body(Vec<u8>),
    /// 430: the article is not on the server.
    Missing,
    /// 400: the server cannot serve right now.
    Unavailable,
}

/// A plain-TCP NNTP server answering from `articles` (keyed by message-id
/// without angle brackets). Returns its port.
async fn fake_nntp(articles: HashMap<String, Article>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let articles = Arc::new(articles);
    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            let articles = articles.clone();
            tokio::spawn(async move {
                let (read, mut write) = socket.into_split();
                let mut lines = BufReader::new(read).lines();
                if write.write_all(b"200 fake news\r\n").await.is_err() {
                    return;
                }
                while let Ok(Some(line)) = lines.next_line().await {
                    let reply: Vec<u8> = if let Some(group) = line.strip_prefix("GROUP ") {
                        format!("211 1 1 1 {group}\r\n").into_bytes()
                    } else if let Some(id) = line.strip_prefix("BODY ") {
                        let id = id.trim_start_matches('<').trim_end_matches('>');
                        match articles.get(id) {
                            Some(Article::Body(body)) => {
                                let mut out = format!("222 0 <{id}>\r\n").into_bytes();
                                out.extend_from_slice(body);
                                out.extend_from_slice(b"\r\n.\r\n");
                                out
                            }
                            Some(Article::Unavailable) => {
                                b"400 service temporarily unavailable\r\n".to_vec()
                            }
                            Some(Article::Missing) | None => b"430 no such article\r\n".to_vec(),
                        }
                    } else if line.starts_with("QUIT") {
                        let _ = write.write_all(b"205 bye\r\n").await;
                        return;
                    } else {
                        b"500 unknown command\r\n".to_vec()
                    };
                    if write.write_all(&reply).await.is_err() {
                        return;
                    }
                }
            });
        }
    });
    port
}

/// A single-part yEnc article holding `data`.
fn yenc(name: &str, data: &[u8]) -> Vec<u8> {
    let mut out = format!("=ybegin line=128 size={} name={name}\r\n", data.len()).into_bytes();
    for &b in data {
        let e = b.wrapping_add(42);
        if matches!(e, 0 | b'\n' | b'\r' | b'=' | b'.') {
            out.push(b'=');
            out.push(e.wrapping_add(64));
        } else {
            out.push(e);
        }
    }
    out.extend_from_slice(format!("\r\n=yend size={}", data.len()).as_bytes());
    out
}

/// An NZB with one file per `(name, message-ids)` pair.
fn nzb(files: &[(&str, &[&str])]) -> String {
    let mut out = String::from(
        r#"<?xml version="1.0" encoding="utf-8"?>
<nzb xmlns="http://www.newzbin.com/DTD/2003/nzb">"#,
    );
    for (name, ids) in files {
        out.push_str(&format!(
            r#"<file poster="p@example.com" date="1700000000" subject="&quot;{name}&quot; yEnc (1/{})"><groups><group>alt.binaries.test</group></groups><segments>"#,
            ids.len()
        ));
        for (i, id) in ids.iter().enumerate() {
            out.push_str(&format!(
                r#"<segment bytes="100" number="{}">{id}</segment>"#,
                i + 1
            ));
        }
        out.push_str("</segments></file>");
    }
    out.push_str("</nzb>");
    out
}

/// A coordinator over a temp dir whose only Usenet server is `port`.
async fn usenet_coordinator(port: u16) -> (tempfile::TempDir, Arc<Coordinator>) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut storage = Storage::open_memory().expect("storage");
    storage.download_dir = tmp.path().join("downloads");
    storage.temp_dir = tmp.path().join("tmp");
    std::fs::create_dir_all(&storage.download_dir).unwrap();
    std::fs::create_dir_all(&storage.temp_dir).unwrap();
    storage
        .insert_usenet_server(&UsenetServerRow {
            id: "fake".into(),
            name: "fake".into(),
            host: "127.0.0.1".into(),
            port,
            ssl: false,
            username: String::new(),
            password: String::new(),
            connections: 2,
            priority: 0,
            created_at: String::new(),
        })
        .await
        .unwrap();
    let mut config = Config {
        download_dir: tmp.path().join("downloads").display().to_string(),
        max_concurrent_downloads: 2,
        ..Config::default()
    };
    config.retry.max_retries = 1;
    config.retry.base_delay_secs = 0.05;
    config.retry.max_delay_secs = 0.1;
    let coord = Arc::new(Coordinator::new(config, storage));
    coord.spawn_queue_advance_loop();
    (tmp, coord)
}

/// `Some(true)` on completion, `Some(false)` on failure.
async fn wait_for_terminal(mut rx: Receiver<DownloadEvent>, id: &str) -> Option<bool> {
    timeout(Duration::from_secs(20), async move {
        loop {
            match rx.recv().await {
                Ok(DownloadEvent::Completed { id: ev }) if ev == id => return Some(true),
                Ok(DownloadEvent::Failed { id: ev, .. }) if ev == id => return Some(false),
                Ok(_) => continue,
                Err(_) => return None,
            }
        }
    })
    .await
    .ok()
    .flatten()
}

async fn metadata(coord: &Coordinator, id: &str) -> Value {
    let raw = coord
        .storage()
        .get_download_metadata(id)
        .await
        .unwrap()
        .expect("metadata");
    serde_json::from_str(&raw).unwrap()
}

async fn status(coord: &Coordinator, id: &str) -> String {
    coord
        .storage()
        .get_download(id)
        .await
        .unwrap()
        .expect("row")
        .status
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_articles_without_repair_data_fail_the_download_for_health() {
    let port = fake_nntp(HashMap::from([
        (
            "a1@test".to_string(),
            Article::Body(yenc("movie.mkv", b"first half of the movie")),
        ),
        ("a2@test".to_string(), Article::Missing),
    ]))
    .await;
    let (_tmp, coord) = usenet_coordinator(port).await;
    let rx = coord.subscribe();

    let id = coord
        .add_nzb_download(
            "Broken.Release",
            &nzb(&[("movie.mkv", &["a1@test", "a2@test"])]),
            Some("movies"),
        )
        .await
        .unwrap();

    assert_eq!(wait_for_terminal(rx, &id).await, Some(false));
    assert_eq!(status(&coord, &id).await, "failed");
    let meta = metadata(&coord, &id).await;
    assert_eq!(meta["failure"], "health");
    assert_eq!(meta["health"], 500);
    assert_eq!(meta["critical_health"], 1000);
    assert_eq!(meta["total_articles"], 2);
    assert_eq!(meta["failed_articles"], 1);
    // What append stored stays.
    assert_eq!(meta["category"], "movies");
    assert!(meta["nzb_data"].is_string());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_broken_archive_fails_the_download_for_unpack() {
    let port = fake_nntp(HashMap::from([(
        "r1@test".to_string(),
        Article::Body(yenc("release.rar", b"this is not a rar archive at all")),
    )]))
    .await;
    let (_tmp, coord) = usenet_coordinator(port).await;
    let rx = coord.subscribe();

    let id = coord
        .add_nzb_download(
            "Broken.Archive",
            &nzb(&[("release.rar", &["r1@test"])]),
            None,
        )
        .await
        .unwrap();

    assert_eq!(wait_for_terminal(rx, &id).await, Some(false));
    assert_eq!(status(&coord, &id).await, "failed");
    let meta = metadata(&coord, &id).await;
    assert_eq!(meta["failure"], "unpack");
    assert_eq!(meta["unpack_status"], "FAILURE");
    assert_eq!(meta["health"], 1000);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unavailable_server_is_not_blamed_on_the_release() {
    let port = fake_nntp(HashMap::from([
        (
            "a1@test".to_string(),
            Article::Body(yenc("movie.mkv", b"first half")),
        ),
        ("a2@test".to_string(), Article::Unavailable),
    ]))
    .await;
    let (_tmp, coord) = usenet_coordinator(port).await;
    let rx = coord.subscribe();

    let id = coord
        .add_nzb_download(
            "Fine.Release",
            &nzb(&[("movie.mkv", &["a1@test", "a2@test"])]),
            None,
        )
        .await
        .unwrap();

    assert_eq!(wait_for_terminal(rx, &id).await, Some(false));
    assert_eq!(status(&coord, &id).await, "failed");
    let meta = metadata(&coord, &id).await;
    assert!(
        meta.get("failure").is_none_or(Value::is_null),
        "a server problem is not a broken release: {meta}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_complete_release_succeeds_with_full_health() {
    let port = fake_nntp(HashMap::from([
        (
            "a1@test".to_string(),
            Article::Body(yenc("movie.mkv", b"first half,")),
        ),
        (
            "a2@test".to_string(),
            Article::Body(yenc("movie.mkv", b" second half")),
        ),
    ]))
    .await;
    let (tmp, coord) = usenet_coordinator(port).await;
    let rx = coord.subscribe();

    let id = coord
        .add_nzb_download(
            "Good.Release",
            &nzb(&[("movie.mkv", &["a1@test", "a2@test"])]),
            None,
        )
        .await
        .unwrap();

    assert_eq!(wait_for_terminal(rx, &id).await, Some(true));
    let file = tmp.path().join("downloads/Good.Release/movie.mkv");
    assert_eq!(std::fs::read(file).unwrap(), b"first half, second half");
    let meta = metadata(&coord, &id).await;
    assert!(meta["failure"].is_null());
    assert_eq!(meta["health"], 1000);
    assert_eq!(meta["par_status"], "NONE");
    assert_eq!(meta["unpack_status"], "NONE");
}
