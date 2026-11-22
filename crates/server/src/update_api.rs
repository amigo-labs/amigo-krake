//! REST API routes for update management.

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use krake_core::updater::{self, CoreUpdateStatus};

use crate::api::AppState;

/// The desktop app's update choice (`docs/specs/desktop-app.md`, AC-25).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UpdateMode {
    /// Install verified updates by themselves (the default).
    Auto,
    /// Announce each new version once; install on request.
    Notify,
    /// No background checks.
    Off,
}

/// Update-choice state, answered without network access (AC-34).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DesktopModeState {
    pub update_mode: UpdateMode,
    pub update_mode_chosen: bool,
    pub question_pending: bool,
    pub can_self_update: bool,
    pub pending_restart: Option<String>,
    pub announcement: Option<String>,
    /// `false` after a failed write of `desktop.json` (only in PUT answers).
    pub saved: bool,
}

/// A newer desktop version found by a check.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesktopUpdateInfo {
    pub version: String,
    pub notes: Option<String>,
}

/// The desktop app's updater. Implemented by the Tauri app on top of
/// tauri-plugin-updater; the server only calls it (AC-17, AC-31, AC-34,
/// AC-35).
#[async_trait::async_trait]
pub trait DesktopUpdater: Send + Sync {
    /// Whether this copy may update itself (AC-16).
    fn can_self_update(&self) -> bool;
    /// Version downloaded and verified in this session, if any.
    fn pending_restart(&self) -> Option<String>;
    /// Check only; `None` when up to date.
    async fn check(&self) -> Result<Option<DesktopUpdateInfo>, String>;
    /// Check, download, verify and install / hold (AC-14, AC-15); returns
    /// the version.
    async fn apply(&self) -> Result<String, String>;
    /// Update-choice state without network access (AC-34).
    fn mode_state(&self) -> DesktopModeState;
    /// Make a choice effective for this run and store it (AC-31).
    fn set_mode(&self, mode: UpdateMode) -> DesktopModeState;
    /// Record a close of the first-start question without a choice (AC-26).
    fn dismiss_question(&self);
    /// Mark `version` as announced; `Err` if it is not the pending one
    /// (AC-35).
    fn acknowledge(&self, version: &str) -> Result<(), String>;
}

pub fn update_router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/updates/check", get(check_updates))
        .route("/api/v1/updates/core", post(apply_core_update))
        .route(
            "/api/v1/updates/mode",
            get(get_update_mode).put(put_update_mode),
        )
        .route("/api/v1/updates/announced", post(post_announced))
        .route(
            "/api/v1/updates/plugins/available",
            get(list_available_plugins),
        )
        .route(
            "/api/v1/updates/plugins/update-all",
            post(update_all_plugins),
        )
        .route("/api/v1/updates/plugins/{id}", post(update_plugin))
        .route("/api/v1/updates/plugins/{id}/install", post(install_plugin))
        .with_state(state)
}

// --- Response types ---

#[derive(Serialize)]
struct CheckUpdatesResponse {
    core: CoreUpdateInfo,
    plugins: Vec<PluginUpdateEntry>,
}

#[derive(Serialize)]
struct CoreUpdateInfo {
    current_version: String,
    latest_version: Option<String>,
    update_available: bool,
    release_notes: Option<String>,
    distribution: String,
    can_self_update: bool,
    /// Desktop mode: the version downloaded and verified in this run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pending_restart: Option<Option<String>>,
    /// Desktop mode: the effective update choice (AC-17).
    #[serde(skip_serializing_if = "Option::is_none")]
    update_mode: Option<UpdateMode>,
    /// Desktop mode: whether a choice is stored (AC-17).
    #[serde(skip_serializing_if = "Option::is_none")]
    update_mode_chosen: Option<bool>,
}

#[derive(Serialize)]
struct PluginUpdateEntry {
    plugin_id: String,
    current_version: Option<String>,
    available_version: String,
    is_new: bool,
    /// Domains the new version declares; `null` = unscoped (any public host).
    declared_domains: Option<Vec<String>>,
    /// Domains the update adds over the installed version (`["*"]` when it
    /// drops its allowlist). Present only when the update widens access.
    added_domains: Option<Vec<String>>,
    /// The update is never auto-applied; it must be confirmed by posting
    /// `{"approve_permissions": true}`.
    requires_approval: bool,
}

#[derive(Serialize)]
struct MarketplaceEntry {
    id: String,
    name: String,
    version: String,
    description: String,
    author: String,
    tags: Vec<String>,
    installed: bool,
    /// Host-API major the plugin targets.
    api_version: u32,
    /// Domains the plugin may reach; `null` = unscoped (any public host).
    domains: Option<Vec<String>>,
}

/// Optional body for install / single-plugin update. Installing always needs
/// `approve_permissions: true`; updating needs it only when the update widens
/// the plugin's domain set. Without it the server answers 428 with the
/// domains that need confirming.
#[derive(Deserialize, Default)]
struct ApprovalRequest {
    #[serde(default)]
    approve_permissions: bool,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

// --- Handlers ---

/// The core part of `GET /updates/check` in desktop mode (AC-17).
async fn desktop_core_info(
    updater: &dyn DesktopUpdater,
) -> Result<CoreUpdateInfo, (StatusCode, Json<ErrorResponse>)> {
    let found = updater
        .check()
        .await
        .map_err(|error| (StatusCode::BAD_GATEWAY, Json(ErrorResponse { error })))?;
    let mode = updater.mode_state();
    Ok(CoreUpdateInfo {
        current_version: updater::CURRENT_VERSION.to_string(),
        update_available: found.is_some(),
        latest_version: found.as_ref().map(|f| f.version.clone()),
        release_notes: found.and_then(|f| f.notes),
        distribution: "tauri".into(),
        can_self_update: updater.can_self_update(),
        pending_restart: Some(updater.pending_restart()),
        update_mode: Some(mode.update_mode),
        update_mode_chosen: Some(mode.update_mode_chosen),
    })
}

async fn check_updates(
    State(state): State<AppState>,
) -> Result<Json<CheckUpdatesResponse>, (StatusCode, Json<ErrorResponse>)> {
    if let Some(desktop) = state.desktop_updater.clone() {
        let core = desktop_core_info(desktop.as_ref()).await?;
        let plugins = plugin_update_entries(&state).await;
        return Ok(Json(CheckUpdatesResponse { core, plugins }));
    }

    // Check core update
    let core_status = updater::check_for_update(
        &state.http_client,
        &state.coordinator.config().await.update.github_repo,
        updater::Product::Server,
    )
    .await;

    let core = match core_status {
        Ok(CoreUpdateStatus::UpdateAvailable {
            current,
            latest,
            release_notes,
            can_self_update,
            ..
        }) => CoreUpdateInfo {
            current_version: current,
            latest_version: Some(latest),
            update_available: true,
            release_notes: Some(release_notes),
            distribution: format!("{:?}", updater::detect_distribution()),
            can_self_update,
            pending_restart: None,
            update_mode: None,
            update_mode_chosen: None,
        },
        _ => CoreUpdateInfo {
            current_version: updater::CURRENT_VERSION.to_string(),
            latest_version: None,
            update_available: false,
            release_notes: None,
            distribution: format!("{:?}", updater::detect_distribution()),
            can_self_update: false,
            pending_restart: None,
            update_mode: None,
            update_mode_chosen: None,
        },
    };

    let plugins = plugin_update_entries(&state).await;
    Ok(Json(CheckUpdatesResponse { core, plugins }))
}

async fn plugin_update_entries(state: &AppState) -> Vec<PluginUpdateEntry> {
    let plugin_updates = state
        .plugin_updater
        .check_updates()
        .await
        .unwrap_or_default();

    plugin_updates
        .into_iter()
        .map(|u| PluginUpdateEntry {
            plugin_id: u.plugin_id,
            current_version: u.current_version,
            available_version: u.available_version,
            is_new: u.is_new,
            declared_domains: u.declared_domains,
            added_domains: u.added_domains,
            requires_approval: u.requires_approval,
        })
        .collect()
}

async fn apply_core_update(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<ErrorResponse>)> {
    // Desktop mode: check, download, verify and install (or hold for Quit
    // on Windows) through the desktop updater, whatever the choice (AC-17).
    if let Some(desktop) = state.desktop_updater.clone() {
        if !desktop.can_self_update() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "This copy cannot update itself; update it with your package manager"
                        .into(),
                }),
            ));
        }
        return match desktop.apply().await {
            Ok(version) => Ok((
                StatusCode::OK,
                Json(serde_json::json!({ "status": "ready", "version": version })),
            )),
            Err(error) => Err((StatusCode::BAD_GATEWAY, Json(ErrorResponse { error }))),
        };
    }
    let status = updater::check_for_update(
        &state.http_client,
        &state.coordinator.config().await.update.github_repo,
        updater::Product::Server,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    match status {
        CoreUpdateStatus::UpToDate => Ok((
            StatusCode::OK,
            Json(serde_json::json!({"message": "Already up to date"})),
        )),
        CoreUpdateStatus::UpdateAvailable {
            latest,
            can_self_update,
            download_url,
            sha256_url,
            sig_url,
            ..
        } => {
            if !can_self_update {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Self-update not supported for this distribution".into(),
                    }),
                ));
            }

            // Spawn background task for download + apply. The announced version
            // is passed through so the apply path re-checks it's an upgrade.
            let client = state.http_client.clone();
            let ver = latest.clone();
            tokio::spawn(async move {
                match updater::download_and_apply(
                    &client,
                    &download_url,
                    sha256_url.as_deref(),
                    sig_url.as_deref(),
                    &ver,
                    updater::Product::Server,
                )
                .await
                {
                    Ok(()) => tracing::info!("Core update to v{ver} applied — restart needed"),
                    Err(e) => tracing::error!("Core update failed: {e}"),
                }
            });

            Ok((
                StatusCode::ACCEPTED,
                Json(serde_json::json!({
                    "message": format!("Update to v{latest} initiated. Restart required after completion."),
                    "version": latest,
                })),
            ))
        }
    }
}

async fn list_available_plugins(
    State(state): State<AppState>,
) -> Result<Json<Vec<MarketplaceEntry>>, (StatusCode, Json<ErrorResponse>)> {
    let available = state.plugin_updater.list_available().await.map_err(|e| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    let installed = state.plugins.list_plugins().await;
    let installed_ids: Vec<String> = installed.iter().map(|p| p.id.clone()).collect();

    let entries = available
        .into_iter()
        .map(|p| MarketplaceEntry {
            installed: installed_ids.contains(&p.id),
            api_version: p.required_api_version(),
            domains: p.declared_domains().map(<[String]>::to_vec),
            id: p.id,
            name: p.name,
            version: p.version,
            description: p.description,
            author: p.author,
            tags: p.tags,
        })
        .collect();

    Ok(Json(entries))
}

/// Map a plugin-runtime error to an HTTP status. NotFound and registry
/// failures are user-actionable; checksum/version issues are integrity
/// problems; everything else is treated as an internal failure.
fn plugin_error_status(err: &krake_plugin_runtime::Error) -> StatusCode {
    match err {
        krake_plugin_runtime::Error::NotFound(_) => StatusCode::NOT_FOUND,
        krake_plugin_runtime::Error::RegistryUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
        krake_plugin_runtime::Error::ChecksumMismatch(_) => StatusCode::BAD_GATEWAY,
        krake_plugin_runtime::Error::IncompatibleVersion { .. } => StatusCode::CONFLICT,
        krake_plugin_runtime::Error::PermissionApprovalRequired(_) => {
            StatusCode::PRECONDITION_REQUIRED
        }
        krake_plugin_runtime::Error::SandboxViolation(_) => StatusCode::BAD_GATEWAY,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn update_all_plugins(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.plugin_updater.update_all_plugins().await {
        Ok(updated) => {
            let entries: Vec<serde_json::Value> = updated
                .into_iter()
                .map(|m| serde_json::json!({"id": m.id, "version": m.version}))
                .collect();
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "updated": entries.len(),
                    "plugins": entries,
                })),
            )
        }
        Err(e) => (
            plugin_error_status(&e),
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn update_plugin(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<ApprovalRequest>>,
) -> (StatusCode, Json<serde_json::Value>) {
    let approved = body.map(|Json(b)| b.approve_permissions).unwrap_or(false);
    match state.plugin_updater.update_plugin(&id, approved).await {
        Ok(meta) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "id": meta.id,
                "version": meta.version,
            })),
        ),
        Err(e) => (
            plugin_error_status(&e),
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn install_plugin(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<ApprovalRequest>>,
) -> (StatusCode, Json<serde_json::Value>) {
    let approved = body.map(|Json(b)| b.approve_permissions).unwrap_or(false);
    match state.plugin_updater.install_plugin(&id, approved).await {
        Ok(meta) => (
            StatusCode::CREATED,
            Json(serde_json::json!({
                "id": meta.id,
                "version": meta.version,
            })),
        ),
        Err(e) => (
            plugin_error_status(&e),
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

// --- Desktop update choice (AC-31, AC-34, AC-35) ---

type DesktopResult = Result<Json<DesktopModeState>, (StatusCode, Json<ErrorResponse>)>;

fn not_desktop() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: "only in the desktop app".into(),
        }),
    )
}

fn bad_body(error: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: error.into(),
        }),
    )
}

/// `GET /updates/mode`: the update-choice state, without network access.
async fn get_update_mode(State(state): State<AppState>) -> DesktopResult {
    let desktop = state.desktop_updater.ok_or_else(not_desktop)?;
    Ok(Json(desktop.mode_state()))
}

/// `PUT /updates/mode`: `{"mode": …}` makes a choice, `{"dismissed": true}`
/// records a close of the first-start question without one.
async fn put_update_mode(
    State(state): State<AppState>,
    body: Result<Json<serde_json::Value>, axum::extract::rejection::JsonRejection>,
) -> DesktopResult {
    let desktop = state.desktop_updater.ok_or_else(not_desktop)?;
    let Json(value) = body.map_err(|_| bad_body("expected a JSON object"))?;
    let object = value
        .as_object()
        .filter(|o| o.len() == 1)
        .ok_or_else(|| bad_body(r#"expected {"mode": …} or {"dismissed": true}"#))?;
    if let Some(mode) = object.get("mode") {
        let mode: UpdateMode = serde_json::from_value(mode.clone())
            .map_err(|_| bad_body(r#"mode must be "auto", "notify" or "off""#))?;
        return Ok(Json(desktop.set_mode(mode)));
    }
    if object.get("dismissed") == Some(&serde_json::Value::Bool(true)) {
        desktop.dismiss_question();
        return Ok(Json(desktop.mode_state()));
    }
    Err(bad_body(r#"expected {"mode": …} or {"dismissed": true}"#))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AnnouncedRequest {
    version: String,
}

/// `POST /updates/announced`: mark the pending announcement as shown.
async fn post_announced(
    State(state): State<AppState>,
    body: Result<Json<AnnouncedRequest>, axum::extract::rejection::JsonRejection>,
) -> DesktopResult {
    let desktop = state.desktop_updater.ok_or_else(not_desktop)?;
    let Json(request) = body.map_err(|_| bad_body(r#"expected {"version": "x.y.z"}"#))?;
    desktop
        .acknowledge(&request.version)
        .map_err(|e| bad_body(&e))?;
    Ok(Json(desktop.mode_state()))
}
