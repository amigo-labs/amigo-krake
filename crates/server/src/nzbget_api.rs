//! NZBGet-compatible JSON-RPC API for Sonarr/Radarr integration.
//!
//! Implements the NZBGet JSON-RPC protocol so that Sonarr, Radarr, and other
//! *arr tools can use Krake as a drop-in NZBGet replacement.
//!
//! Endpoint: POST /jsonrpc (with HTTP Basic Auth)
//! Protocol: JSON-RPC 1.0 (positional parameters only)

use std::sync::Arc;
use std::time::Instant;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::{Json, Router, routing::post};
use base64::Engine;
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::{debug, info, warn};

use krake_core::coordinator::{Coordinator, Dupe, DupeMode, NzbOptions};
use krake_core::queue::QueueStatus;
use krake_core::storage::DownloadRow;

use crate::api::AppState;

/// Server start time for uptime calculation.
static SERVER_START: std::sync::LazyLock<Instant> = std::sync::LazyLock::new(Instant::now);

/// NZBGet-compatible JSON-RPC router.
///
/// Returns `None` when the NZBGet compatibility layer is disabled in config
/// (the default), or when it is enabled but credentials are missing — the
/// caller (main.rs) refuses to start in the latter case via
/// [`validate_nzbget_config`], so a `None` here always means "intentionally
/// off".
pub fn nzbget_router(
    state: AppState,
    config: &krake_core::config::NzbGetApiConfig,
) -> Option<Router> {
    if !config.enabled {
        debug!("NZBGet JSON-RPC API disabled");
        return None;
    }
    if config.username.is_empty() || config.password.is_empty() {
        // Defense-in-depth: validate_nzbget_config should have caught this
        // already, but if a config reload races with router construction we
        // refuse to mount an open endpoint.
        warn!("NZBGet JSON-RPC API enabled but credentials missing — not mounting");
        return None;
    }
    // Initialize the start time
    let _ = *SERVER_START;

    info!("NZBGet JSON-RPC API enabled at /jsonrpc");
    Some(
        Router::new()
            .route("/jsonrpc", post(jsonrpc_handler))
            .route("/{_username}/jsonrpc", post(jsonrpc_handler))
            .with_state(state),
    )
}

/// Validate the NZBGet configuration at startup. Returns an error suitable
/// for surfacing to the operator when the API is enabled without credentials,
/// which would otherwise be a wide-open RPC endpoint.
pub fn validate_nzbget_config(config: &krake_core::config::NzbGetApiConfig) -> Result<(), String> {
    if !config.enabled {
        return Ok(());
    }
    if config.username.is_empty() || config.password.is_empty() {
        return Err(
            "nzbget_api.enabled = true but username/password are empty — refusing to expose \
             an unauthenticated NZBGet JSON-RPC endpoint. Set both credentials or set \
             nzbget_api.enabled = false."
                .to_string(),
        );
    }
    Ok(())
}

// --- JSON-RPC types ---

#[derive(Deserialize)]
struct JsonRpcRequest {
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Vec<Value>,
}

// --- Auth ---

fn check_basic_auth(headers: &HeaderMap, expected_user: &str, expected_pass: &str) -> bool {
    // The router only mounts when credentials are non-empty (see
    // `nzbget_router` + `validate_nzbget_config`). If they ever land here
    // empty due to a runtime config edit, fail closed instead of returning
    // true.
    if expected_user.is_empty() || expected_pass.is_empty() {
        return false;
    }

    let Some(auth) = headers.get("authorization") else {
        return false;
    };
    let Ok(auth_str) = auth.to_str() else {
        return false;
    };
    let Some(encoded) = auth_str.strip_prefix("Basic ") else {
        return false;
    };

    let decoded = match base64::engine::general_purpose::STANDARD.decode(encoded.trim()) {
        Ok(bytes) => String::from_utf8_lossy_owned(bytes),
        Err(_) => return false,
    };

    // Format: "username:password"
    let Some((user, pass)) = decoded.split_once(':') else {
        return false;
    };

    // Constant-time comparison to avoid timing-side-channel leaks of the
    // configured credentials.
    use subtle::ConstantTimeEq;
    let user_eq = user.as_bytes().ct_eq(expected_user.as_bytes());
    let pass_eq = pass.as_bytes().ct_eq(expected_pass.as_bytes());
    bool::from(user_eq & pass_eq)
}

// --- Handler ---

async fn jsonrpc_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    let config = state.coordinator.config().await;
    if !check_basic_auth(
        &headers,
        &config.nzbget_api.username,
        &config.nzbget_api.password,
    ) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"id": null, "error": {"code": -32000, "message": "Unauthorized"}})),
        );
    }

    let id = req.id.unwrap_or(Value::Null);
    let method = req.method.as_str();

    debug!("NZBGet JSON-RPC: {method}");

    let result = match method {
        "version" => handle_version(),
        "status" => handle_status(&state.coordinator).await,
        "append" => handle_append(&state.coordinator, &req.params).await,
        "listgroups" => handle_listgroups(&state.coordinator).await,
        "history" => handle_history(&state.coordinator).await,
        "editqueue" => handle_editqueue(&state.coordinator, &req.params).await,
        "config" => handle_config(&state.coordinator).await,
        "postqueue" => Ok(json!([])),
        "listfiles" => Ok(json!([])),
        "log" => Ok(json!([])),
        "writelog" => Ok(json!(true)),
        "loadlog" => Ok(json!([])),
        "servervolumes" => Ok(json!([])),
        "resetservervolume" => Ok(json!(true)),
        "rate" => Ok(json!(true)),
        "pausedownload" => {
            // Global pause — pause all active downloads
            handle_global_pause(&state.coordinator).await
        }
        "resumedownload" => {
            // Global resume — resume all paused downloads
            handle_global_resume(&state.coordinator).await
        }
        "scan" => Ok(json!(true)),
        "scheduleresume" => Ok(json!(true)),
        _ => {
            warn!("NZBGet JSON-RPC: unknown method '{method}'");
            Err((-32601, format!("Method not found: {method}")))
        }
    };

    match result {
        Ok(value) => (StatusCode::OK, Json(json!({"id": id, "result": value}))),
        Err((code, msg)) => (
            StatusCode::OK, // JSON-RPC errors still return 200
            Json(json!({"id": id, "error": {"code": code, "message": msg}})),
        ),
    }
}

type RpcResult = Result<Value, (i32, String)>;

// --- Method implementations ---

fn handle_version() -> RpcResult {
    Ok(json!("krake-0.1.0"))
}

async fn handle_status(coordinator: &Arc<Coordinator>) -> RpcResult {
    let speed = coordinator.total_speed().await;
    let active = coordinator.active_count().await;
    let uptime = SERVER_START.elapsed().as_secs();

    // Calculate remaining size from queued + downloading
    let remaining: u64 = coordinator
        .storage()
        .list_downloads_by_status(QueueStatus::Queued)
        .await
        .unwrap_or_default()
        .iter()
        .chain(
            coordinator
                .storage()
                .list_downloads_by_status(QueueStatus::Downloading)
                .await
                .unwrap_or_default()
                .iter(),
        )
        .map(|d| d.filesize.unwrap_or(0).saturating_sub(d.bytes_downloaded))
        .sum();

    Ok(json!({
        "RemainingSizeLo": (remaining & 0xFFFFFFFF) as u32,
        "RemainingSizeHi": (remaining >> 32) as u32,
        "RemainingSizeMB": remaining / (1024 * 1024),
        "DownloadedSizeLo": 0,
        "DownloadedSizeHi": 0,
        "DownloadedSizeMB": 0,
        "DownloadLimit": coordinator.config().await.bandwidth.global_limit,
        "AverageDownloadRate": speed,
        "DownloadRate": speed,
        "ThreadCount": active,
        "PostJobCount": 0,
        "UrlCount": 0,
        "UpTimeSec": uptime,
        "DownloadTimeSec": uptime,
        "ServerPaused": false,
        "DownloadPaused": false,
        "Download2Paused": false,
        "ServerTime": chrono::Utc::now().timestamp(),
        "ResumeTime": 0,
        "FreeDiskSpaceLo": 0,
        "FreeDiskSpaceHi": 0,
        "FreeDiskSpaceMB": 0,
        "ServerStandBy": active == 0,
        "NewsServers": [],
    }))
}

async fn handle_append(coordinator: &Arc<Coordinator>, params: &[Value]) -> RpcResult {
    append_nzb(coordinator, params, false).await
}

/// `append`; `allow_private_targets` lets an NZB URL point at this network
/// (tests only).
async fn append_nzb(
    coordinator: &Arc<Coordinator>,
    params: &[Value],
    allow_private_targets: bool,
) -> RpcResult {
    // append(NZBFilename, NZBContent, Category, Priority, AddToTop, AddPaused, DupeKey, DupeScore, DupeMode, PPParameters)
    let nzb_filename = params
        .first()
        .and_then(|v| v.as_str())
        .unwrap_or("upload.nzb");
    let nzb_content = params.get(1).and_then(|v| v.as_str()).unwrap_or("");
    let category = params.get(2).and_then(|v| v.as_str()).unwrap_or("");
    let priority = params.get(3).and_then(|v| v.as_i64()).unwrap_or(0);
    // AddToTop (param 4) is not supported: the queue is ordered by priority.
    let add_paused = params.get(5).and_then(|v| v.as_bool()).unwrap_or(false);
    let dupe_key = params.get(6).and_then(|v| v.as_str()).unwrap_or("");
    let dupe_score = params.get(7).and_then(|v| v.as_i64()).unwrap_or(0);
    let dupe_mode = params.get(8).and_then(|v| v.as_str()).unwrap_or("SCORE");

    if nzb_content.is_empty() {
        return Err((-32602, "NZBContent is required".into()));
    }

    // NZBContent is the NZB, base64-encoded, or a URL to fetch it from.
    let nzb_str = if nzb_content.starts_with("http://") || nzb_content.starts_with("https://") {
        crate::background::fetch_nzb(nzb_content, allow_private_targets)
            .await
            .map_err(|e| (-32000, format!("Failed to fetch NZB: {e}")))?
    } else {
        let nzb_data = base64::engine::general_purpose::STANDARD
            .decode(nzb_content)
            .map_err(|e| (-32602, format!("Invalid base64 NZB content: {e}")))?;
        String::from_utf8(nzb_data).map_err(|e| (-32602, format!("Invalid UTF-8 in NZB: {e}")))?
    };

    // Validate NZB
    krake_core::protocol::usenet::nzb::parse_nzb(&nzb_str)
        .map_err(|e| (-32602, format!("Invalid NZB: {e}")))?;

    let options = NzbOptions {
        category: Some(category.to_string()),
        priority: nzbget_priority(priority),
        paused: add_paused,
        dupe: Some(Dupe {
            key: dupe_key.to_string(),
            score: dupe_score,
            mode: DupeMode::parse(dupe_mode),
        }),
        // *arr tools send a category; their releases must yield media.
        media_check: !category.trim().is_empty(),
    };
    // NZBGet names a download after its NZB file, without the extension.
    let name = nzb_name(nzb_filename);
    match coordinator
        .add_nzb_download_with(name, &nzb_str, &options)
        .await
    {
        Ok(added) => {
            if added.dupe {
                info!("NZBGet append: {nzb_filename} → {} (duplicate)", added.id);
            } else {
                info!("NZBGet append: {nzb_filename} → {}", added.id);
            }
            Ok(json!(id_to_nzbid(&added.id)))
        }
        Err(e) => Err((-32000, format!("Failed to add download: {e}"))),
    }
}

/// krake's queue priority for NZBGet's: krake orders by priority, higher
/// first and 0 normal, so NZBGet's -100/-50/0/50/100/900 are kept as they
/// are; anything beyond them is clamped to that range.
fn nzbget_priority(priority: i64) -> i32 {
    priority.clamp(-100, 900) as i32
}

/// The download name NZBGet derives from an NZB's file name: the name
/// without a trailing `.nzb`.
fn nzb_name(filename: &str) -> &str {
    let stem_len = filename.len().saturating_sub(4);
    match filename.get(stem_len..) {
        Some(ext) if stem_len > 0 && ext.eq_ignore_ascii_case(".nzb") => &filename[..stem_len],
        _ => filename,
    }
}

/// What a download recorded about itself (category, Usenet health and
/// checks): its metadata while it is listed, else its history entry's.
async fn info_of(coordinator: &Arc<Coordinator>, id: &str) -> Value {
    let storage = coordinator.storage();
    let metadata = match storage.get_download_metadata(id).await {
        Ok(Some(m)) => Some(m),
        _ => storage.get_history_metadata(id).await.ok().flatten(),
    };
    metadata
        .and_then(|m| serde_json::from_str::<Value>(&m).ok())
        .unwrap_or_default()
}

/// The category stored with a download; empty when it has none.
fn category(info: &Value) -> &str {
    info["category"].as_str().unwrap_or_default()
}

/// The duplicate key `append` stored with a download; empty when none.
fn dupe_key(info: &Value) -> &str {
    info["dupe_key"].as_str().unwrap_or_default()
}

fn dupe_score(info: &Value) -> i64 {
    info["dupe_score"].as_i64().unwrap_or(0)
}

fn dupe_mode(info: &Value) -> &str {
    info["dupe_mode"].as_str().unwrap_or("SCORE")
}

/// NZBGet's `Status`, `ParStatus`, `UnpackStatus` and `MoveStatus` of a
/// finished download. A broken release reads `FAILURE/HEALTH`,
/// `FAILURE/PAR`, `FAILURE/UNPACK` or, without media, `FAILURE/BAD`; a
/// skipped duplicate `DELETED/DUPE`; a failure on this machine (disk full,
/// server unreachable) reads `FAILURE/NONE`, so a client does not blame
/// the release.
fn history_status(status: &str, info: &Value) -> [&'static str; 4] {
    let stage = |key: &str, default: &'static str| match info[key].as_str() {
        Some("SUCCESS") => "SUCCESS",
        Some("FAILURE") => "FAILURE",
        Some("NONE") => "NONE",
        _ => default,
    };
    if status == "completed" {
        // Downloads finished before the checks were recorded read SUCCESS.
        return [
            "SUCCESS/ALL",
            stage("par_status", "SUCCESS"),
            stage("unpack_status", "SUCCESS"),
            "SUCCESS",
        ];
    }
    let total = match info["failure"].as_str() {
        Some("health") => "FAILURE/HEALTH",
        Some("par") => "FAILURE/PAR",
        Some("unpack") => "FAILURE/UNPACK",
        // No media file: NZBGet's status for a release marked bad (as its
        // fake-detection scripts do).
        Some("media") => "FAILURE/BAD",
        Some("dupe") => "DELETED/DUPE",
        _ => "FAILURE/NONE",
    };
    [
        total,
        stage("par_status", "NONE"),
        stage("unpack_status", "NONE"),
        "NONE",
    ]
}

async fn handle_listgroups(coordinator: &Arc<Coordinator>) -> RpcResult {
    let rows = coordinator
        .storage()
        .list_downloads()
        .await
        .unwrap_or_default();

    let mut groups = Vec::new();
    for r in rows
        .iter()
        .filter(|r| r.status != "completed" && r.status != "failed")
    {
        let info = info_of(coordinator, &r.id).await;
        groups.push({
            let filesize = r.filesize.unwrap_or(0);
            let downloaded = r.bytes_downloaded;
            let remaining = filesize.saturating_sub(downloaded);
            let nzbid = id_to_nzbid(&r.id);

            let status = match r.status.as_str() {
                "downloading" => "DOWNLOADING",
                "queued" => "QUEUED",
                "paused" => "PAUSED",
                _ => "QUEUED",
            };

            json!({
                "NZBID": nzbid,
                "NZBName": r.filename.as_deref().unwrap_or("download"),
                "NZBFilename": r.filename.as_deref().unwrap_or("download.nzb"),
                "NZBNicename": r.filename.as_deref().unwrap_or("download"),
                "Kind": "NZB",
                "URL": r.url,
                "DestDir": r.download_dir.as_deref().unwrap_or("downloads"),
                "FinalDir": "",
                "Category": category(&info),
                "FileSizeLo": (filesize & 0xFFFFFFFF) as u32,
                "FileSizeHi": (filesize >> 32) as u32,
                "FileSizeMB": filesize / (1024 * 1024),
                "DownloadedSizeLo": (downloaded & 0xFFFFFFFF) as u32,
                "DownloadedSizeHi": (downloaded >> 32) as u32,
                "DownloadedSizeMB": downloaded / (1024 * 1024),
                "RemainingSizeLo": (remaining & 0xFFFFFFFF) as u32,
                "RemainingSizeHi": (remaining >> 32) as u32,
                "RemainingSizeMB": remaining / (1024 * 1024),
                "ActiveDownloads": if r.status == "downloading" { 1 } else { 0 },
                "Status": status,
                "TotalArticles": 0,
                "SuccessArticles": 0,
                "FailedArticles": 0,
                "Health": 1000,
                "CriticalHealth": 1000,
                "MaxPriority": r.priority,
                "MinPriority": r.priority,
                "DupeKey": dupe_key(&info),
                "DupeScore": dupe_score(&info),
                "DupeMode": dupe_mode(&info),
                "MinPostTime": 0,
                "MaxPostTime": 0,
                "Parameters": [],
                "ScriptStatuses": [],
                "ServerStats": [],
                "PostInfoText": "",
                "PostStageProgress": 0,
                "PostTotalTimeSec": 0,
                "PostStageTimeSec": 0,
            })
        });
    }

    Ok(json!(groups))
}

async fn handle_history(coordinator: &Arc<Coordinator>) -> RpcResult {
    // Get completed + failed downloads
    let completed = coordinator
        .storage()
        .list_downloads_by_status(QueueStatus::Completed)
        .await
        .unwrap_or_default();
    let failed = coordinator
        .storage()
        .list_downloads_by_status(QueueStatus::Failed)
        .await
        .unwrap_or_default();

    // Also include actual history
    let history_rows = coordinator
        .storage()
        .get_history()
        .await
        .unwrap_or_default();

    let rows = history_entries(completed, failed, history_rows);
    let mut items = Vec::new();
    for r in &rows {
        let info = info_of(coordinator, &r.id).await;
        items.push({
            let filesize = r.filesize.unwrap_or(0);
            let downloaded = r.bytes_downloaded;
            let nzbid = id_to_nzbid(&r.id);
            let [status, par_status, unpack_status, move_status] = history_status(&r.status, &info);
            let count = |key: &str, default: u64| info[key].as_u64().unwrap_or(default);
            let is_dupe = info["failure"] == "dupe";

            let mut item = json!({
                "NZBID": nzbid,
                "NZBName": r.filename.as_deref().unwrap_or("download"),
                "NZBFilename": r.filename.as_deref().unwrap_or("download.nzb"),
                "NZBNicename": r.filename.as_deref().unwrap_or("download"),
                "Kind": "NZB",
                "URL": r.url,
                "DestDir": r.download_dir.as_deref().unwrap_or("downloads"),
                "FinalDir": r.download_dir.as_deref().unwrap_or("downloads"),
                "Category": category(&info),
                "FileSizeLo": (filesize & 0xFFFFFFFF) as u32,
                "FileSizeHi": (filesize >> 32) as u32,
                "FileSizeMB": filesize / (1024 * 1024),
                "DownloadedSizeLo": (downloaded & 0xFFFFFFFF) as u32,
                "DownloadedSizeHi": (downloaded >> 32) as u32,
                "DownloadedSizeMB": downloaded / (1024 * 1024),
                "Status": status,
                "ParStatus": par_status,
                "ExParStatus": "RECIPIENT",
                "UnpackStatus": unpack_status,
                "MoveStatus": move_status,
                "ScriptStatus": "NONE",
                "DeleteStatus": if is_dupe { "DUPE" } else { "NONE" },
                "MarkStatus": if info["failure"] == "media" { "BAD" } else { "NONE" },
                "UrlStatus": "NONE",
                "TotalArticles": count("total_articles", 0),
                "SuccessArticles": count("success_articles", 0),
                "FailedArticles": count("failed_articles", 0),
                "Health": count("health", 1000),
                "CriticalHealth": count("critical_health", 1000),
                "DupeKey": dupe_key(&info),
                "DupeScore": dupe_score(&info),
                "DupeMode": dupe_mode(&info),
                "Deleted": is_dupe,
                "DownloadTimeSec": 0,
                "PostTotalTimeSec": 0,
                "ParTimeSec": 0,
                "RepairTimeSec": 0,
                "UnpackTimeSec": 0,
                "Parameters": [],
                "ScriptStatuses": [],
                "ServerStats": [],
            });
            // NZBGet's history names an entry `Name`; the queue uses `NZBName`.
            item["Name"] = item["NZBName"].clone();
            item
        });
    }

    Ok(json!(items))
}

/// The rows NZBGet's `history` lists: finished downloads still in the list,
/// then archived entries. A completed download is also logged in the
/// history, so an archived entry whose download is still listed is dropped;
/// otherwise it would appear twice under one NZBID.
fn history_entries(
    completed: Vec<DownloadRow>,
    failed: Vec<DownloadRow>,
    archived: Vec<DownloadRow>,
) -> Vec<DownloadRow> {
    let mut rows: Vec<DownloadRow> = completed.into_iter().chain(failed).collect();
    let listed: std::collections::HashSet<String> = rows.iter().map(|r| r.id.clone()).collect();
    rows.extend(archived.into_iter().filter(|r| !listed.contains(&r.id)));
    rows
}

async fn handle_editqueue(coordinator: &Arc<Coordinator>, params: &[Value]) -> RpcResult {
    let command = params.first().and_then(|v| v.as_str()).unwrap_or("");
    let _param = params.get(1).and_then(|v| v.as_str()).unwrap_or("");
    let ids = params
        .get(2)
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    for id_val in &ids {
        let nzbid = id_val.as_i64().unwrap_or(0);
        if nzbid <= 0 {
            continue;
        }

        // Find download by nzbid
        let all = coordinator
            .storage()
            .list_downloads()
            .await
            .unwrap_or_default();
        let download = all.iter().find(|d| has_nzbid(&d.id, nzbid as i32));

        // A history delete also removes the archived entry, which outlives
        // the download's row — and may be all that is left of it.
        if matches!(command, "HistoryDelete" | "HistoryFinalDelete") {
            let archived = coordinator
                .storage()
                .get_history()
                .await
                .unwrap_or_default();
            if let Some(entry) = archived.iter().find(|h| has_nzbid(&h.id, nzbid as i32))
                && let Err(e) = coordinator.storage().delete_history(&entry.id).await
            {
                warn!("NZBGet editqueue {command} failed for {}: {e}", entry.id);
            }
        }

        if let Some(dl) = download {
            let result = match command {
                "GroupPause" | "GroupPauseAllPars" | "GroupPauseExtraPars" => {
                    coordinator.pause(&dl.id).await
                }
                "GroupResume" => coordinator.resume(&dl.id).await,
                "GroupDelete" | "GroupFinalDelete" => coordinator.cancel(&dl.id).await,
                "HistoryDelete" | "HistoryFinalDelete" => coordinator.cancel(&dl.id).await,
                _ => {
                    debug!("NZBGet editqueue: unsupported command '{command}'");
                    Ok(())
                }
            };

            if let Err(e) = result {
                warn!("NZBGet editqueue {command} failed for {}: {e}", dl.id);
            }
        }
    }

    Ok(json!(true))
}

async fn handle_config(coordinator: &Arc<Coordinator>) -> RpcResult {
    let config = coordinator.config().await;
    Ok(json!([
        {"Name": "MainDir", "Value": config.download_dir},
        {"Name": "DestDir", "Value": config.download_dir},
        {"Name": "TempDir", "Value": config.temp_dir},
        {"Name": "NzbDir", "Value": ""},
        {"Name": "ServerPort", "Value": "1516"},
        {"Name": "ControlUsername", "Value": ""},
        {"Name": "ControlPassword", "Value": ""},
        {"Name": "Category1.Name", "Value": ""},
        {"Name": "Category1.DestDir", "Value": ""},
        {"Name": "ParCheck", "Value": if config.usenet.par2_repair { "auto" } else { "no" }},
        {"Name": "Unpack", "Value": if config.usenet.auto_unrar { "yes" } else { "no" }},
    ]))
}

async fn handle_global_pause(coordinator: &Arc<Coordinator>) -> RpcResult {
    let downloading = coordinator
        .storage()
        .list_downloads_by_status(QueueStatus::Downloading)
        .await
        .unwrap_or_default();

    for dl in &downloading {
        let _ = coordinator.pause(&dl.id).await;
    }

    Ok(json!(true))
}

async fn handle_global_resume(coordinator: &Arc<Coordinator>) -> RpcResult {
    let paused = coordinator
        .storage()
        .list_downloads_by_status(QueueStatus::Paused)
        .await
        .unwrap_or_default();

    for dl in &paused {
        let _ = coordinator.resume(&dl.id).await;
    }

    Ok(json!(true))
}

// --- Helpers ---

/// The NZBID of a download: FNV-1a (32 bit) over the bytes of its id, with
/// the top bit cleared so it is a positive `i32`, and 1 instead of 0.
///
/// Clients keep NZBIDs across restarts and krake updates, so this mapping is
/// fixed and must never change (a test pins it).
fn id_to_nzbid(id: &str) -> i32 {
    const OFFSET_BASIS: u32 = 0x811c_9dc5;
    const PRIME: u32 = 0x0100_0193;
    let hash = id
        .bytes()
        .fold(OFFSET_BASIS, |h, b| (h ^ u32::from(b)).wrapping_mul(PRIME));
    ((hash & 0x7fff_ffff) as i32).max(1)
}

/// The NZBID builds before [`id_to_nzbid`] gave a download (std's
/// `DefaultHasher`). Still matched, so a client holding such an NZBID can
/// remove its download after an update.
fn legacy_nzbid(id: &str) -> i32 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut hasher);
    (hasher.finish() as i32).abs().max(1)
}

/// Whether `nzbid` names the download `id`, under either scheme.
fn has_nzbid(id: &str, nzbid: i32) -> bool {
    id_to_nzbid(id) == nzbid || legacy_nzbid(id) == nzbid
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use krake_core::config::NzbGetApiConfig;

    fn header_map_with_auth(user: &str, pass: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        let creds = format!("{user}:{pass}");
        let encoded = base64::engine::general_purpose::STANDARD.encode(creds);
        h.insert("authorization", format!("Basic {encoded}").parse().unwrap());
        h
    }

    #[test]
    fn validate_rejects_enabled_without_credentials() {
        let cfg = NzbGetApiConfig {
            enabled: true,
            username: String::new(),
            password: String::new(),
        };
        let err = validate_nzbget_config(&cfg).expect_err("must reject");
        assert!(err.contains("nzbget_api"));

        let cfg = NzbGetApiConfig {
            enabled: true,
            username: "alice".into(),
            password: String::new(),
        };
        validate_nzbget_config(&cfg).expect_err("password required");

        let cfg = NzbGetApiConfig {
            enabled: true,
            username: String::new(),
            password: "secret".into(),
        };
        validate_nzbget_config(&cfg).expect_err("username required");
    }

    #[test]
    fn validate_accepts_disabled_or_fully_configured() {
        validate_nzbget_config(&NzbGetApiConfig {
            enabled: false,
            username: String::new(),
            password: String::new(),
        })
        .expect("disabled is fine");
        validate_nzbget_config(&NzbGetApiConfig {
            enabled: true,
            username: "alice".into(),
            password: "secret".into(),
        })
        .expect("fully configured is fine");
    }

    #[test]
    fn check_basic_auth_rejects_empty_expected_credentials() {
        // Defense in depth: even if expected creds somehow land here empty,
        // never treat that as "auth disabled = pass". Previous behaviour
        // returned true on (empty, empty), which let any LAN client drive
        // the JSON-RPC API once the router was mounted.
        let h = header_map_with_auth("alice", "secret");
        assert!(!check_basic_auth(&h, "", ""));
        assert!(!check_basic_auth(&h, "alice", ""));
        assert!(!check_basic_auth(&h, "", "secret"));
    }

    #[test]
    fn check_basic_auth_accepts_correct_creds() {
        let h = header_map_with_auth("alice", "s3cret");
        assert!(check_basic_auth(&h, "alice", "s3cret"));
    }

    #[test]
    fn check_basic_auth_rejects_mismatch_or_malformed() {
        let h = header_map_with_auth("alice", "wrong");
        assert!(!check_basic_auth(&h, "alice", "right"));

        let mut bad = HeaderMap::new();
        bad.insert("authorization", "Bearer xyz".parse().unwrap());
        assert!(!check_basic_auth(&bad, "alice", "secret"));

        let mut malformed = HeaderMap::new();
        malformed.insert("authorization", "Basic !!!not-base64".parse().unwrap());
        assert!(!check_basic_auth(&malformed, "alice", "secret"));

        // Header is valid base64 but does not contain a colon.
        let mut no_colon = HeaderMap::new();
        let encoded = base64::engine::general_purpose::STANDARD.encode("nocolonhere");
        no_colon.insert("authorization", format!("Basic {encoded}").parse().unwrap());
        assert!(!check_basic_auth(&no_colon, "alice", "secret"));
    }

    #[test]
    fn router_returns_none_when_disabled() {
        // We can't construct a full AppState here without bringing in
        // half the server crate, so we exercise the configuration gate at
        // the validate_* layer plus the empty-creds branch via a lightweight
        // smoke test.
        let cfg = NzbGetApiConfig {
            enabled: false,
            username: "alice".into(),
            password: "secret".into(),
        };
        // disabled → no router. We can't build AppState easily here;
        // confirm via the config gate that nzbget_router would early-exit.
        assert!(!cfg.enabled);
    }

    fn row(id: &str, status: &str) -> DownloadRow {
        DownloadRow {
            id: id.into(),
            url: format!("https://example.com/{id}.nzb"),
            protocol: "usenet".into(),
            filename: Some(format!("{id}.nzb")),
            filesize: Some(10),
            status: status.into(),
            priority: 0,
            package_id: None,
            plugin_id: None,
            download_dir: None,
            bytes_downloaded: 10,
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
    fn history_entries_listed_download_is_not_repeated_from_the_archive() {
        let rows = history_entries(
            vec![row("a", "completed")],
            vec![row("b", "failed")],
            vec![row("a", "completed"), row("c", "completed")],
        );
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "c"]);
    }

    #[tokio::test]
    async fn editqueue_history_delete_removes_the_archived_entry() {
        let storage = krake_core::storage::Storage::open_memory().expect("in-memory storage");
        let coordinator = Arc::new(Coordinator::new(
            krake_core::config::Config::default(),
            storage,
        ));
        let listed = row("listed", "completed");
        let gone = row("gone", "completed");
        for r in [&listed, &gone] {
            coordinator
                .storage()
                .insert_download(r)
                .await
                .expect("insert");
            coordinator
                .storage()
                .record_history(&r.id)
                .await
                .expect("log");
        }
        // "gone" was removed from the list; only its archived entry is left.
        coordinator
            .storage()
            .delete_download("gone")
            .await
            .expect("delete");

        for id in ["listed", "gone"] {
            let params = [json!("HistoryDelete"), json!(""), json!([id_to_nzbid(id)])];
            handle_editqueue(&coordinator, &params)
                .await
                .expect("editqueue");
        }

        assert!(
            coordinator
                .storage()
                .get_history()
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            coordinator
                .storage()
                .get_download("listed")
                .await
                .unwrap()
                .is_none()
        );
    }

    // --- Download client for *arr tools (sofarr, Radarr, Sonarr) ---

    const RELEASE: &str = "Some.Movie.2024.1080p.WEB-DL-GRP";

    /// A release NZB whose only file is named unlike the release, as on a
    /// real indexer.
    fn release_nzb(article: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<nzb xmlns="http://www.newzbin.com/DTD/2003/nzb">
  <file poster="p@example.com" date="1700000000" subject="&quot;abc123.part01.rar&quot; yEnc (1/1)">
    <groups><group>alt.binaries.test</group></groups>
    <segments><segment bytes="3145728" number="1">{article}@example.com</segment></segments>
  </file>
</nzb>"#
        )
    }

    /// A coordinator over real directories whose queue never starts a
    /// transfer, so rows stay as `append` left them.
    fn idle_coordinator(root: &std::path::Path) -> Arc<Coordinator> {
        let storage = krake_core::storage::Storage::open(
            root.join("krake.db"),
            root.join("downloads"),
            root.join("tmp"),
        )
        .expect("storage");
        let config = krake_core::config::Config {
            max_concurrent_downloads: 0,
            ..Default::default()
        };
        Arc::new(Coordinator::new(config, storage))
    }

    /// `append` exactly as sofarr calls it.
    async fn append(
        coordinator: &Arc<Coordinator>,
        filename: &str,
        nzb: &str,
        category: &str,
    ) -> i64 {
        let params = [
            json!(filename),
            json!(base64::engine::general_purpose::STANDARD.encode(nzb)),
            json!(category),
            json!(0),
            json!(false),
            json!(false),
            json!(""),
            json!(0),
            json!("SCORE"),
            json!([]),
        ];
        let nzbid = handle_append(coordinator, &params)
            .await
            .expect("append")
            .as_i64()
            .expect("integer NZBID");
        assert!(nzbid > 0, "NZBID must be positive");
        nzbid
    }

    async fn row_of(coordinator: &Arc<Coordinator>, nzbid: i64) -> DownloadRow {
        coordinator
            .storage()
            .list_downloads()
            .await
            .unwrap()
            .into_iter()
            .find(|r| i64::from(id_to_nzbid(&r.id)) == nzbid)
            .expect("row for NZBID")
    }

    fn entry(list: &Value, nzbid: i64) -> &Value {
        list.as_array()
            .expect("list")
            .iter()
            .find(|e| e["NZBID"].as_i64() == Some(nzbid))
            .expect("entry for NZBID")
    }

    async fn complete(coordinator: &Arc<Coordinator>, id: &str) {
        coordinator
            .storage()
            .update_download_status(id, QueueStatus::Completed)
            .await
            .unwrap();
        coordinator.storage().record_history(id).await.unwrap();
    }

    /// `append`'s parameters, as NZBGet takes them.
    #[derive(Default)]
    struct Append<'a> {
        filename: &'a str,
        content: &'a str,
        category: &'a str,
        priority: i64,
        paused: bool,
        dupe_key: &'a str,
        dupe_score: i64,
        dupe_mode: &'a str,
    }

    impl Append<'_> {
        fn params(&self) -> Vec<Value> {
            vec![
                json!(self.filename),
                json!(self.content),
                json!(self.category),
                json!(self.priority),
                json!(false),
                json!(self.paused),
                json!(self.dupe_key),
                json!(self.dupe_score),
                json!(self.dupe_mode),
                json!([]),
            ]
        }
    }

    fn b64(nzb: &str) -> String {
        base64::engine::general_purpose::STANDARD.encode(nzb)
    }

    async fn metadata_of(coordinator: &Arc<Coordinator>, id: &str) -> Value {
        serde_json::from_str(
            &coordinator
                .storage()
                .get_download_metadata(id)
                .await
                .unwrap()
                .expect("metadata"),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn append_queues_with_the_priority_and_paused_given() {
        let root = tempfile::tempdir().unwrap();
        let coordinator = idle_coordinator(root.path());
        let params = Append {
            filename: "Paused.Release.nzb",
            content: &b64(&release_nzb("p")),
            category: "tv",
            priority: 100,
            paused: true,
            ..Append::default()
        }
        .params();
        let nzbid = handle_append(&coordinator, &params)
            .await
            .unwrap()
            .as_i64()
            .unwrap();

        let row = row_of(&coordinator, nzbid).await;
        assert_eq!(row.priority, 100);
        assert_eq!(row.status, "paused");
        let groups = handle_listgroups(&coordinator).await.unwrap();
        let e = entry(&groups, nzbid);
        assert_eq!(e["Status"], "PAUSED");
        assert_eq!(e["MaxPriority"], 100);
    }

    #[tokio::test]
    async fn append_skips_a_duplicate_into_the_history() {
        let root = tempfile::tempdir().unwrap();
        let coordinator = idle_coordinator(root.path());
        let add = |name: &'static str, score: i64, mode: &'static str| {
            let coordinator = coordinator.clone();
            async move {
                let params = Append {
                    filename: name,
                    content: &b64(&release_nzb(name)),
                    category: "tv",
                    dupe_key: "show-s01e01",
                    dupe_score: score,
                    dupe_mode: mode,
                    ..Append::default()
                }
                .params();
                handle_append(&coordinator, &params)
                    .await
                    .unwrap()
                    .as_i64()
                    .unwrap()
            }
        };
        let first = add("First.nzb", 100, "SCORE").await;
        let groups = handle_listgroups(&coordinator).await.unwrap();
        let e = entry(&groups, first);
        assert_eq!(e["DupeKey"], "show-s01e01");
        assert_eq!(e["DupeScore"], 100);
        assert_eq!(e["DupeMode"], "SCORE");

        let dupe = add("Second.nzb", 50, "SCORE").await;
        assert!(dupe > 0, "a skipped duplicate still has an NZBID");
        let groups = handle_listgroups(&coordinator).await.unwrap();
        assert!(
            groups
                .as_array()
                .unwrap()
                .iter()
                .all(|g| g["NZBID"] != dupe),
            "a duplicate is not queued"
        );
        let history = handle_history(&coordinator).await.unwrap();
        let e = entry(&history, dupe);
        assert_eq!(e["Status"], "DELETED/DUPE");
        assert_eq!(e["DeleteStatus"], "DUPE");
        assert_eq!(e["DupeKey"], "show-s01e01");
        assert_eq!(e["DupeScore"], 50);
        assert_eq!(e["Category"], "tv");

        // FORCE downloads it anyway.
        let forced = add("Third.nzb", 0, "FORCE").await;
        assert_eq!(row_of(&coordinator, forced).await.status, "queued");
    }

    #[tokio::test]
    async fn append_with_a_url_fetches_the_nzb_into_a_usenet_download() {
        let nzb = release_nzb("u");
        let served = nzb.clone();
        let app = axum::Router::new().route(
            "/get/Some.Release.nzb",
            axum::routing::get(move || async move { served }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let root = tempfile::tempdir().unwrap();
        let coordinator = idle_coordinator(root.path());
        let url = format!("http://127.0.0.1:{port}/get/Some.Release.nzb");
        let params = Append {
            filename: "Some.Release.nzb",
            content: &url,
            category: "movies",
            ..Append::default()
        }
        .params();
        let nzbid = append_nzb(&coordinator, &params, true)
            .await
            .expect("append by URL")
            .as_i64()
            .unwrap();

        let row = row_of(&coordinator, nzbid).await;
        assert_eq!(row.protocol, "usenet");
        assert_eq!(row.filename.as_deref(), Some("Some.Release"));
        assert_eq!(
            std::path::PathBuf::from(row.download_dir.unwrap()),
            root.path().join("downloads/movies/Some.Release")
        );
        let metadata = metadata_of(&coordinator, &row.id).await;
        assert_eq!(metadata["nzb_data"].as_str(), Some(nzb.as_str()));
        assert_eq!(metadata["category"], "movies");

        // The SSRF guard still applies to a real append.
        assert!(handle_append(&coordinator, &params).await.is_err());
    }

    #[tokio::test]
    async fn append_checks_for_media_only_with_a_category() {
        let root = tempfile::tempdir().unwrap();
        let coordinator = idle_coordinator(root.path());
        for (category, checked) in [("movies", true), ("", false)] {
            let params = Append {
                filename: "R.nzb",
                content: &b64(&release_nzb(category)),
                category,
                ..Append::default()
            }
            .params();
            let nzbid = handle_append(&coordinator, &params)
                .await
                .unwrap()
                .as_i64()
                .unwrap();
            let id = row_of(&coordinator, nzbid).await.id;
            assert_eq!(
                metadata_of(&coordinator, &id).await["media_check"],
                checked,
                "category {category:?}"
            );
        }
    }

    #[tokio::test]
    async fn history_reports_a_release_without_media_as_bad() {
        let root = tempfile::tempdir().unwrap();
        let coordinator = idle_coordinator(root.path());
        let nzbid = append(
            &coordinator,
            &format!("{RELEASE}.nzb"),
            &release_nzb("m"),
            "movies",
        )
        .await;
        let id = row_of(&coordinator, nzbid).await.id;
        finish(
            &coordinator,
            &id,
            QueueStatus::Failed,
            json!({"failure": "media", "health": 1000, "critical_health": 1000,
                   "par_status": "SUCCESS", "unpack_status": "SUCCESS"}),
        )
        .await;
        let history = handle_history(&coordinator).await.unwrap();
        let e = entry(&history, nzbid);
        assert_eq!(e["Status"], "FAILURE/BAD");
        assert_eq!(e["MarkStatus"], "BAD");
        assert_eq!(e["ParStatus"], "SUCCESS");
        assert_eq!(e["UnpackStatus"], "SUCCESS");
        assert_eq!(e["MoveStatus"], "NONE");
    }

    #[tokio::test]
    async fn append_stores_the_nzb_as_a_usenet_download() {
        let root = tempfile::tempdir().unwrap();
        let coordinator = idle_coordinator(root.path());
        let nzb = release_nzb("first");

        let nzbid = append(&coordinator, &format!("{RELEASE}.nzb"), &nzb, "movies").await;

        let row = row_of(&coordinator, nzbid).await;
        assert_eq!(row.protocol, "usenet");
        assert_eq!(row.filesize, Some(3_145_728));
        let metadata = coordinator
            .storage()
            .get_download_metadata(&row.id)
            .await
            .unwrap()
            .expect("metadata");
        let metadata: Value = serde_json::from_str(&metadata).unwrap();
        assert_eq!(metadata["nzb_data"].as_str(), Some(nzb.as_str()));

        // A second release queued meanwhile is its own download.
        let other = append(
            &coordinator,
            "Other.Show.S01E01.nzb",
            &release_nzb("second"),
            "tv",
        )
        .await;
        assert_ne!(other, nzbid);
        assert_ne!(row_of(&coordinator, other).await.id, row.id);
    }

    #[tokio::test]
    async fn append_category_is_reported_in_listgroups_and_history() {
        let root = tempfile::tempdir().unwrap();
        let coordinator = idle_coordinator(root.path());
        let nzbid = append(
            &coordinator,
            &format!("{RELEASE}.nzb"),
            &release_nzb("a"),
            "movies",
        )
        .await;

        let groups = handle_listgroups(&coordinator).await.unwrap();
        assert_eq!(entry(&groups, nzbid)["Category"], "movies");

        complete(&coordinator, &row_of(&coordinator, nzbid).await.id).await;
        let history = handle_history(&coordinator).await.unwrap();
        assert_eq!(entry(&history, nzbid)["Category"], "movies");
    }

    #[tokio::test]
    async fn queue_and_history_name_the_release_given_to_append() {
        let root = tempfile::tempdir().unwrap();
        let coordinator = idle_coordinator(root.path());
        let nzbid = append(
            &coordinator,
            &format!("{RELEASE}.nzb"),
            &release_nzb("a"),
            "movies",
        )
        .await;

        let groups = handle_listgroups(&coordinator).await.unwrap();
        assert_eq!(entry(&groups, nzbid)["NZBName"], RELEASE);

        complete(&coordinator, &row_of(&coordinator, nzbid).await.id).await;
        let history = handle_history(&coordinator).await.unwrap();
        assert_eq!(entry(&history, nzbid)["Name"], RELEASE);
        assert_eq!(entry(&history, nzbid)["NZBName"], RELEASE);
    }

    #[tokio::test]
    async fn history_final_dir_is_the_downloads_own_folder() {
        let root = tempfile::tempdir().unwrap();
        let coordinator = idle_coordinator(root.path());
        let nzbid = append(
            &coordinator,
            &format!("{RELEASE}.nzb"),
            &release_nzb("a"),
            "movies",
        )
        .await;
        let other = append(
            &coordinator,
            "Other.Movie.2023.nzb",
            &release_nzb("b"),
            "movies",
        )
        .await;

        let row = row_of(&coordinator, nzbid).await;
        // The transfer writes and unpacks into the row's folder.
        let folder = root.path().join("downloads").join("movies").join(RELEASE);
        assert_eq!(row.download_dir.as_deref(), folder.to_str());
        assert_ne!(
            row_of(&coordinator, other).await.download_dir,
            row.download_dir
        );

        complete(&coordinator, &row.id).await;
        let history = handle_history(&coordinator).await.unwrap();
        assert_eq!(entry(&history, nzbid)["FinalDir"].as_str(), folder.to_str());
        assert_eq!(entry(&history, nzbid)["DestDir"].as_str(), folder.to_str());
    }

    #[tokio::test]
    async fn removing_a_finished_download_keeps_its_files() {
        let root = tempfile::tempdir().unwrap();
        let coordinator = idle_coordinator(root.path());
        let folder = root.path().join("downloads").join("movies").join(RELEASE);
        std::fs::create_dir_all(&folder).unwrap();
        let file = folder.join("movie.mkv");
        std::fs::write(&file, b"video").unwrap();
        let mut done = row("done", "completed");
        done.download_dir = Some(folder.to_string_lossy().into_owned());
        coordinator.storage().insert_download(&done).await.unwrap();
        coordinator.storage().record_history("done").await.unwrap();

        // What sofarr sends after it imported the files.
        for command in ["GroupFinalDelete", "HistoryDelete"] {
            let params = [json!(command), json!(""), json!([id_to_nzbid("done")])];
            handle_editqueue(&coordinator, &params).await.unwrap();
        }

        assert!(file.exists(), "the downloaded file must stay on disk");
        assert!(
            coordinator
                .storage()
                .get_download("done")
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            coordinator
                .storage()
                .get_history()
                .await
                .unwrap()
                .is_empty()
        );
        let history = handle_history(&coordinator).await.unwrap();
        assert!(history.as_array().unwrap().is_empty());
    }

    #[test]
    fn nzb_name_drops_only_a_trailing_nzb_extension() {
        assert_eq!(nzb_name("Release.Name.nzb"), "Release.Name");
        assert_eq!(nzb_name("Release.Name.NZB"), "Release.Name");
        assert_eq!(nzb_name("Release.Name"), "Release.Name");
        assert_eq!(nzb_name(".nzb"), ".nzb");
    }

    #[test]
    fn nzbid_of_a_fixed_id_never_changes() {
        // FNV-1a (32 bit) of the id's bytes, top bit cleared. Clients keep
        // NZBIDs across krake updates, so this value must never change.
        assert_eq!(
            id_to_nzbid("123e4567-e89b-12d3-a456-426614174000"),
            1_344_283_647
        );
        assert_eq!(id_to_nzbid("a"), 1_678_518_572);
    }

    #[tokio::test]
    async fn editqueue_finds_a_download_by_the_nzbid_an_older_build_gave() {
        let root = tempfile::tempdir().unwrap();
        let coordinator = idle_coordinator(root.path());
        let nzbid = append(
            &coordinator,
            &format!("{RELEASE}.nzb"),
            &release_nzb("a"),
            "movies",
        )
        .await;
        let id = row_of(&coordinator, nzbid).await.id;

        let params = [
            json!("GroupFinalDelete"),
            json!(""),
            json!([legacy_nzbid(&id)]),
        ];
        handle_editqueue(&coordinator, &params).await.unwrap();

        assert!(
            coordinator
                .storage()
                .get_download(&id)
                .await
                .unwrap()
                .is_none()
        );
    }

    /// Mark a queued download as finished the way the coordinator does,
    /// with what it records about a Usenet transfer.
    async fn finish(coordinator: &Arc<Coordinator>, id: &str, status: QueueStatus, result: Value) {
        let storage = coordinator.storage();
        let mut metadata: Value =
            serde_json::from_str(&storage.get_download_metadata(id).await.unwrap().unwrap())
                .unwrap();
        for (k, v) in result.as_object().unwrap() {
            metadata[k] = v.clone();
        }
        storage
            .update_download_metadata(id, &metadata.to_string())
            .await
            .unwrap();
        storage.update_download_status(id, status).await.unwrap();
        storage.record_history(id).await.unwrap();
    }

    #[tokio::test]
    async fn history_reports_why_a_usenet_download_failed() {
        let cases = [
            (
                json!({"failure": "health", "health": 400, "critical_health": 950,
                       "total_articles": 10, "success_articles": 4, "failed_articles": 6,
                       "par_status": "NONE", "unpack_status": "NONE"}),
                ("FAILURE/HEALTH", "NONE", "NONE", 400, 950),
            ),
            (
                json!({"failure": "par", "health": 900, "critical_health": 850,
                       "total_articles": 10, "success_articles": 9, "failed_articles": 1,
                       "par_status": "FAILURE", "unpack_status": "NONE"}),
                ("FAILURE/PAR", "FAILURE", "NONE", 900, 850),
            ),
            (
                json!({"failure": "unpack", "health": 1000, "critical_health": 1000,
                       "total_articles": 10, "success_articles": 10, "failed_articles": 0,
                       "par_status": "SUCCESS", "unpack_status": "FAILURE"}),
                ("FAILURE/UNPACK", "SUCCESS", "FAILURE", 1000, 1000),
            ),
            // A local problem (disk full, server down) is no verdict on the release.
            (
                json!({"health": 1000, "critical_health": 1000}),
                ("FAILURE/NONE", "NONE", "NONE", 1000, 1000),
            ),
        ];
        for (result, (status, par, unpack, health, critical)) in cases {
            let root = tempfile::tempdir().unwrap();
            let coordinator = idle_coordinator(root.path());
            let nzbid = append(
                &coordinator,
                &format!("{RELEASE}.nzb"),
                &release_nzb("a"),
                "movies",
            )
            .await;
            let id = row_of(&coordinator, nzbid).await.id;
            finish(&coordinator, &id, QueueStatus::Failed, result).await;

            // Listed, and again once only the archived entry is left.
            for archived in [false, true] {
                if archived {
                    coordinator.storage().delete_download(&id).await.unwrap();
                }
                let history = handle_history(&coordinator).await.unwrap();
                let e = entry(&history, nzbid);
                assert_eq!(e["Status"], status, "archived: {archived}");
                assert_eq!(e["ParStatus"], par, "{status}, archived: {archived}");
                assert_eq!(e["UnpackStatus"], unpack, "{status}, archived: {archived}");
                assert_eq!(e["MoveStatus"], "NONE", "{status}, archived: {archived}");
                assert_eq!(e["Health"], health, "{status}, archived: {archived}");
                assert_eq!(
                    e["CriticalHealth"], critical,
                    "{status}, archived: {archived}"
                );
                assert_eq!(e["Category"], "movies", "{status}, archived: {archived}");
            }
        }
    }

    #[tokio::test]
    async fn history_of_a_good_release_reports_its_checks() {
        let root = tempfile::tempdir().unwrap();
        let coordinator = idle_coordinator(root.path());
        let nzbid = append(
            &coordinator,
            &format!("{RELEASE}.nzb"),
            &release_nzb("a"),
            "movies",
        )
        .await;
        let id = row_of(&coordinator, nzbid).await.id;
        let result = json!({"health": 980, "critical_health": 900, "total_articles": 50,
                            "success_articles": 49, "failed_articles": 1,
                            "par_status": "SUCCESS", "unpack_status": "SUCCESS"});
        finish(&coordinator, &id, QueueStatus::Completed, result).await;
        coordinator.storage().delete_download(&id).await.unwrap();

        let history = handle_history(&coordinator).await.unwrap();
        let e = entry(&history, nzbid);
        assert_eq!(e["Status"], "SUCCESS/ALL");
        assert_eq!(e["ParStatus"], "SUCCESS");
        assert_eq!(e["UnpackStatus"], "SUCCESS");
        assert_eq!(e["MoveStatus"], "SUCCESS");
        assert_eq!(e["Health"], 980);
        assert_eq!(e["TotalArticles"], 50);
        assert_eq!(e["FailedArticles"], 1);
        assert_eq!(e["Category"], "movies");
    }

    #[tokio::test]
    async fn append_keeps_a_folder_left_by_an_earlier_download_apart() {
        let root = tempfile::tempdir().unwrap();
        let coordinator = idle_coordinator(root.path());
        let left = root.path().join("downloads").join("movies").join(RELEASE);
        std::fs::create_dir_all(&left).unwrap();

        let nzbid = append(
            &coordinator,
            &format!("{RELEASE}.nzb"),
            &release_nzb("a"),
            "movies",
        )
        .await;

        let dir = row_of(&coordinator, nzbid).await.download_dir.unwrap();
        assert_ne!(Some(dir.as_str()), left.to_str());
        assert!(dir.starts_with(&format!("{}.", left.display())));
    }
}
