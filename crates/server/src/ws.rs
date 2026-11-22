//! WebSocket handler for live progress updates.
//!
//! Events are scoped per authenticated principal: a browser login session only
//! receives events for downloads it created, while operator-level credentials
//! (the pre-shared token and API tokens) receive everything. Events not tied
//! to a specific download (queue/plugin notifications) go to every client.

use std::sync::Arc;

use axum::{
    Extension, Router,
    extract::{ConnectInfo, State, WebSocketUpgrade, ws::Message},
    http::{HeaderMap, Uri},
    response::Response,
    routing::get,
};
use tokio::sync::broadcast::Receiver;
use tracing::{debug, warn};

use crate::api::AppState;
use crate::auth::Principal;
use krake_core::coordinator::{Coordinator, DownloadEvent};

pub fn ws_router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/ws", get(ws_handler))
        .with_state(state)
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    // Populated by the auth middleware in production. `Option` so the
    // auth-less test router (and any unauthenticated deployment) still works;
    // a missing principal is treated as admin — see `can_see`.
    principal: Option<Extension<Principal>>,
    connect: Option<Extension<ConnectInfo<std::net::SocketAddr>>>,
    headers: HeaderMap,
    uri: Uri,
) -> Response {
    // Subscribe *before* the upgrade so the receiver is live by the time the
    // client sees the 101 response. If we subscribed inside `on_upgrade`,
    // the on_upgrade future could still be scheduling when the first event
    // is broadcast, causing the client to silently miss it.
    let rx = state.coordinator.subscribe();
    let coordinator = state.coordinator.clone();
    let mut principal = principal.map(|Extension(p)| p);
    // In the desktop app the desktop session is the only user: it sees every
    // download, also those Click'n'Load or the watch folder added unowned.
    if state.desktop_updater.is_some()
        && matches!(&principal, Some(Principal::Session { username, .. })
            if username == crate::desktop_session::DESKTOP_PRINCIPAL)
    {
        principal = Some(Principal::Preshared);
    }
    // A socket let in without a login is checked again after every config
    // change and closed once the login mode no longer lets it in (AC-08).
    let recheck = matches!(principal, Some(Principal::Local { .. })).then(|| LocalRecheck {
        // Subscribed here, before the upgrade; `handle_socket` checks once
        // more right away, so a change between the auth decision and this
        // subscription is not missed.
        epoch: state.config_epoch.subscribe(),
        state: state.clone(),
        peer: connect.map(|Extension(ConnectInfo(addr))| addr.ip()),
        headers,
        uri,
    });
    ws.on_upgrade(move |socket| handle_socket(socket, rx, coordinator, principal, recheck))
}

/// What a socket let in without a login needs to be checked again.
struct LocalRecheck {
    epoch: tokio::sync::watch::Receiver<u64>,
    state: AppState,
    peer: Option<std::net::IpAddr>,
    headers: HeaderMap,
    uri: Uri,
}

impl LocalRecheck {
    async fn still_let_in(&self) -> bool {
        crate::auth::local_principal(&self.state, self.peer, &self.headers, &self.uri)
            .await
            .is_some()
    }
}

/// Whether `principal` is allowed to see events for a download owned by
/// `owner`. Operator credentials (pre-shared / API token) and — for
/// backward compatibility with auth-less deployments — a missing principal
/// see everything; a login session sees only downloads it owns.
fn can_see(principal: &Option<Principal>, owner: Option<&str>) -> bool {
    match principal {
        None
        | Some(Principal::Preshared | Principal::ApiToken { .. } | Principal::Local { .. }) => true,
        Some(Principal::Session { username, .. }) => owner == Some(username.as_str()),
    }
}

async fn handle_socket(
    mut socket: axum::extract::ws::WebSocket,
    mut rx: Receiver<DownloadEvent>,
    coordinator: Arc<Coordinator>,
    principal: Option<Principal>,
    recheck: Option<LocalRecheck>,
) {
    debug!("WebSocket client connected");
    let mut epoch = recheck.as_ref().map(|r| r.epoch.clone());
    if let Some(r) = &recheck
        && !r.still_let_in().await
    {
        let _ = socket.send(Message::Close(None)).await;
        return;
    }

    loop {
        let received = match epoch.as_mut() {
            None => rx.recv().await,
            Some(epoch) => tokio::select! {
                received = rx.recv() => received,
                changed = epoch.changed() => {
                    let still = match (&recheck, changed) {
                        (Some(r), Ok(())) => r.still_let_in().await,
                        _ => false,
                    };
                    if !still {
                        debug!("Closing a WebSocket the login mode no longer lets in");
                        let _ = socket.send(Message::Close(None)).await;
                        break;
                    }
                    continue;
                }
            },
        };
        match received {
            Ok(event) => {
                // Filter per principal. Events not tied to a download are
                // global; download-scoped events are gated by ownership. The
                // owner lookup is skipped for admin principals (fast path).
                let deliver = match event.subject_download_id() {
                    None => true,
                    Some(id) => match &principal {
                        None
                        | Some(
                            Principal::Preshared
                            | Principal::ApiToken { .. }
                            | Principal::Local { .. },
                        ) => true,
                        Some(Principal::Session { .. }) => {
                            let owner = coordinator.download_owner(id).await;
                            can_see(&principal, owner.as_deref())
                        }
                    },
                };
                if !deliver {
                    continue;
                }

                // DownloadEvent derives Serialize with tag="type", so we can serialize directly
                if let Ok(json) = serde_json::to_string(&event)
                    && socket.send(Message::Text(json.into())).await.is_err()
                {
                    break;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                warn!("WebSocket client lagged, skipped {n} events");
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }

    debug!("WebSocket client disconnected");
}
