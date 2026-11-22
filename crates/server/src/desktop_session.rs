//! `GET /api/v1/desktop/session?token=<hex>` — the desktop app's sign-in
//! (`docs/specs/desktop-app.md`, AC-3).
//!
//! The desktop app generates a random token per launch and opens its window
//! at this URL first. The right token is exchanged once for a session cookie
//! of the principal `desktop` and a redirect to `/`; every other request —
//! a wrong token, or the right one a second time — gets `401`. The route is
//! mounted only when the server runs embedded in the desktop app, sits
//! outside `require_auth`, and is rate-limited like `/login`.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    Router,
    extract::{ConnectInfo, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::api::AppState;
use crate::auth::{self, SESSION_COOKIE};
use crate::login::{self, RateLimiter, rl_allow};

/// The principal a desktop session belongs to.
pub const DESKTOP_PRINCIPAL: &str = "desktop";

#[derive(Clone)]
struct SessionState {
    app: AppState,
    /// The launch token until it was used once.
    token: Arc<Mutex<Option<String>>>,
    rl: RateLimiter,
}

#[derive(Deserialize)]
struct TokenQuery {
    token: Option<String>,
}

/// The desktop sign-in route for the launch token `token`.
pub fn desktop_session_router(state: AppState, token: String) -> Router {
    Router::new()
        .route("/api/v1/desktop/session", get(exchange))
        .with_state(SessionState {
            app: state,
            token: Arc::new(Mutex::new(Some(token))),
            rl: Arc::new(Mutex::new(HashMap::new())),
        })
}

async fn exchange(
    State(s): State<SessionState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(query): Query<TokenQuery>,
) -> Response {
    let cfg = s.app.coordinator.config().await;
    let ip = auth::client_ip(&headers, Some(peer), cfg.server.trust_proxy);
    if !rl_allow(&s.rl, &ip).await {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }

    let offered = query.token.unwrap_or_default();
    {
        let mut slot = s.token.lock().await;
        match slot.as_deref() {
            Some(expected) if auth::ct_eq(offered.as_bytes(), expected.as_bytes()) => {
                // Single use: the token is gone before the session exists.
                *slot = None;
            }
            _ => return StatusCode::UNAUTHORIZED.into_response(),
        }
    }

    let session_id =
        match login::create_session(&s.app, DESKTOP_PRINCIPAL, cfg.server.session_ttl_secs).await {
            Ok(id) => id,
            Err(e) => {
                tracing::error!("desktop session: create failed: {e}");
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };
    let cookie = Cookie::build((SESSION_COOKIE, session_id))
        .http_only(true)
        .same_site(SameSite::Strict)
        .path("/")
        .max_age(cookie::time::Duration::seconds(cfg.server.session_ttl_secs))
        .build();

    (
        StatusCode::SEE_OTHER,
        [
            (header::LOCATION, "/".to_string()),
            (header::SET_COOKIE, cookie.to_string()),
        ],
    )
        .into_response()
}
