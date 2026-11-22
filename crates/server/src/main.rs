use std::path::PathBuf;

use tracing_subscriber::EnvFilter;

use krake_core::config::Config;
use krake_server::embedded::{self, ServeOptions, env_flag};
use krake_server::password;

/// The database, relative to the working directory.
const DB_FILE: &str = "krake.db";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config_path = Config::resolve_path();
    let mut config = Config::load(&config_path).unwrap_or_default();

    // Env overrides — useful for Docker / IaC where flipping `bind` or
    // enabling reverse-proxy awareness shouldn't require editing a TOML
    // file that also lives in a volume.
    if let Ok(bind) = krake_core::env::var("KRAKE_BIND")
        && !bind.trim().is_empty()
    {
        config.server.bind = bind;
    }
    if env_flag("KRAKE_TRUST_PROXY") {
        config.server.trust_proxy = true;
    }
    if env_flag("KRAKE_AUTO_UPDATE_PLUGINS") {
        config.update.auto_update_plugins = true;
    }

    // Headless setup: if `KRAKE_SETUP_USER` + `KRAKE_SETUP_PASSWORD` are
    // provided and the wizard has not been completed, finish it now so the
    // server comes up fully authenticated without the browser.
    if !config.server.setup_complete
        && let (Ok(user), Ok(pw)) = (
            krake_core::env::var("KRAKE_SETUP_USER"),
            krake_core::env::var("KRAKE_SETUP_PASSWORD"),
        )
    {
        if user.trim().is_empty() || pw.len() < 8 {
            tracing::error!(
                "KRAKE_SETUP_USER / KRAKE_SETUP_PASSWORD provided but invalid (user empty or password < 8 chars)"
            );
            anyhow::bail!("invalid setup env vars");
        }
        let hash =
            password::hash_password(&pw).map_err(|e| anyhow::anyhow!("setup hash failed: {e}"))?;
        config.server.admin_username = Some(user);
        config.server.admin_password_hash = Some(hash);
        config.server.setup_complete = true;
        config.save(&config_path)?;
        tracing::info!("Setup completed from environment variables — admin account active.");
    }

    // Pre-rename installs kept their database as `amigo.db`.
    if let Err(e) = krake_core::migrate::migrate_db(
        std::path::Path::new("amigo.db"),
        std::path::Path::new(DB_FILE),
    ) {
        tracing::warn!("Could not move amigo.db to {DB_FILE}: {e}");
    }
    // Locally compiled plugin bytecode and other daemon-only caches live next
    // to the config (`KRAKE_CONFIG_DIR`), else in the working directory.
    let cache_dir = krake_core::env::var_os("KRAKE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("cache");

    let bind = config.server.bind.clone();
    tracing::info!("Starting krake-server on {bind}");
    let listener = tokio::net::TcpListener::bind(&bind).await?;

    let handle = embedded::serve(ServeOptions {
        config,
        config_path,
        db_path: PathBuf::from(DB_FILE),
        plugins_dir: PathBuf::from("plugins"),
        cache_dir,
        listener,
        start_clicknload: true,
        desktop: None,
    })
    .await?;
    handle.task.await??;
    Ok(())
}
