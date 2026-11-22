//! Integration tests for the REST API endpoints.
//!
//! Tests exercise the Axum router through actual HTTP requests using
//! a real in-memory SQLite backend. Each test gets a fresh server instance.

use std::net::SocketAddr;

use krake_core::config::Config;
use std::assert_matches;

/// Spawn a test server on a random port. Returns the bound address.
async fn spawn_test_server() -> SocketAddr {
    spawn_test_server_with(Config::default()).await
}

/// Spawn a test server with `config`; downloads never auto-start.
async fn spawn_test_server_with(config: Config) -> SocketAddr {
    // Prevent auto-start of downloads during tests
    let config = Config {
        max_concurrent_downloads: 0,
        ..config
    };

    let state = krake_server::build_test_state(config);
    let app = krake_server::build_test_router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind test server");
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    addr
}

/// Build a test HTTP client.
fn test_client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("Failed to build test client")
}

fn base_url(addr: SocketAddr) -> String {
    format!("http://{addr}")
}

// =============================================================================
// Status & Stats
// =============================================================================

#[tokio::test]
async fn test_status_returns_ok() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .get(format!("{}/api/v1/status", base_url(addr)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
    assert!(body["version"].is_string());
}

#[tokio::test]
async fn test_stats_returns_zeroes_initially() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .get(format!("{}/api/v1/stats", base_url(addr)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["active_downloads"], 0);
    assert_eq!(body["speed_bytes_per_sec"], 0);
    assert_eq!(body["queued"], 0);
    assert_eq!(body["completed"], 0);
}

// =============================================================================
// Downloads CRUD
// =============================================================================

#[tokio::test]
async fn test_add_download_returns_created() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .post(format!("{}/api/v1/downloads", base_url(addr)))
        .json(&serde_json::json!({
            "url": "https://example.com/file.zip"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["id"].is_string());
    assert!(!body["id"].as_str().unwrap().is_empty());
}

/// Resolves one fixed album URL to three files.
struct AlbumResolver;

#[async_trait::async_trait]
impl krake_core::protocol::UrlResolver for AlbumResolver {
    async fn resolve(&self, url: &str) -> Option<krake_core::protocol::ResolvedPackage> {
        (url == "https://example.com/album/1").then(|| krake_core::protocol::ResolvedPackage {
            resolver: "album".into(),
            name: "Album".into(),
            downloads: ["a", "b", "c"]
                .iter()
                .map(|n| krake_core::protocol::ResolvedDownload {
                    url: format!("https://cdn.example/{n}"),
                    filename: Some(format!("{n}.bin")),
                    filesize: None,
                    protocol: krake_core::protocol::Protocol::Http,
                    headers: Default::default(),
                })
                .collect(),
        })
    }
}

#[tokio::test]
async fn ac09_add_answers_every_id_of_a_package_in_order() {
    let config = Config {
        max_concurrent_downloads: 0,
        ..Config::default()
    };
    let state = krake_server::build_test_state_with_resolvers(
        config,
        vec![std::sync::Arc::new(AlbumResolver)],
    );
    let coordinator = state.coordinator.clone();
    let app = krake_server::build_test_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let resp = test_client()
        .post(format!("{}/api/v1/downloads", base_url(addr)))
        .json(&serde_json::json!({ "url": "https://example.com/album/1" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = resp.json().await.unwrap();
    let ids: Vec<String> = serde_json::from_value(body["ids"].clone()).unwrap();
    assert_eq!(ids.len(), 3);
    assert_eq!(body["id"], ids[0]);
    let mut files = Vec::new();
    for id in &ids {
        let row = coordinator
            .storage()
            .get_download(id)
            .await
            .unwrap()
            .unwrap();
        files.push(row.filename.unwrap());
    }
    assert_eq!(files, ["a.bin", "b.bin", "c.bin"]);

    // A plain URL still answers one id in both fields.
    let resp = test_client()
        .post(format!("{}/api/v1/downloads", base_url(addr)))
        .json(&serde_json::json!({ "url": "https://example.com/file.zip" }))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["ids"], serde_json::json!([body["id"].clone()]));
}

/// Resolves one URL to a file that needs a secret cookie and header.
struct SecretResolver;

#[async_trait::async_trait]
impl krake_core::protocol::UrlResolver for SecretResolver {
    async fn resolve(&self, url: &str) -> Option<krake_core::protocol::ResolvedPackage> {
        (url == "https://example.com/secret/1").then(|| krake_core::protocol::ResolvedPackage {
            resolver: "secret".into(),
            name: "Secret".into(),
            downloads: vec![krake_core::protocol::ResolvedDownload {
                url: "https://cdn.example/secret.bin".into(),
                filename: Some("secret.bin".into()),
                filesize: None,
                protocol: krake_core::protocol::Protocol::Http,
                headers: [
                    (
                        "Cookie".to_string(),
                        "accountToken=TOP-SECRET-COOKIE".to_string(),
                    ),
                    (
                        "Authorization".to_string(),
                        "Bearer TOP-SECRET-HEADER".to_string(),
                    ),
                ]
                .into_iter()
                .collect(),
            }],
        })
    }
}

/// `docs/specs/plugin-packages.md` AC-05: entry headers and cookies never
/// show in the REST responses for downloads or history.
#[tokio::test]
async fn ac05_entry_headers_never_show_in_download_or_history_responses() {
    let config = Config {
        max_concurrent_downloads: 0,
        ..Config::default()
    };
    let state = krake_server::build_test_state_with_resolvers(
        config,
        vec![std::sync::Arc::new(SecretResolver)],
    );
    let coordinator = state.coordinator.clone();
    let app = krake_server::build_test_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let base = base_url(addr);

    let body: serde_json::Value = test_client()
        .post(format!("{base}/api/v1/downloads"))
        .json(&serde_json::json!({ "url": "https://example.com/secret/1" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = body["id"].as_str().unwrap().to_string();

    // The secrets are stored for the transfer (so the check below means
    // something) ...
    let stored = coordinator
        .storage()
        .get_download_metadata(&id)
        .await
        .unwrap()
        .unwrap();
    assert!(stored.contains("TOP-SECRET-COOKIE"), "{stored}");

    // ... but no response shows them.
    let mut answers = Vec::new();
    for path in [
        "/api/v1/downloads".to_string(),
        format!("/api/v1/downloads/{id}"),
    ] {
        answers.push(
            test_client()
                .get(format!("{base}{path}"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap(),
        );
    }
    coordinator.storage().record_history(&id).await.unwrap();
    let history = test_client()
        .get(format!("{base}/api/v1/history"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        history.contains("secret.bin"),
        "history lists the download: {history}"
    );
    answers.push(history);
    for answer in answers {
        assert!(!answer.contains("TOP-SECRET"), "secret leaked: {answer}");
    }
}

#[tokio::test]
async fn test_add_download_rejects_ssrf_targets() {
    // The main REST download path must apply the same SSRF guard as
    // Click'n'Load / RSS / webhooks: loopback, RFC1918, and cloud-metadata
    // targets are rejected with 400 rather than fetched off the network.
    let addr = spawn_test_server().await;
    let client = test_client();

    for url in [
        "http://127.0.0.1/secret",
        "http://169.254.169.254/latest/meta-data/",
        "http://10.0.0.1/x",
        "file:///etc/passwd",
    ] {
        let resp = client
            .post(format!("{}/api/v1/downloads", base_url(addr)))
            .json(&serde_json::json!({ "url": url }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 400, "url {url} must be rejected");
    }
}

#[tokio::test]
async fn test_add_download_with_filename() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .post(format!("{}/api/v1/downloads", base_url(addr)))
        .json(&serde_json::json!({
            "url": "https://example.com/dl?id=123",
            "filename": "custom.zip"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = resp.json().await.unwrap();
    let id = body["id"].as_str().unwrap();

    // Verify the filename was set
    let resp = client
        .get(format!("{}/api/v1/downloads/{}", base_url(addr), id))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let dl: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(dl["filename"], "custom.zip");
}

#[tokio::test]
async fn test_list_downloads_empty() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .get(format!("{}/api/v1/downloads", base_url(addr)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_list_downloads_after_add() {
    let addr = spawn_test_server().await;
    let client = test_client();
    let base = base_url(addr);

    // Add a download
    client
        .post(format!("{base}/api/v1/downloads"))
        .json(&serde_json::json!({"url": "https://example.com/file1.zip"}))
        .send()
        .await
        .unwrap();

    let resp = client
        .get(format!("{base}/api/v1/downloads"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["url"], "https://example.com/file1.zip");
    assert_eq!(arr[0]["status"], "queued");
}

#[tokio::test]
async fn test_get_download_by_id() {
    let addr = spawn_test_server().await;
    let client = test_client();
    let base = base_url(addr);

    let resp = client
        .post(format!("{base}/api/v1/downloads"))
        .json(&serde_json::json!({"url": "https://example.com/file.zip"}))
        .send()
        .await
        .unwrap();
    let id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = client
        .get(format!("{base}/api/v1/downloads/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["id"], id);
    assert_eq!(body["url"], "https://example.com/file.zip");
    assert_eq!(body["protocol"], "http");
    assert_eq!(body["status"], "queued");
}

#[tokio::test]
async fn test_get_nonexistent_download_returns_404() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .get(format!(
            "{}/api/v1/downloads/nonexistent-id",
            base_url(addr)
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_patch_download_pause_and_resume() {
    let addr = spawn_test_server().await;
    let client = test_client();
    let base = base_url(addr);

    let resp = client
        .post(format!("{base}/api/v1/downloads"))
        .json(&serde_json::json!({"url": "https://example.com/file.zip"}))
        .send()
        .await
        .unwrap();
    let id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Pause
    let resp = client
        .patch(format!("{base}/api/v1/downloads/{id}"))
        .json(&serde_json::json!({"action": "pause"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Verify paused
    let resp = client
        .get(format!("{base}/api/v1/downloads/{id}"))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "paused");

    // Resume
    let resp = client
        .patch(format!("{base}/api/v1/downloads/{id}"))
        .json(&serde_json::json!({"action": "resume"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Verify resumed -> queued
    let resp = client
        .get(format!("{base}/api/v1/downloads/{id}"))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "queued");
}

#[tokio::test]
async fn test_patch_download_invalid_action() {
    let addr = spawn_test_server().await;
    let client = test_client();
    let base = base_url(addr);

    let resp = client
        .post(format!("{base}/api/v1/downloads"))
        .json(&serde_json::json!({"url": "https://example.com/file.zip"}))
        .send()
        .await
        .unwrap();
    let id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = client
        .patch(format!("{base}/api/v1/downloads/{id}"))
        .json(&serde_json::json!({"action": "invalid"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_delete_download() {
    let addr = spawn_test_server().await;
    let client = test_client();
    let base = base_url(addr);

    let resp = client
        .post(format!("{base}/api/v1/downloads"))
        .json(&serde_json::json!({"url": "https://example.com/file.zip"}))
        .send()
        .await
        .unwrap();
    let id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Delete
    let resp = client
        .delete(format!("{base}/api/v1/downloads/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // Verify gone
    let resp = client
        .get(format!("{base}/api/v1/downloads/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

// =============================================================================
// Batch
// =============================================================================

#[tokio::test]
async fn test_add_batch_downloads() {
    let addr = spawn_test_server().await;
    let client = test_client();
    let base = base_url(addr);

    let resp = client
        .post(format!("{base}/api/v1/downloads/batch"))
        .json(&serde_json::json!({
            "urls": [
                "https://example.com/file1.zip",
                "https://example.com/file2.zip",
                "https://example.com/file3.zip"
            ]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = resp.json().await.unwrap();
    let ids = body["ids"].as_array().unwrap();
    assert_eq!(ids.len(), 3);
    assert!(body["errors"].as_array().unwrap().is_empty());

    // Verify all appear in list
    let resp = client
        .get(format!("{base}/api/v1/downloads"))
        .send()
        .await
        .unwrap();
    let list: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(list.as_array().unwrap().len(), 3);
}

// =============================================================================
// Queue
// =============================================================================

#[tokio::test]
async fn test_queue_empty_initially() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .get(format!("{}/api/v1/queue", base_url(addr)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_queue_shows_queued_downloads() {
    let addr = spawn_test_server().await;
    let client = test_client();
    let base = base_url(addr);

    client
        .post(format!("{base}/api/v1/downloads"))
        .json(&serde_json::json!({"url": "https://example.com/file.zip"}))
        .send()
        .await
        .unwrap();

    let resp = client
        .get(format!("{base}/api/v1/queue"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_queue_reorder() {
    let addr = spawn_test_server().await;
    let client = test_client();
    let base = base_url(addr);

    // Add 3 downloads
    let mut ids = Vec::new();
    for i in 1..=3 {
        let resp = client
            .post(format!("{base}/api/v1/downloads"))
            .json(&serde_json::json!({"url": format!("https://example.com/file{i}.zip")}))
            .send()
            .await
            .unwrap();
        let body: serde_json::Value = resp.json().await.unwrap();
        ids.push(body["id"].as_str().unwrap().to_string());
    }

    // Reorder: reverse
    let reordered = vec![ids[2].clone(), ids[1].clone(), ids[0].clone()];
    let resp = client
        .patch(format!("{base}/api/v1/queue/reorder"))
        .json(&serde_json::json!({"ids": reordered}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Verify priorities changed
    let resp = client
        .get(format!("{base}/api/v1/downloads/{}", ids[2]))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    let prio_first = body["priority"].as_i64().unwrap();

    let resp = client
        .get(format!("{base}/api/v1/downloads/{}", ids[0]))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    let prio_last = body["priority"].as_i64().unwrap();

    // First in reorder list should have higher priority
    assert!(prio_first > prio_last);
}

// =============================================================================
// History
// =============================================================================

#[tokio::test]
async fn test_history_empty_initially() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .get(format!("{}/api/v1/history", base_url(addr)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_delete_history() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .delete(format!("{}/api/v1/history", base_url(addr)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 204);
}

// =============================================================================
// Config
// =============================================================================

#[tokio::test]
async fn test_get_config() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .get(format!("{}/api/v1/config", base_url(addr)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["download_dir"].is_string());
    assert!(body["max_concurrent_downloads"].is_number());
    assert!(body["http"].is_object());
}

#[tokio::test]
async fn test_put_config_updates_values() {
    let addr = spawn_test_server().await;
    let client = test_client();
    let base = base_url(addr);

    // Get current config
    let resp = client
        .get(format!("{base}/api/v1/config"))
        .send()
        .await
        .unwrap();
    let mut config: serde_json::Value = resp.json().await.unwrap();

    // Modify a value
    config["max_concurrent_downloads"] = serde_json::json!(5);

    let resp = client
        .put(format!("{base}/api/v1/config"))
        .json(&config)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Verify the change persisted
    let resp = client
        .get(format!("{base}/api/v1/config"))
        .send()
        .await
        .unwrap();
    let updated: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(updated["max_concurrent_downloads"], 5);
}

// =============================================================================
// Plugins
// =============================================================================

#[tokio::test]
async fn test_list_plugins() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .get(format!("{}/api/v1/plugins", base_url(addr)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.is_array());
}

// =============================================================================
// Captcha
// =============================================================================

#[tokio::test]
async fn test_captcha_pending_empty() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .get(format!("{}/api/v1/captcha/pending", base_url(addr)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.as_array().unwrap().is_empty());
}

// =============================================================================
// Webhooks
// =============================================================================

#[tokio::test]
async fn test_webhook_crud_lifecycle() {
    let addr = spawn_test_server().await;
    let client = test_client();
    let base = base_url(addr);

    // List — empty
    let resp = client
        .get(format!("{base}/api/v1/webhooks"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.as_array().unwrap().is_empty());

    // Create
    let resp = client
        .post(format!("{base}/api/v1/webhooks"))
        .json(&serde_json::json!({
            "name": "test-hook",
            "url": "https://example.com/webhook"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let hook: serde_json::Value = resp.json().await.unwrap();
    let hook_id = hook["id"].as_str().unwrap().to_string();

    // List — should have one
    let resp = client
        .get(format!("{base}/api/v1/webhooks"))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body.as_array().unwrap().len(), 1);

    // Delete
    let resp = client
        .delete(format!("{base}/api/v1/webhooks/{hook_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // List — empty again
    let resp = client
        .get(format!("{base}/api/v1/webhooks"))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_webhook_without_secret_gets_generated_signing_secret() {
    // Signing is mandatory: a webhook created without a secret must still get
    // a random signing secret. The plaintext is returned once in the create
    // response, then redacted from the listing.
    let addr = spawn_test_server().await;
    let client = test_client();
    let base = base_url(addr);

    let resp = client
        .post(format!("{base}/api/v1/webhooks"))
        .json(&serde_json::json!({
            "name": "unsigned-hook",
            "url": "https://example.com/webhook"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let hook: serde_json::Value = resp.json().await.unwrap();

    // Create response returns the real secret once: 64 hex chars.
    let secret = hook["secret"]
        .as_str()
        .expect("secret present in create response");
    assert_eq!(secret.len(), 64, "generated secret must be 32 bytes hex");
    assert!(secret.bytes().all(|b| b.is_ascii_hexdigit()));

    // Listing must NOT leak the plaintext secret.
    let resp = client
        .get(format!("{base}/api/v1/webhooks"))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    let listed_secret = body[0]["secret"].as_str().unwrap_or_default();
    assert_ne!(
        listed_secret, secret,
        "listing must redact the plaintext signing secret"
    );
}

#[tokio::test]
async fn test_delete_nonexistent_webhook_returns_404() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .delete(format!("{}/api/v1/webhooks/nonexistent", base_url(addr)))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_webhook_creation_syncs_to_config() {
    // Regression: create/delete used to touch only the in-memory dispatcher,
    // so hooks never reached config.webhooks (vanished on restart, and
    // GET /config disagreed with GET /webhooks). Use a valid config so the
    // save path runs, then assert the created hook appears in GET /config.
    let config = Config {
        max_concurrent_downloads: 1,
        ..Config::default()
    };
    let state = krake_server::build_test_state(config);
    let app = krake_server::build_test_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = test_client();
    let base = base_url(addr);

    let resp = client
        .post(format!("{base}/api/v1/webhooks"))
        .json(&serde_json::json!({ "name": "h", "url": "https://example.com/wh" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    let cfg: serde_json::Value = client
        .get(format!("{base}/api/v1/config"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let hooks = cfg["webhooks"].as_array().expect("webhooks array");
    assert_eq!(
        hooks.len(),
        1,
        "created webhook must be persisted to config"
    );
    assert_eq!(hooks[0]["url"], "https://example.com/wh");
}

// =============================================================================
// RSS (feature-gated, disabled by default)
// =============================================================================

#[tokio::test]
async fn test_rss_feeds_empty_when_disabled() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .get(format!("{}/api/v1/rss", base_url(addr)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.as_array().unwrap().is_empty());
}

// =============================================================================
// Usenet (feature-gated, disabled by default)
// =============================================================================

#[tokio::test]
async fn test_usenet_servers_empty_when_disabled() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .get(format!("{}/api/v1/usenet/servers", base_url(addr)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.as_array().unwrap().is_empty());
}

// =============================================================================
// Response structure validation
// =============================================================================

#[tokio::test]
async fn test_download_response_has_expected_fields() {
    let addr = spawn_test_server().await;
    let client = test_client();
    let base = base_url(addr);

    let resp = client
        .post(format!("{base}/api/v1/downloads"))
        .json(&serde_json::json!({"url": "https://example.com/file.zip"}))
        .send()
        .await
        .unwrap();
    let id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = client
        .get(format!("{base}/api/v1/downloads/{id}"))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();

    // Verify all expected fields are present
    assert!(body.get("id").is_some());
    assert!(body.get("url").is_some());
    assert!(body.get("protocol").is_some());
    assert!(body.get("status").is_some());
    assert!(body.get("priority").is_some());
    assert!(body.get("bytes_downloaded").is_some());
    assert!(body.get("speed").is_some());
    assert!(body.get("created_at").is_some());
}

// =============================================================================
// Error handling — malformed requests
// =============================================================================

#[tokio::test]
async fn test_add_download_without_url_returns_error() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .post(format!("{}/api/v1/downloads", base_url(addr)))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();

    // Should be 4xx (likely 422 from Axum deserialization failure)
    assert!(resp.status().is_client_error());
}

#[tokio::test]
async fn test_add_download_with_invalid_json_returns_error() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .post(format!("{}/api/v1/downloads", base_url(addr)))
        .header("content-type", "application/json")
        .body("not valid json")
        .send()
        .await
        .unwrap();

    assert!(resp.status().is_client_error());
}

// =============================================================================
// Body-size limits (audit finding #3 — DoS via unbounded uploads)
// =============================================================================

#[tokio::test]
async fn dlc_upload_rejects_oversize_body() {
    let addr = spawn_test_server().await;
    let client = test_client();

    // 2 MiB synthetic payload — over the 1 MiB DLC limit. We hand-roll the
    // multipart body so the test depends only on the JSON-enabled client
    // used elsewhere. The body-size guard triggers from DefaultBodyLimit
    // before the multipart parser even runs, so the exact framing does not
    // matter as long as the wire size exceeds the limit.
    let payload = vec![b'A'; 2 * 1024 * 1024];
    let boundary = "----kraketest";
    let mut body: Vec<u8> = Vec::with_capacity(payload.len() + 256);
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"evil.dlc\"\r\n",
    );
    body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
    body.extend_from_slice(&payload);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let resp = client
        .post(format!("{}/api/v1/downloads/container", base_url(addr)))
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(body)
        .send()
        .await
        .unwrap();

    // The body-size guard rejects the request before the handler ever
    // sees it. The exact status varies (axum's DefaultBodyLimit returns
    // 413 for raw bodies, the Multipart extractor surfaces it as 400),
    // but it must not be a 2xx — the must-not is that an oversize body
    // gets buffered into the handler.
    let status = resp.status();
    assert!(
        status.is_client_error(),
        "expected 4xx for oversize DLC, got {status}"
    );
    assert_matches!(
        status.as_u16(),
        400 | 413,
        "expected 400 or 413 for oversize DLC, got {status}"
    );
}

#[tokio::test]
async fn nzb_upload_rejects_oversize_body() {
    let addr = spawn_test_server().await;
    let client = test_client();

    // 65 MiB JSON body — over the 64 MiB NZB limit. We stuff the bytes into
    // the nzb_data field as a long string.
    let big_string = "X".repeat(65 * 1024 * 1024);
    let body = serde_json::json!({ "nzb_data": big_string });

    let resp = client
        .post(format!("{}/api/v1/downloads/nzb", base_url(addr)))
        .json(&body)
        .send()
        .await
        .unwrap();

    // Either 413 Payload Too Large from DefaultBodyLimit, or a 4xx if the
    // server rejects the connection earlier — anything in the client-error
    // range is acceptable; the must-not is "200 OK".
    assert!(
        resp.status().is_client_error(),
        "expected 4xx, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn nzb_upload_queues_each_nzb_as_its_own_usenet_download() {
    let mut config = Config::default();
    config.features.usenet = true;
    let addr = spawn_test_server_with(config).await;
    let client = test_client();

    let mut ids = Vec::new();
    for article in ["first", "second"] {
        let nzb = format!(
            r#"<nzb><file poster="p" date="1" subject="&quot;{article}.rar&quot; yEnc (1/1)"><groups><group>alt.binaries.test</group></groups><segments><segment bytes="100" number="1">{article}@example.com</segment></segments></file></nzb>"#
        );
        let resp = client
            .post(format!("{}/api/v1/downloads/nzb", base_url(addr)))
            .json(&serde_json::json!({ "nzb_data": nzb }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 201);
        let body: serde_json::Value = resp.json().await.unwrap();
        let id = body["id"].as_str().unwrap().to_string();

        let resp = client
            .get(format!("{}/api/v1/downloads/{id}", base_url(addr)))
            .send()
            .await
            .unwrap();
        let download: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(download["protocol"], "usenet");
        assert_eq!(download["filesize"], 100);
        ids.push(id);
    }
    assert_ne!(
        ids[0], ids[1],
        "a second NZB must not collapse into the first"
    );
}

// =============================================================================
// SSRF guard (audit finding #7)
// =============================================================================

#[tokio::test]
async fn create_webhook_rejects_private_ip_url() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .post(format!("{}/api/v1/webhooks", base_url(addr)))
        .json(&serde_json::json!({
            "name": "evil",
            "url": "http://127.0.0.1:22/",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 400, "loopback must be rejected");

    let resp = client
        .post(format!("{}/api/v1/webhooks", base_url(addr)))
        .json(&serde_json::json!({
            "name": "metadata",
            "url": "http://169.254.169.254/latest/meta-data/",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status().as_u16(),
        400,
        "AWS metadata IP must be rejected"
    );

    let resp = client
        .post(format!("{}/api/v1/webhooks", base_url(addr)))
        .json(&serde_json::json!({
            "name": "scheme",
            "url": "file:///etc/passwd",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status().as_u16(),
        400,
        "non-http(s) scheme must be rejected"
    );
}

#[tokio::test]
async fn add_rss_feed_rejects_private_ip_url() {
    // Build a server with rss_feeds enabled so we exercise the SSRF guard
    // rather than the feature flag short-circuit.
    let config = Config {
        max_concurrent_downloads: 0,
        features: krake_core::config::FeatureFlags {
            rss_feeds: true,
            ..Default::default()
        },
        ..Config::default()
    };
    let state = krake_server::build_test_state(config);
    let app = krake_server::build_test_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = test_client();

    let resp = client
        .post(format!("{}/api/v1/rss", base_url(addr)))
        .json(&serde_json::json!({
            "name": "evil",
            "url": "http://10.0.0.1/feed.xml",
            "category": "",
            "interval_minutes": 60_u32,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status().as_u16(),
        400,
        "RFC1918 RSS URL must be rejected"
    );
}

// =============================================================================
// Plugin suggestion uses the configured registry (plugin-registry AC-17)
// =============================================================================

/// Serve an unsigned registry index with one plugin matching `suggest.test`.
async fn spawn_registry() -> SocketAddr {
    let index = r#"{"schema_version":1,"plugins":[{"id":"suggested","name":"Suggested",
        "version":"1.0.0","description":"","author":"","url_pattern":"https?://suggest\\.test/.+",
        "min_app_version":null,"sha256":"00","download_url":"https://x.test/p.ts","tags":[]}]}"#;
    let app = axum::Router::new().route(
        "/index.json",
        axum::routing::get(move || async move { index }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

async fn spawn_server_with_registry(registry_url: String) -> SocketAddr {
    let mut config = Config {
        max_concurrent_downloads: 0,
        ..Config::default()
    };
    config.update.plugin_registry_url = registry_url;
    let state = krake_server::build_test_state(config);
    let app = krake_server::build_test_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

async fn suggest(addr: SocketAddr, url: &str) -> serde_json::Value {
    test_client()
        .post(format!("{}/api/v1/plugins/suggest", base_url(addr)))
        .json(&serde_json::json!({ "url": url }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[tokio::test]
async fn ac17_suggest_uses_the_configured_registry() {
    let registry = spawn_registry().await;
    let addr = spawn_server_with_registry(format!("http://{registry}/index.json")).await;
    let body = suggest(addr, "https://suggest.test/file/1").await;
    assert_eq!(body["found"], true, "{body}");
    assert_eq!(body["plugin_id"], "suggested");
}

#[tokio::test]
async fn ac17_unreachable_registry_means_no_suggestion() {
    // Nothing listens on port 9 (discard) on loopback in the test sandbox.
    let addr = spawn_server_with_registry("http://127.0.0.1:9/index.json".into()).await;
    let body = suggest(addr, "https://suggest.test/file/1").await;
    assert_eq!(body["found"], false, "{body}");
}

#[test]
fn ac14_server_and_runtime_default_to_the_same_registry() {
    assert_eq!(
        krake_core::config::DEFAULT_PLUGIN_REGISTRY_URL,
        krake_plugin_runtime::registry::DEFAULT_INDEX_URL
    );
}

/// Serve the suggest-test index with `signature` as `index.json.sig`.
async fn spawn_signed_registry(signature: Option<String>) -> SocketAddr {
    let index = SUGGEST_INDEX;
    let mut app = axum::Router::new().route(
        "/index.json",
        axum::routing::get(move || async move { index }),
    );
    if let Some(sig) = signature {
        app = app.route(
            "/index.json.sig",
            axum::routing::get(move || async move { sig }),
        );
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

const SUGGEST_INDEX: &[u8] =
    br#"{"schema_version":1,"plugins":[{"id":"suggested","name":"Suggested",
        "version":"1.0.0","description":"","author":"","url_pattern":"https?://suggest\\.test/.+",
        "min_app_version":null,"sha256":"00","download_url":"https://x.test/p.ts","tags":[]}]}"#;

async fn spawn_server_with_pinned_registry(registry: SocketAddr, seed: &str) -> SocketAddr {
    use krake_plugin_runtime::registry::RegistryConfig;
    let pubkey = krake_plugin_runtime::registry_build::public_key_hex(seed).unwrap();
    let mut key = [0u8; 32];
    for (i, b) in key.iter_mut().enumerate() {
        *b = u8::from_str_radix(&pubkey[i * 2..i * 2 + 2], 16).unwrap();
    }
    let mut registry_config =
        RegistryConfig::for_index(format!("http://{registry}/index.json"), false)
            .with_index_verifier(key)
            .unwrap();
    registry_config.cache_path = None;
    let config = Config {
        max_concurrent_downloads: 0,
        ..Config::default()
    };
    let state = krake_server::build_test_state_with_registry(config, registry_config);
    let app = krake_server::build_test_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

#[tokio::test]
async fn ac17_suggest_verifies_the_registry_signature() {
    let seed = "0707070707070707070707070707070707070707070707070707070707070707";
    let other = "0909090909090909090909090909090909090909090909090909090909090909";
    let pubkey = krake_plugin_runtime::registry_build::public_key_hex(seed).unwrap();
    let good =
        krake_plugin_runtime::registry_build::sign_index(SUGGEST_INDEX, Some(seed), Some(&pubkey))
            .unwrap();
    let other_pub = krake_plugin_runtime::registry_build::public_key_hex(other).unwrap();
    let forged = krake_plugin_runtime::registry_build::sign_index(
        SUGGEST_INDEX,
        Some(other),
        Some(&other_pub),
    )
    .unwrap();

    // Valid signature from the pinned key: the suggestion is found.
    let registry = spawn_signed_registry(Some(good)).await;
    let addr = spawn_server_with_pinned_registry(registry, seed).await;
    let body = suggest(addr, "https://suggest.test/file/1").await;
    assert_eq!(body["found"], true, "{body}");

    // Signed by another key, or no signature at all: no suggestion.
    for signature in [Some(forged), None] {
        let registry = spawn_signed_registry(signature).await;
        let addr = spawn_server_with_pinned_registry(registry, seed).await;
        let body = suggest(addr, "https://suggest.test/file/1").await;
        assert_eq!(body["found"], false, "{body}");
    }
}

// =============================================================================
// Plugin suggestion for a list of links (plugin-suggest-on-add)
// =============================================================================

const LIST_INDEX: &str = r#"{"schema_version":1,"plugins":[
    {"id":"multi","name":"Multi","version":"1.0.0","description":"","author":"",
     "url_pattern":"https?://hoster\\.test/.+","min_app_version":null,"sha256":"00",
     "download_url":"https://x.test/m.ts","tags":["multi-hoster"]},
    {"id":"hoster","name":"Hoster","version":"2.0.0","description":"","author":"",
     "url_pattern":"https?://hoster\\.test/.+","min_app_version":null,"sha256":"00",
     "download_url":"https://x.test/h.ts","tags":["hoster"],
     "permissions":{"domains":["api.hoster.test"]}},
    {"id":"generic","name":"Generic","version":"1.0.0","description":"","author":"",
     "url_pattern":"https?://.+","min_app_version":null,"sha256":"00",
     "download_url":"https://x.test/g.ts","tags":["generic"]}]}"#;

async fn spawn_list_registry() -> SocketAddr {
    let app = axum::Router::new().route("/index.json", axum::routing::get(|| async { LIST_INDEX }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

async fn suggest_body(addr: SocketAddr, body: serde_json::Value) -> (u16, serde_json::Value) {
    let resp = test_client()
        .post(format!("{}/api/v1/plugins/suggest", base_url(addr)))
        .json(&body)
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    let json = resp.json().await.unwrap_or(serde_json::Value::Null);
    (status, json)
}

#[tokio::test]
async fn suggest_ac01_ac02_a_list_of_links_gets_hoster_suggestions() {
    let registry = spawn_list_registry().await;
    let addr = spawn_server_with_registry(format!("http://{registry}/index.json")).await;
    let (status, body) = suggest_body(
        addr,
        serde_json::json!({ "urls": [
            "https://unknown.test/1",
            "https://hoster.test/a",
            "https://hoster.test/b",
            "https://hoster.test/a",
            "ftp://hoster.test/c"
        ]}),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        body,
        serde_json::json!({ "suggestions": [{
            "id": "hoster",
            "name": "Hoster",
            "version": "2.0.0",
            "domains": ["api.hoster.test"],
            "urls": ["https://hoster.test/a", "https://hoster.test/b"],
            "action": "install"
        }]})
    );
}

#[tokio::test]
async fn suggest_ac07_an_unreachable_registry_means_no_install_suggestion() {
    let addr = spawn_server_with_registry("http://127.0.0.1:9/index.json".into()).await;
    let (status, body) = suggest_body(
        addr,
        serde_json::json!({ "urls": ["https://hoster.test/a"] }),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body, serde_json::json!({ "suggestions": [] }));
}

#[tokio::test]
async fn suggest_ac08_bad_lists_are_refused() {
    let registry = spawn_list_registry().await;
    let addr = spawn_server_with_registry(format!("http://{registry}/index.json")).await;

    let too_many: Vec<String> = (0..1001)
        .map(|i| format!("https://hoster.test/{i}"))
        .collect();
    let refused = [
        serde_json::json!({ "urls": too_many }),
        serde_json::json!({ "urls": [1, 2] }),
        serde_json::json!({ "urls": "https://hoster.test/a" }),
        serde_json::json!({ "urls": null }),
        serde_json::json!({ "url": "https://hoster.test/a", "urls": null }),
        serde_json::json!({ "url": null, "urls": ["https://hoster.test/a"] }),
        serde_json::json!({ "url": "https://hoster.test/a", "urls": ["https://hoster.test/a"] }),
        serde_json::json!({}),
    ];
    for body in refused {
        let (status, answer) = suggest_body(addr, body.clone()).await;
        assert!(
            status == 400 || status == 422,
            "{body} -> {status} {answer}"
        );
        assert!(answer.get("suggestions").is_none(), "{body} -> {answer}");
    }

    // Exactly 1 000 is allowed; an empty list has no suggestions.
    let thousand: Vec<String> = (0..1000)
        .map(|i| format!("https://hoster.test/{i}"))
        .collect();
    let (status, body) = suggest_body(addr, serde_json::json!({ "urls": thousand })).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        body["suggestions"][0]["urls"].as_array().unwrap().len(),
        1000
    );
    let (status, body) = suggest_body(addr, serde_json::json!({ "urls": [] })).await;
    assert_eq!(status, 200);
    assert_eq!(body, serde_json::json!({ "suggestions": [] }));
}

#[tokio::test]
async fn suggest_ac09_the_single_url_form_is_unchanged() {
    let registry = spawn_list_registry().await;
    let addr = spawn_server_with_registry(format!("http://{registry}/index.json")).await;
    // The single form keeps its old answer: first registry match, any tag.
    let body = suggest(addr, "https://hoster.test/a").await;
    assert_eq!(
        body,
        serde_json::json!({
            "found": true,
            "plugin_id": "multi",
            "plugin_name": "Multi",
            "install_command": "krake plugins install multi"
        })
    );
}
