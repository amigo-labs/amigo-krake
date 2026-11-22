//! The whole server stack behind one call, shared by the `krake-server`
//! binary and the desktop app (`docs/specs/desktop-app.md`, AC-1): API,
//! WebSocket, updates, feedback, NZBGet, the static web UI, CORS and the
//! security headers, served on a listener the caller has already bound.

use std::path::PathBuf;
use std::sync::Arc;

use axum::http::{HeaderValue, Method, header};
use tower_http::cors::CorsLayer;

use krake_core::captcha::CaptchaManager;
use krake_core::config::Config;
use krake_core::coordinator::{Coordinator, DownloadEvent};
use krake_core::storage::Storage;
use krake_plugin_runtime::loader::PluginLoader;
use krake_plugin_runtime::registry::RegistryConfig;
use krake_plugin_runtime::sandbox::SandboxLimits;
use krake_plugin_runtime::updater::PluginUpdater;

use crate::update_api::DesktopUpdater;
use crate::{
    api, auth, background, clicknload, desktop_session, feedback, login, nzbget_api, pairing,
    resolver, security_headers_layer, setup, static_files, update_api, webhooks, ws,
};

/// Everything [`serve`] needs; the binary builds it from env and config, the
/// desktop app from Tauri's path resolver.
pub struct ServeOptions {
    pub config: Config,
    /// Where setup and `PUT /config` persist the config.
    pub config_path: PathBuf,
    pub db_path: PathBuf,
    pub plugins_dir: PathBuf,
    /// Daemon-writable cache (plugin bytecode).
    pub cache_dir: PathBuf,
    /// Already bound; its local address decides loopback handling.
    pub listener: tokio::net::TcpListener,
    pub start_clicknload: bool,
    /// `Some` in the desktop app: enables `/api/v1/desktop/session` and the
    /// desktop update routes.
    pub desktop: Option<DesktopOptions>,
}

/// What the desktop app adds to the server.
pub struct DesktopOptions {
    /// Per-launch token, 32 random bytes, lowercase hex. Single use.
    pub session_token: String,
    pub updater: Arc<dyn DesktopUpdater>,
}

/// Handle returned by [`serve`]; dropping it does not stop the server.
pub struct ServerHandle {
    pub local_addr: std::net::SocketAddr,
    pub coordinator: Arc<Coordinator>,
    /// Send `()` to stop accepting connections and end the server task.
    pub shutdown: tokio::sync::oneshot::Sender<()>,
    /// The server task; finishes after a shutdown or a fatal I/O error.
    pub task: tokio::task::JoinHandle<std::io::Result<()>>,
}

/// Whether a boolean-style environment variable is set to a truthy value
/// (`1` / `true` / `yes` / `on`, case-insensitive).
pub fn env_flag(name: &str) -> bool {
    krake_core::env::var(name)
        .is_ok_and(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
}

/// Start the server stack on `opts.listener` and return once it accepts
/// connections.
pub async fn serve(opts: ServeOptions) -> anyhow::Result<ServerHandle> {
    let ServeOptions {
        mut config,
        config_path,
        db_path,
        plugins_dir,
        cache_dir,
        listener,
        start_clicknload,
        desktop,
    } = opts;
    let local_addr = listener.local_addr()?;
    let bind_is_loopback = local_addr.ip().is_loopback();

    if desktop.is_some() {
        // The desktop app owns its listener; the config's bind does not
        // apply. There is no setup wizard: the per-launch session token
        // stands in for the admin login (AC-3, AC-4).
        config.server.bind = local_addr.to_string();
        config.server.setup_complete = true;
    }

    // Reject misconfigured bind/token combinations early.
    let validation_errors = config.validate();
    if !validation_errors.is_empty() {
        for err in &validation_errors {
            tracing::error!("Config error: {err}");
        }
        anyhow::bail!("Invalid configuration — refusing to start");
    }

    // Login mode (spec `local-auth-bypass`): an unknown value, or `never`
    // on a bind other people can reach, stops the start (AC-04, AC-10).
    let auth_mode_env = crate::resolve_auth_mode(
        &config,
        krake_core::env::var("KRAKE_AUTH_MODE").ok().as_deref(),
    )
    .map_err(|e| anyhow::anyhow!("{e} — refusing to start"))?;

    // NZBGet JSON-RPC has its own HTTP Basic Auth layer. Refuse to start when
    // the operator turned the API on without setting credentials.
    nzbget_api::validate_nzbget_config(&config.nzbget_api).map_err(|e| anyhow::anyhow!(e))?;

    let storage = Storage::open(
        db_path,
        PathBuf::from(&config.download_dir),
        PathBuf::from(&config.temp_dir),
    )?;
    let mut coordinator = Coordinator::new(config.clone(), storage);

    // Shared HTTP client
    let http_client = reqwest::Client::builder().user_agent("krake").build()?;

    let captcha_manager = Arc::new(CaptchaManager::new(
        coordinator.event_sender(),
        &config.captcha,
    ));

    // Load plugins with notify + captcha callbacks wired. `from_sandbox`
    // picks up the SSRF policy and request limit from the same struct that
    // governs other sandbox limits.
    let sandbox_limits = SandboxLimits::default();
    let mut plugin_host_api =
        krake_plugin_runtime::host_api::HostApi::from_sandbox(&sandbox_limits);

    // Wire notify callback: plugin → broadcast → WebSocket → UI toast
    {
        let event_tx = coordinator.event_sender();
        plugin_host_api.set_notify_callback(Arc::new(move |plugin_id, title, message| {
            let _ = event_tx.send(DownloadEvent::PluginNotification {
                plugin_id: plugin_id.to_string(),
                title: title.to_string(),
                message: message.to_string(),
            });
        }));
    }

    // Wire captcha callback: plugin → CaptchaManager → WebSocket → UI → REST → plugin
    {
        let cm = captcha_manager.clone();
        plugin_host_api.set_captcha_callback(Arc::new(
            move |plugin_id, download_id, image_url, captcha_type| {
                let cm = cm.clone();
                Box::pin(async move {
                    cm.request_solve(&plugin_id, &download_id, &image_url, &captcha_type)
                        .await
                        .map_err(|e| e.to_string())
                })
            },
        ));
    }

    let mut plugin_loader =
        PluginLoader::new_with_host_api(plugins_dir, sandbox_limits, plugin_host_api)
            .map_err(|e| anyhow::anyhow!("Failed to initialize plugin runtime: {e}"))?;
    // Locally compiled plugin bytecode, so restarts skip transpile + compile.
    // Daemon-writable only — plugin JS has no filesystem access; see
    // `krake_plugin_runtime::bytecode_cache`.
    let bytecode_dir = cache_dir.join("plugin-bytecode");
    match krake_plugin_runtime::bytecode_cache::BytecodeCache::open(
        &bytecode_dir,
        krake_plugin_runtime::bytecode_cache::DEFAULT_MAX_ENTRIES,
    ) {
        Ok(cache) => plugin_loader = plugin_loader.with_bytecode_cache(cache),
        Err(e) => tracing::warn!(
            "Plugin bytecode cache disabled ({}): {e}",
            bytecode_dir.display()
        ),
    }
    let plugin_loader = Arc::new(plugin_loader);
    let discovered = plugin_loader.discover().await.unwrap_or_default();
    tracing::info!("Loaded {} plugins", discovered.len());

    // Wire URL resolvers: plugins handle all URL resolution (YouTube, generic-http, etc.)
    let plugin_resolver = resolver::PluginUrlResolver::new(plugin_loader.clone());
    coordinator.set_resolvers(vec![Arc::new(plugin_resolver)]);

    let coordinator = Arc::new(coordinator);

    // Spawn queue advance loop — starts next queued download when one completes
    coordinator.spawn_queue_advance_loop();

    // Recover any downloads that were in-progress when the server last stopped
    let recovered = coordinator.recover_downloads().await.unwrap_or(0);
    if recovered > 0 {
        tracing::info!("Recovered {recovered} interrupted downloads");
    }

    // Plugin updater. Registry signatures are required by default — a
    // developer bootstrap override (`KRAKE_PLUGIN_REGISTRY_DEV_UNSIGNED=1`)
    // disables verification with a loud warning, for working against a
    // locally-hosted unsigned fork.
    let dev_unsigned = env_flag("KRAKE_PLUGIN_REGISTRY_DEV_UNSIGNED");
    if dev_unsigned {
        tracing::warn!(
            "KRAKE_PLUGIN_REGISTRY_DEV_UNSIGNED is set — plugin registry signatures will NOT be verified. DO NOT use this for production."
        );
    }
    // Derived from `RegistryConfig::default()`, which never trusts the
    // all-zero placeholder key of a build without KRAKE_REGISTRY_PUBKEY_HEX.
    let registry_config =
        RegistryConfig::for_index(config.update.plugin_registry_url.clone(), dev_unsigned);
    let plugin_updater = Arc::new(PluginUpdater::new(
        registry_config,
        http_client.clone(),
        plugin_loader.clone(),
    ));

    // Webhook dispatcher
    let webhook_dispatcher = Arc::new(webhooks::WebhookDispatcher::new(config.webhooks.clone()));
    {
        let dispatcher = webhook_dispatcher.clone();
        let event_rx = coordinator.subscribe();
        tokio::spawn(async move {
            dispatcher.run(event_rx).await;
        });
    }

    let state = api::AppState {
        coordinator: coordinator.clone(),
        plugins: plugin_loader,
        plugin_updater,
        http_client,
        captcha_manager,
        webhook_dispatcher,
        config_path,
        auth_mode_env,
        bind_is_loopback,
        config_lock: Arc::new(tokio::sync::Mutex::new(())),
        config_epoch: Arc::new(tokio::sync::watch::channel(0).0),
        desktop_updater: desktop.as_ref().map(|d| d.updater.clone()),
    };

    // Feedback rate limiter
    let feedback_limiter = feedback::new_rate_limiter(config.feedback.max_issues_per_hour);

    // Build the CORS layer from explicit config. Empty = no CORS (same-origin
    // only, which is what the bundled Web UI needs). Permissive was a blanket
    // CSRF hole.
    let cors = if config.server.cors_origins.is_empty() {
        CorsLayer::new()
    } else {
        let origins: Vec<HeaderValue> = config
            .server
            .cors_origins
            .iter()
            .filter_map(|o| o.parse().ok())
            .collect();
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::PATCH,
                Method::DELETE,
            ])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
    };

    let setup_pin_env = krake_core::env::var("KRAKE_SETUP_PIN").ok();
    let auth_state = auth::AuthState::new(
        state.clone(),
        config.server.api_token.clone(),
        setup_pin_env,
        config.server.setup_complete,
        bind_is_loopback,
    );
    if !config.server.setup_complete && !bind_is_loopback {
        tracing::warn!(
            "Setup not complete — server is running in setup-mode. Open http://{local_addr} in a browser to configure the admin account."
        );
    }
    let auth_layer = axum::middleware::from_fn_with_state(auth_state.clone(), auth::require_auth);
    let setup_guard_layer =
        axum::middleware::from_fn_with_state(auth_state.clone(), auth::setup_guard);

    let protected = api::router(state.clone())
        .merge(ws::ws_router(state.clone()))
        .merge(update_api::update_router(state.clone()))
        .merge(feedback::feedback_router(state.clone(), feedback_limiter))
        .layer(auth_layer);

    // Public / partially-authenticated routes (login, /me, setup wizard,
    // pairing endpoints).
    let mut open = setup::setup_router(state.clone(), auth_state.clone())
        .merge(login::login_router(state.clone(), auth_state.clone()))
        .merge(pairing::pairing_router(state.clone(), auth_state.clone()));
    if let Some(desktop) = &desktop {
        open = open.merge(desktop_session::desktop_session_router(
            state.clone(),
            desktop.session_token.clone(),
        ));
    }

    // NZBGet JSON-RPC has its own HTTP Basic Auth layer (Sonarr/Radarr
    // compatibility) and static files are public.
    let mut app = protected.merge(open);
    if let Some(nzb) = nzbget_api::nzbget_router(state.clone(), &config.nzbget_api) {
        app = app.merge(nzb);
    }
    let app = app
        .merge(static_files::static_router())
        .layer(setup_guard_layer)
        .layer(cors)
        // Set baseline security headers (X-Content-Type-Options, X-Frame-
        // Options, Referrer-Policy, CSP, Permissions-Policy) on every
        // response. Outermost layer so even error / redirect responses
        // carry them.
        .layer(axum::middleware::from_fn(security_headers_layer));

    // Start background tasks (NZB watch folder, RSS poller)
    background::spawn_background_tasks(
        coordinator.clone(),
        state.http_client.clone(),
        state.plugin_updater.clone(),
    );

    // Click'n'Load listener on port 9666 — started here only, so the desktop
    // app never runs a second one (AC-8).
    if start_clicknload {
        let cnl_coordinator = coordinator.clone();
        tokio::spawn(async move {
            if let Err(e) = clicknload::start_clicknload(cnl_coordinator).await {
                tracing::warn!("Click'n'Load listener failed: {e}");
            }
        });
    }

    tracing::info!("Serving Krake on {local_addr}");
    let (shutdown, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let task = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .with_graceful_shutdown(async move {
            // A dropped handle is not a shutdown: only an explicit send is.
            if shutdown_rx.await.is_err() {
                std::future::pending::<()>().await;
            }
        })
        .await
    });

    Ok(ServerHandle {
        local_addr,
        coordinator,
        shutdown,
        task,
    })
}
