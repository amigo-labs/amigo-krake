//! Optional login for private networks, end to end (spec `local-auth-bypass`).
//!
//! The test server listens on 127.0.0.1, so every request comes from a
//! private peer; the cases differ in the login mode, the headers and the
//! config.

use std::net::SocketAddr;
use std::time::Duration;

use futures_util::StreamExt;
use krake_core::config::{AuthMode, Config};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

const TOKEN: &str = "admin-token-for-tests";

struct Server {
    addr: SocketAddr,
    state: krake_server::api::AppState,
}

/// Start a server whose setup is complete, with a pre-shared admin token.
async fn spawn_with(tweak: impl FnOnce(&mut Config), env_mode: Option<AuthMode>) -> Server {
    let mut config = Config::default();
    config.server.bind = "0.0.0.0:0".into();
    config.server.setup_complete = true;
    config.server.api_token = Some(TOKEN.into());
    tweak(&mut config);
    let bind_is_loopback = config.server.is_bind_loopback();

    let mut state = krake_server::build_test_state(config);
    state.auth_mode_env = env_mode;
    let app = krake_server::build_full_test_router(state.clone(), None, bind_is_loopback).await;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    tokio::time::sleep(Duration::from_millis(20)).await;
    Server { addr, state }
}

async fn spawn(mode: &str) -> Server {
    let mode = mode.to_string();
    spawn_with(|c| c.server.auth_mode = mode, None).await
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
}

impl Server {
    fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.addr)
    }

    /// GET a protected route with extra headers, without any login.
    async fn status_of(&self, headers: &[(&str, &str)]) -> u16 {
        let mut req = client().get(self.url("/api/v1/downloads"));
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        req.send().await.unwrap().status().as_u16()
    }

    async fn get_config(&self) -> serde_json::Value {
        client()
            .get(self.url("/api/v1/config"))
            .bearer_auth(TOKEN)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    async fn put_config(&self, config: &serde_json::Value) -> reqwest::Response {
        client()
            .put(self.url("/api/v1/config"))
            .bearer_auth(TOKEN)
            .json(config)
            .send()
            .await
            .unwrap()
    }

    async fn put_with(&self, edit: impl FnOnce(&mut serde_json::Value)) -> u16 {
        let mut config = self.get_config().await;
        edit(&mut config);
        self.put_config(&config).await.status().as_u16()
    }
}

// ---- Modes ----

#[tokio::test]
async fn always_is_the_default_and_needs_a_login() {
    // AC-01
    let server = spawn_with(|_| {}, None).await;
    assert_eq!(
        server.state.coordinator.config().await.server.auth_mode,
        "always"
    );
    assert_eq!(server.status_of(&[]).await, 401);
}

#[tokio::test]
async fn private_lets_local_requests_in() {
    // AC-02, AC-07: a private client, a local host name, same origin.
    let server = spawn("private").await;
    assert_eq!(server.status_of(&[]).await, 200);
    let host = format!("casa:{}", server.addr.port());
    assert_eq!(server.status_of(&[("host", &host)]).await, 200);
    let origin = format!("http://{}", server.addr);
    assert_eq!(
        server
            .status_of(&[("origin", &origin), ("sec-fetch-site", "same-origin")])
            .await,
        200
    );
}

#[tokio::test]
async fn private_refuses_other_hosts_and_origins() {
    // AC-02: DNS rebinding and cross-site requests need a login.
    let server = spawn("private").await;
    assert_eq!(server.status_of(&[("host", "evil.example")]).await, 401);
    assert_eq!(
        server
            .status_of(&[("origin", "https://evil.example")])
            .await,
        401
    );
    assert_eq!(
        server.status_of(&[("sec-fetch-site", "cross-site")]).await,
        401
    );
    assert_eq!(server.status_of(&[("origin", "null")]).await, 401);
}

#[tokio::test]
async fn never_lets_direct_requests_in_on_a_loopback_bind() {
    // AC-03
    let server = spawn_with(
        |c| {
            c.server.bind = "127.0.0.1:0".into();
            c.server.auth_mode = "never".into();
        },
        None,
    )
    .await;
    assert_eq!(server.status_of(&[]).await, 200);
    assert_eq!(server.status_of(&[("host", "evil.example")]).await, 401);
    // A forwarded request is checked as in private: without trust_proxy, no.
    assert_eq!(
        server
            .status_of(&[("x-forwarded-for", "192.168.1.5")])
            .await,
        401
    );
}

// ---- Address checks ----

#[tokio::test]
async fn forwarded_requests_need_trust_proxy() {
    // AC-05
    let server = spawn("private").await;
    assert_eq!(
        server
            .status_of(&[("x-forwarded-for", "192.168.1.5")])
            .await,
        401
    );
    assert_eq!(
        server.status_of(&[("forwarded", "for=192.168.1.5")]).await,
        401
    );

    let proxied = spawn_with(
        |c| {
            c.server.auth_mode = "private".into();
            c.server.trust_proxy = true;
        },
        None,
    )
    .await;
    assert_eq!(
        proxied
            .status_of(&[("x-forwarded-for", "192.168.1.5")])
            .await,
        200
    );
    assert_eq!(
        proxied
            .status_of(&[("x-forwarded-for", "192.168.1.5, 203.0.113.7")])
            .await,
        401
    );
    assert_eq!(
        proxied.status_of(&[("x-forwarded-for", "unknown")]).await,
        401
    );
}

#[tokio::test]
async fn a_valid_login_wins_and_an_invalid_one_falls_back() {
    // AC-06
    let server = spawn("private").await;
    let me: serde_json::Value = client()
        .get(server.url("/api/v1/me"))
        .bearer_auth(TOKEN)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(me["kind"], "preshared");
    assert_eq!(
        server.status_of(&[("cookie", "krake_session=bogus")]).await,
        200
    );
    assert_eq!(
        server
            .status_of(&[("authorization", "Bearer revoked")])
            .await,
        200
    );
}

// ---- What a request without login sees ----

#[tokio::test]
async fn me_reports_the_mode_and_added_downloads_belong_to_no_one() {
    // AC-07
    let server = spawn("private").await;
    let me: serde_json::Value = client()
        .get(server.url("/api/v1/me"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(me["kind"], "local");
    assert_eq!(me["mode"], "private");

    let res: serde_json::Value = client()
        .post(server.url("/api/v1/downloads"))
        .json(&serde_json::json!({"url": "https://example.com/file.bin"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = res["id"].as_str().expect("download id");
    assert_eq!(server.state.coordinator.download_owner(id).await, None);
}

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect_ws(server: &Server) -> Ws {
    let req = format!("ws://{}/api/v1/ws", server.addr)
        .into_client_request()
        .unwrap();
    let (ws, response) = tokio_tungstenite::connect_async(req)
        .await
        .expect("ws connect");
    assert_eq!(response.status(), 101);
    ws
}

#[tokio::test]
async fn websocket_without_login_sees_events_and_is_closed_when_the_mode_changes() {
    // AC-07, AC-08
    let server = spawn("private").await;
    let mut ws = connect_ws(&server).await;

    // An event of a download someone else owns reaches it (admin view).
    let id = server
        .state
        .coordinator
        .add_download_with_options(
            "https://example.com/a.bin",
            None,
            None,
            0,
            Some("alice".into()),
        )
        .await
        .unwrap();
    let msg = tokio::time::timeout(Duration::from_secs(2), ws.next())
        .await
        .expect("event")
        .unwrap()
        .unwrap();
    assert!(msg.to_text().unwrap().contains(&id), "{msg:?}");

    // Switching to always closes it.
    assert_eq!(
        server
            .put_with(|c| c["server"]["auth_mode"] = "always".into())
            .await,
        200
    );
    let closed = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match ws.next().await {
                None | Some(Err(_)) => return true,
                Some(Ok(m)) if m.is_close() => return true,
                Some(Ok(_)) => continue,
            }
        }
    })
    .await
    .unwrap_or(false);
    assert!(closed, "the websocket must be closed after the switch");
}

// ---- Configuration ----

#[tokio::test]
async fn never_needs_a_loopback_bind_through_the_api() {
    // AC-04
    let server = spawn("always").await;
    assert_eq!(
        server
            .put_with(|c| c["server"]["auth_mode"] = "never".into())
            .await,
        400
    );
    assert_eq!(
        server.state.coordinator.config().await.server.auth_mode,
        "always"
    );

    let local = spawn_with(|c| c.server.bind = "127.0.0.1:0".into(), None).await;
    let status = local
        .put_with(|c| {
            c["server"]["auth_mode"] = "never".into();
            c["server"]["bind"] = "0.0.0.0:1516".into();
        })
        .await;
    assert_eq!(status, 400, "never together with a public bind");
    assert_eq!(
        local
            .put_with(|c| c["server"]["auth_mode"] = "never".into())
            .await,
        200
    );
    let status = local
        .put_with(|c| c["server"]["bind"] = "0.0.0.0:1516".into())
        .await;
    assert_eq!(status, 400, "a public bind while never is set");
    let config = local.state.coordinator.config().await;
    assert_eq!(
        (
            config.server.auth_mode.as_str(),
            config.server.bind.as_str()
        ),
        ("never", "127.0.0.1:0")
    );
}

#[tokio::test]
async fn concurrent_changes_cannot_combine_into_a_refused_state() {
    // AC-11: each change alone is fine, both together are not.
    for _ in 0..5 {
        let server = spawn_with(|c| c.server.bind = "127.0.0.1:0".into(), None).await;
        let base = server.get_config().await;
        let mut never = base.clone();
        never["server"]["auth_mode"] = "never".into();
        let mut public = base.clone();
        public["server"]["bind"] = "0.0.0.0:1516".into();
        let (a, b) = tokio::join!(server.put_config(&never), server.put_config(&public));
        let codes = [a.status().as_u16(), b.status().as_u16()];
        let config = server.state.coordinator.config().await;
        assert!(
            !(config.server.auth_mode == "never" && !config.server.is_bind_loopback()),
            "refused state reached: {codes:?}"
        );
    }
}

#[tokio::test]
async fn environment_mode_overrides_the_file_and_is_fixed() {
    // AC-09
    let server = spawn_with(
        |c| c.server.auth_mode = "always".into(),
        Some(AuthMode::Private),
    )
    .await;
    assert_eq!(
        server.status_of(&[]).await,
        200,
        "the environment's mode is in effect"
    );
    let config = server.get_config().await;
    assert_eq!(config["server"]["auth_mode"], "private");
    assert_eq!(config["server"]["auth_mode_from_env"], true);

    // Saving the document as read keeps the file's own value.
    assert_eq!(server.put_config(&config).await.status(), 200);
    assert_eq!(
        server.state.coordinator.config().await.server.auth_mode,
        "always"
    );
    // A different mode is refused.
    assert_eq!(
        server
            .put_with(|c| c["server"]["auth_mode"] = "never".into())
            .await,
        400
    );
}

#[tokio::test]
async fn unknown_modes_are_refused_through_the_api() {
    // AC-10
    let server = spawn("always").await;
    let res = {
        let mut config = server.get_config().await;
        config["server"]["auth_mode"] = "sometimes".into();
        server.put_config(&config).await
    };
    assert_eq!(res.status(), 400);
    let body = res.text().await.unwrap();
    assert!(
        body.contains("sometimes") && body.contains("always, private, never"),
        "{body}"
    );
}

#[tokio::test]
async fn a_mode_change_applies_to_the_next_request() {
    // AC-11
    let server = spawn("always").await;
    assert_eq!(server.status_of(&[]).await, 401);
    assert_eq!(
        server
            .put_with(|c| c["server"]["auth_mode"] = "private".into())
            .await,
        200
    );
    assert_eq!(server.status_of(&[]).await, 200);
    assert_eq!(
        server
            .put_with(|c| c["server"]["auth_mode"] = "always".into())
            .await,
        200
    );
    assert_eq!(server.status_of(&[]).await, 401);
}

// ---- Startup ----

#[test]
fn startup_refuses_unknown_modes_and_never_without_loopback() {
    // AC-04, AC-10
    let mut config = Config::default();
    config.server.auth_mode = "sometimes".into();
    let err = krake_server::resolve_auth_mode(&config, None).unwrap_err();
    assert!(
        err.contains("sometimes") && err.contains("always, private, never"),
        "{err}"
    );

    let config = Config::default();
    let err = krake_server::resolve_auth_mode(&config, Some("often")).unwrap_err();
    assert!(
        err.contains("often") && err.contains("KRAKE_AUTH_MODE"),
        "{err}"
    );

    let mut config = Config::default();
    config.server.bind = "0.0.0.0:1516".into();
    config.server.auth_mode = "never".into();
    let err = krake_server::resolve_auth_mode(&config, None).unwrap_err();
    assert!(
        err.contains("never") && err.contains("0.0.0.0:1516"),
        "{err}"
    );
    let err =
        krake_server::resolve_auth_mode(&Config { ..config.clone() }, Some("never")).unwrap_err();
    assert!(err.contains("0.0.0.0:1516"), "{err}");

    // The environment wins; an empty value counts as unset; case is ignored.
    let config = Config::default();
    assert_eq!(
        krake_server::resolve_auth_mode(&config, Some(" Private ")),
        Ok(Some(AuthMode::Private))
    );
    assert_eq!(
        krake_server::resolve_auth_mode(&config, Some("  ")),
        Ok(None)
    );
    assert_eq!(krake_server::resolve_auth_mode(&config, None), Ok(None));
}

// ---- Docs ----

#[test]
fn hosting_docs_describe_the_modes_and_their_limits() {
    // AC-17
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/hosting.md");
    let docs = std::fs::read_to_string(path).unwrap();
    for needle in [
        "KRAKE_AUTH_MODE",
        "`always`",
        "`private`",
        "`never`",
        "KRAKE_TRUST_PROXY",
        "chain",
        "public domain",
        "IPv6",
        "Docker Desktop",
        "rootless Docker or Podman",
        "userland proxy",
        "SSH",
        "keep `always`",
    ] {
        assert!(
            docs.contains(needle),
            "docs/hosting.md must mention {needle:?}"
        );
    }
}

#[tokio::test]
async fn websocket_through_a_proxy_is_closed_when_trust_proxy_goes_off() {
    // AC-08: not only a mode change closes it.
    let server = spawn_with(
        |c| {
            c.server.auth_mode = "private".into();
            c.server.trust_proxy = true;
        },
        None,
    )
    .await;
    let mut req = format!("ws://{}/api/v1/ws", server.addr)
        .into_client_request()
        .unwrap();
    req.headers_mut()
        .insert("x-forwarded-for", "192.168.1.5".parse().unwrap());
    let (mut ws, response) = tokio_tungstenite::connect_async(req)
        .await
        .expect("ws connect");
    assert_eq!(response.status(), 101);

    assert_eq!(
        server
            .put_with(|c| c["server"]["trust_proxy"] = false.into())
            .await,
        200
    );
    let closed = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match ws.next().await {
                None | Some(Err(_)) => return true,
                Some(Ok(m)) if m.is_close() => return true,
                Some(Ok(_)) => continue,
            }
        }
    })
    .await
    .unwrap_or(false);
    assert!(
        closed,
        "the websocket must be closed once trust_proxy is off"
    );
}

#[tokio::test]
async fn webhook_changes_cannot_undo_a_mode_change() {
    // AC-11: every config write goes through the same queue.
    for i in 0..10 {
        let server = spawn("private").await;
        let mut always = server.get_config().await;
        always["server"]["auth_mode"] = "always".into();
        let hook = client()
            .post(server.url("/api/v1/webhooks"))
            .bearer_auth(TOKEN)
            .json(&serde_json::json!({"name": format!("h{i}"), "url": "https://8.8.8.8/hook"}))
            .send();
        let (put, hook) = tokio::join!(server.put_config(&always), hook);
        assert_eq!(put.status(), 200);
        assert!(hook.unwrap().status().is_success());
        assert_eq!(
            server.state.coordinator.config().await.server.auth_mode,
            "always",
            "a webhook write restored the old mode"
        );
    }
}
