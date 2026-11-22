//! The server embedded in the desktop app (`docs/specs/desktop-app.md`):
//! `embedded::serve` on a bound listener, the per-launch desktop session,
//! and the `krake-server` binary on the same code path.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use krake_core::config::Config;
use krake_server::embedded::{self, DesktopOptions, ServeOptions, ServerHandle};
use krake_server::update_api::{DesktopModeState, DesktopUpdateInfo, DesktopUpdater, UpdateMode};

const TOKEN: &str = "6b72616b652d6465736b746f702d746f6b656e2d666f722d74657374732d2121";

/// A scripted desktop updater; every call is recorded.
#[derive(Default)]
pub struct FakeUpdater {
    pub can_self_update: bool,
    pub check_result: Mutex<Option<Result<Option<DesktopUpdateInfo>, String>>>,
    pub apply_result: Mutex<Option<Result<String, String>>>,
    pub calls: Mutex<Vec<String>>,
    pub state: Mutex<Option<DesktopModeState>>,
    pub save_fails: bool,
    pub panic_on_network: bool,
}

impl FakeUpdater {
    fn record(&self, call: &str) {
        self.calls.lock().unwrap().push(call.to_string());
    }

    fn current(&self) -> DesktopModeState {
        self.state
            .lock()
            .unwrap()
            .clone()
            .unwrap_or(DesktopModeState {
                update_mode: UpdateMode::Auto,
                update_mode_chosen: false,
                question_pending: true,
                can_self_update: self.can_self_update,
                pending_restart: None,
                announcement: None,
                saved: true,
            })
    }
}

#[async_trait::async_trait]
impl DesktopUpdater for FakeUpdater {
    fn can_self_update(&self) -> bool {
        self.can_self_update
    }
    fn pending_restart(&self) -> Option<String> {
        self.current().pending_restart
    }
    async fn check(&self) -> Result<Option<DesktopUpdateInfo>, String> {
        assert!(!self.panic_on_network, "check must not run here");
        self.record("check");
        self.check_result
            .lock()
            .unwrap()
            .clone()
            .unwrap_or(Ok(None))
    }
    async fn apply(&self) -> Result<String, String> {
        assert!(!self.panic_on_network, "apply must not run here");
        self.record("apply");
        self.apply_result
            .lock()
            .unwrap()
            .clone()
            .unwrap_or(Ok("9.9.9".into()))
    }
    fn mode_state(&self) -> DesktopModeState {
        self.record("mode_state");
        self.current()
    }
    fn set_mode(&self, mode: UpdateMode) -> DesktopModeState {
        self.record(&format!("set_mode:{mode:?}"));
        let mut state = self.current();
        state.update_mode = mode;
        state.update_mode_chosen = !self.save_fails;
        state.question_pending = false;
        state.saved = !self.save_fails;
        *self.state.lock().unwrap() = Some(DesktopModeState {
            saved: true,
            ..state.clone()
        });
        state
    }
    fn dismiss_question(&self) {
        self.record("dismiss");
        let mut state = self.current();
        state.question_pending = false;
        *self.state.lock().unwrap() = Some(state);
    }
    fn acknowledge(&self, version: &str) -> Result<(), String> {
        self.record(&format!("ack:{version}"));
        let mut state = self.current();
        if state.announcement.as_deref() != Some(version) {
            return Err(format!("{version} is not the pending announcement"));
        }
        state.announcement = None;
        *self.state.lock().unwrap() = Some(state);
        Ok(())
    }
}

pub struct Running {
    pub handle: ServerHandle,
    pub _dir: tempfile::TempDir,
}

impl Running {
    pub fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.handle.local_addr)
    }
}

/// Start the full stack on `127.0.0.1:0`; `desktop` adds desktop mode.
pub async fn start(desktop: Option<Arc<FakeUpdater>>) -> Running {
    let dir = tempfile::tempdir().unwrap();
    let config = Config {
        download_dir: dir.path().join("downloads").display().to_string(),
        temp_dir: dir.path().join("downloads/.tmp").display().to_string(),
        ..Config::default()
    };
    std::fs::create_dir_all(dir.path().join("plugins")).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let handle = embedded::serve(ServeOptions {
        config,
        config_path: dir.path().join("config.toml"),
        db_path: dir.path().join("krake.db"),
        plugins_dir: dir.path().join("plugins"),
        cache_dir: dir.path().join("cache"),
        listener,
        start_clicknload: false,
        desktop: desktop.map(|updater| DesktopOptions {
            session_token: TOKEN.to_string(),
            updater,
        }),
    })
    .await
    .expect("serve");
    Running { handle, _dir: dir }
}

/// `/` serves the built web UI; CI builds the server with an empty
/// `web-ui/dist` stub, where the static router answers 404.
fn web_ui_status() -> u16 {
    let index =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web-ui/dist/index.html");
    if index.is_file() { 200 } else { 404 }
}

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
}

/// Exchange the launch token; returns the `krake_session=…` cookie pair.
pub async fn sign_in(server: &Running) -> String {
    let resp = client()
        .get(server.url(&format!("/api/v1/desktop/session?token={TOKEN}")))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);
    let set_cookie = resp.headers()["set-cookie"].to_str().unwrap().to_string();
    set_cookie.split(';').next().unwrap().to_string()
}

#[tokio::test]
async fn ac3_the_launch_token_signs_in_once() {
    let server = start(Some(Arc::new(FakeUpdater::default()))).await;

    // A wrong token does not use it up.
    let wrong = client()
        .get(server.url("/api/v1/desktop/session?token=00"))
        .send()
        .await
        .unwrap();
    assert_eq!(wrong.status(), 401);
    let none = client()
        .get(server.url("/api/v1/desktop/session"))
        .send()
        .await
        .unwrap();
    assert_eq!(none.status(), 401);

    let resp = client()
        .get(server.url(&format!("/api/v1/desktop/session?token={TOKEN}")))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);
    assert_eq!(resp.headers()["location"], "/");
    let set_cookie = resp.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .to_ascii_lowercase();
    assert!(set_cookie.starts_with("krake_session="), "{set_cookie}");
    assert!(set_cookie.contains("httponly"), "{set_cookie}");
    assert!(set_cookie.contains("samesite=strict"), "{set_cookie}");

    // Single use.
    let again = client()
        .get(server.url(&format!("/api/v1/desktop/session?token={TOKEN}")))
        .send()
        .await
        .unwrap();
    assert_eq!(again.status(), 401);
}

#[tokio::test]
async fn ac3_the_session_belongs_to_the_desktop_principal() {
    let server = start(Some(Arc::new(FakeUpdater::default()))).await;
    let cookie = sign_in(&server).await;
    let me: serde_json::Value = client()
        .get(server.url("/api/v1/me"))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(me["username"], "desktop", "{me}");
}

#[tokio::test]
async fn ac3_without_desktop_mode_the_route_does_not_exist() {
    let server = start(None).await;
    let resp = client()
        .get(server.url(&format!("/api/v1/desktop/session?token={TOKEN}")))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn ac4_desktop_mode_needs_no_setup() {
    let server = start(Some(Arc::new(FakeUpdater::default()))).await;
    let status: serde_json::Value = client()
        .get(server.url("/api/v1/setup/status"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(status["needs_setup"], false, "{status}");

    // A plain server with no admin yet still asks for setup.
    let plain = start(None).await;
    let status: serde_json::Value = client()
        .get(plain.url("/api/v1/setup/status"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(status["needs_setup"], true, "{status}");
}

#[tokio::test]
async fn ac1_ac5_api_needs_a_credential_and_the_web_ui_is_served() {
    let server = start(Some(Arc::new(FakeUpdater::default()))).await;

    // Loopback gives no trust: no cookie, no token → 401.
    let anonymous = client()
        .get(server.url("/api/v1/status"))
        .send()
        .await
        .unwrap();
    assert_eq!(anonymous.status(), 401);
    let downloads = client()
        .get(server.url("/api/v1/downloads"))
        .send()
        .await
        .unwrap();
    assert_eq!(downloads.status(), 401);

    let cookie = sign_in(&server).await;
    let signed_in = client()
        .get(server.url("/api/v1/status"))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(signed_in.status(), 200);

    // The static web UI and the security headers come from the same stack.
    let index = client().get(server.url("/")).send().await.unwrap();
    assert_eq!(index.status(), web_ui_status());
    assert!(index.headers().contains_key("content-security-policy"));

    // WebSocket, updates and feedback are mounted (behind auth).
    for path in ["/api/v1/ws", "/api/v1/updates/check", "/api/v1/feedback"] {
        let resp = client().get(server.url(path)).send().await.unwrap();
        assert_eq!(resp.status(), 401, "{path} is not mounted behind auth");
    }
}

#[tokio::test]
async fn ac1_dropping_the_handle_keeps_serving_and_shutdown_stops() {
    let Running { handle, _dir } = start(None).await;
    let addr: SocketAddr = handle.local_addr;
    let ServerHandle { shutdown, task, .. } = handle;
    let status = client()
        .get(format!("http://{addr}/api/v1/setup/status"))
        .send()
        .await
        .unwrap();
    assert_eq!(status.status(), 200);
    shutdown.send(()).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(10), task)
        .await
        .expect("server stops after shutdown")
        .unwrap()
        .unwrap();
}

/// The `krake-server` binary goes through `embedded::serve` too (AC-1).
#[tokio::test]
async fn ac1_the_server_binary_serves_the_same_stack() {
    let dir = tempfile::tempdir().unwrap();
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_krake-server"))
        .current_dir(dir.path())
        .env("KRAKE_CONFIG_DIR", dir.path())
        .env("KRAKE_BIND", format!("127.0.0.1:{port}"))
        .env("RUST_LOG", "warn")
        .kill_on_drop(true)
        .spawn()
        .expect("start krake-server");

    let base = format!("http://127.0.0.1:{port}");
    let mut ready = false;
    for _ in 0..100 {
        if let Ok(resp) = client()
            .get(format!("{base}/api/v1/setup/status"))
            .send()
            .await
            && resp.status() == 200
        {
            ready = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(ready, "krake-server did not come up");

    let status = client()
        .get(format!("{base}/api/v1/status"))
        .send()
        .await
        .unwrap();
    assert_eq!(status.status(), 401);
    let index = client().get(format!("{base}/")).send().await.unwrap();
    assert_eq!(index.status(), web_ui_status());
    assert!(index.headers().contains_key("content-security-policy"));
    // No desktop route on the plain server.
    let session = client()
        .get(format!("{base}/api/v1/desktop/session?token={TOKEN}"))
        .send()
        .await
        .unwrap();
    assert_eq!(session.status(), 404);
    child.kill().await.unwrap();
}

#[tokio::test]
async fn ac9_tray_pauses_and_resumes_every_download() {
    use krake_server::desktop::tray;
    let server = start(Some(Arc::new(FakeUpdater::default()))).await;
    let coordinator = server.handle.coordinator.clone();
    // Unreachable test addresses: the rows stay queued or fail, never finish.
    let mut ids = Vec::new();
    for n in 0..3 {
        ids.push(
            coordinator
                .add_download(&format!("https://example.invalid/{n}.bin"), None)
                .await
                .unwrap(),
        );
    }
    let paused = tray::pause_all(&coordinator).await.unwrap();
    assert!(paused >= 1, "nothing paused");
    for id in &ids {
        let row = coordinator
            .storage()
            .get_download(id)
            .await
            .unwrap()
            .unwrap();
        assert!(
            matches!(row.status.as_str(), "paused" | "failed"),
            "{id}: {}",
            row.status
        );
    }
    let resumed = tray::resume_all(&coordinator).await.unwrap();
    assert_eq!(resumed, paused);
    for id in &ids {
        let row = coordinator
            .storage()
            .get_download(id)
            .await
            .unwrap()
            .unwrap();
        assert_ne!(row.status, "paused", "{id} still paused");
    }
}

async fn signed_in(updater: FakeUpdater) -> (Running, String, Arc<FakeUpdater>) {
    let updater = Arc::new(updater);
    let server = start(Some(updater.clone())).await;
    let cookie = sign_in(&server).await;
    (server, cookie, updater)
}

async fn call(
    server: &Running,
    cookie: &str,
    method: reqwest::Method,
    path: &str,
    body: Option<&str>,
) -> (u16, serde_json::Value) {
    let mut req = client()
        .request(method, server.url(path))
        .header("cookie", cookie);
    if let Some(body) = body {
        req = req
            .header("content-type", "application/json")
            .body(body.to_string());
    }
    let resp = req.send().await.unwrap();
    let status = resp.status().as_u16();
    let json = resp.json().await.unwrap_or(serde_json::Value::Null);
    (status, json)
}

#[tokio::test]
async fn ac17_check_reports_the_desktop_update_and_its_state() {
    let fake = FakeUpdater {
        can_self_update: true,
        ..Default::default()
    };
    *fake.check_result.lock().unwrap() = Some(Ok(Some(DesktopUpdateInfo {
        version: "9.9.9".into(),
        notes: Some("notes".into()),
    })));
    let (server, cookie, _) = signed_in(fake).await;
    let (status, body) = call(
        &server,
        &cookie,
        reqwest::Method::GET,
        "/api/v1/updates/check",
        None,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let core = &body["core"];
    assert_eq!(core["latest_version"], "9.9.9");
    assert_eq!(core["update_available"], true);
    assert_eq!(core["distribution"], "tauri");
    assert_eq!(core["can_self_update"], true);
    assert_eq!(core["pending_restart"], serde_json::Value::Null);
    assert_eq!(core["update_mode"], "auto");
    assert_eq!(core["update_mode_chosen"], false);

    let fake = FakeUpdater::default();
    *fake.check_result.lock().unwrap() = Some(Ok(None));
    let (server, cookie, _) = signed_in(fake).await;
    let (_, body) = call(
        &server,
        &cookie,
        reqwest::Method::GET,
        "/api/v1/updates/check",
        None,
    )
    .await;
    assert_eq!(body["core"]["update_available"], false);
    assert_eq!(body["core"]["can_self_update"], false);

    let fake = FakeUpdater::default();
    *fake.check_result.lock().unwrap() = Some(Err("offline".into()));
    let (server, cookie, _) = signed_in(fake).await;
    let (status, _) = call(
        &server,
        &cookie,
        reqwest::Method::GET,
        "/api/v1/updates/check",
        None,
    )
    .await;
    assert_eq!(status, 502);
}

#[tokio::test]
async fn ac17_apply_installs_through_the_hook() {
    let (server, cookie, updater) = signed_in(FakeUpdater {
        can_self_update: true,
        ..Default::default()
    })
    .await;
    let (status, body) = call(
        &server,
        &cookie,
        reqwest::Method::POST,
        "/api/v1/updates/core",
        None,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        body,
        serde_json::json!({ "status": "ready", "version": "9.9.9" })
    );
    assert!(updater.calls.lock().unwrap().contains(&"apply".to_string()));

    // Not allowed (deb/rpm, debug): 400, nothing applied.
    let (server, cookie, updater) = signed_in(FakeUpdater::default()).await;
    let (status, _) = call(
        &server,
        &cookie,
        reqwest::Method::POST,
        "/api/v1/updates/core",
        None,
    )
    .await;
    assert_eq!(status, 400);
    assert!(!updater.calls.lock().unwrap().contains(&"apply".to_string()));

    // Download or signature failure: 502 with the reason.
    let fake = FakeUpdater {
        can_self_update: true,
        ..Default::default()
    };
    *fake.apply_result.lock().unwrap() = Some(Err("signature mismatch".into()));
    let (server, cookie, _) = signed_in(fake).await;
    let (status, body) = call(
        &server,
        &cookie,
        reqwest::Method::POST,
        "/api/v1/updates/core",
        None,
    )
    .await;
    assert_eq!(status, 502);
    assert_eq!(body["error"], "signature mismatch");
}

#[tokio::test]
async fn ac31_put_mode_makes_the_choice_or_records_a_close() {
    for (mode, expected) in [("auto", "auto"), ("notify", "notify"), ("off", "off")] {
        let (server, cookie, updater) = signed_in(FakeUpdater::default()).await;
        let body = format!(r#"{{"mode":"{mode}"}}"#);
        let (status, state) = call(
            &server,
            &cookie,
            reqwest::Method::PUT,
            "/api/v1/updates/mode",
            Some(&body),
        )
        .await;
        assert_eq!(status, 200, "{state}");
        assert_eq!(state["update_mode"], expected);
        assert_eq!(state["saved"], true);
        assert!(
            updater
                .calls
                .lock()
                .unwrap()
                .iter()
                .any(|c| c.starts_with("set_mode"))
        );
    }

    let (server, cookie, updater) = signed_in(FakeUpdater::default()).await;
    let (status, state) = call(
        &server,
        &cookie,
        reqwest::Method::PUT,
        "/api/v1/updates/mode",
        Some(r#"{"dismissed":true}"#),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(state["question_pending"], false);
    assert!(
        updater
            .calls
            .lock()
            .unwrap()
            .contains(&"dismiss".to_string())
    );

    for bad in [
        r#"{"mode":"sometimes"}"#,
        r#"{"dismissed":false}"#,
        r#"{}"#,
        "nope",
        r#"{"mode":"auto","dismissed":true}"#,
    ] {
        let (status, _) = call(
            &server,
            &cookie,
            reqwest::Method::PUT,
            "/api/v1/updates/mode",
            Some(bad),
        )
        .await;
        assert_eq!(status, 400, "{bad}");
    }

    // A failed write: effective anyway, `saved: false`.
    let (server, cookie, _) = signed_in(FakeUpdater {
        save_fails: true,
        ..Default::default()
    })
    .await;
    let (status, state) = call(
        &server,
        &cookie,
        reqwest::Method::PUT,
        "/api/v1/updates/mode",
        Some(r#"{"mode":"off"}"#),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(state["update_mode"], "off");
    assert_eq!(state["saved"], false);
}

#[tokio::test]
async fn ac34_get_mode_answers_without_the_network() {
    let (server, cookie, _) = signed_in(FakeUpdater {
        panic_on_network: true,
        can_self_update: true,
        ..Default::default()
    })
    .await;
    let (status, state) = call(
        &server,
        &cookie,
        reqwest::Method::GET,
        "/api/v1/updates/mode",
        None,
    )
    .await;
    assert_eq!(status, 200);
    for key in [
        "update_mode",
        "update_mode_chosen",
        "question_pending",
        "can_self_update",
        "pending_restart",
        "announcement",
    ] {
        assert!(state.get(key).is_some(), "{key} missing in {state}");
    }
    assert_eq!(state["question_pending"], true);
}

#[tokio::test]
async fn ac35_announced_marks_only_the_pending_version() {
    let fake = FakeUpdater::default();
    *fake.state.lock().unwrap() = Some(DesktopModeState {
        update_mode: UpdateMode::Notify,
        update_mode_chosen: true,
        question_pending: false,
        can_self_update: true,
        pending_restart: None,
        announcement: Some("2.0.0".into()),
        saved: true,
    });
    let (server, cookie, _) = signed_in(fake).await;
    let (status, _) = call(
        &server,
        &cookie,
        reqwest::Method::POST,
        "/api/v1/updates/announced",
        Some(r#"{"version":"1.0.0"}"#),
    )
    .await;
    assert_eq!(status, 400);
    let (status, _) = call(
        &server,
        &cookie,
        reqwest::Method::POST,
        "/api/v1/updates/announced",
        Some(r#"{"v":"2.0.0"}"#),
    )
    .await;
    assert_eq!(status, 400);
    let (status, state) = call(
        &server,
        &cookie,
        reqwest::Method::POST,
        "/api/v1/updates/announced",
        Some(r#"{"version":"2.0.0"}"#),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(state["announcement"], serde_json::Value::Null);
}

#[tokio::test]
async fn ac31_ac34_ac35_routes_are_absent_outside_desktop_mode() {
    let server = start(None).await;
    // A plain server with no admin yet lets loopback through setup; sign in
    // is not possible, so use the setup flow to get a session.
    let resp = client()
        .post(server.url("/api/v1/setup/complete"))
        .json(&serde_json::json!({ "username": "admin", "password": "correct horse" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let cookie = resp.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    for (method, path, body) in [
        (reqwest::Method::GET, "/api/v1/updates/mode", None),
        (
            reqwest::Method::PUT,
            "/api/v1/updates/mode",
            Some(r#"{"mode":"auto"}"#),
        ),
        (
            reqwest::Method::POST,
            "/api/v1/updates/announced",
            Some(r#"{"version":"1.0.0"}"#),
        ),
    ] {
        let (status, _) = call(&server, &cookie, method, path, body).await;
        assert_eq!(status, 404, "{path}");
    }
}
