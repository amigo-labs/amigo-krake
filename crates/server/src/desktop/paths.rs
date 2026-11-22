//! Where the desktop app keeps its files (AC-6) and how it binds (AC-2).

use std::path::{Path, PathBuf};

use krake_core::config::Config;

use super::state::{DESKTOP_STATE_FILE, DesktopState};

/// The desktop app's files, all under the OS app data folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopPaths {
    pub app_data: PathBuf,
    pub config_path: PathBuf,
    pub db_path: PathBuf,
    pub plugins_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub state_path: PathBuf,
}

impl DesktopPaths {
    pub fn new(app_data: &Path) -> Self {
        Self {
            app_data: app_data.to_path_buf(),
            config_path: app_data.join("config.toml"),
            db_path: app_data.join("krake.db"),
            plugins_dir: app_data.join("plugins"),
            cache_dir: app_data.join("cache"),
            state_path: app_data.join(DESKTOP_STATE_FILE),
        }
    }

    /// Create the folders the server writes into.
    pub fn create_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.plugins_dir)?;
        std::fs::create_dir_all(&self.cache_dir)
    }

    /// The desktop config: the stored one, or on the first start the
    /// defaults with downloads in `<user downloads>/Krake` and the temp
    /// folder inside it. A first start writes the config, so the folders
    /// stay where they were chosen. Never anything relative to the working
    /// directory.
    pub fn load_config(&self, user_downloads: &Path) -> Result<Config, krake_core::Error> {
        if self.config_path.exists() {
            let mut config = Config::load(&self.config_path)?;
            // A relative path would land in the working directory.
            if Path::new(&config.download_dir).is_relative() {
                config.download_dir = default_download_dir(user_downloads).display().to_string();
            }
            if Path::new(&config.temp_dir).is_relative() {
                config.temp_dir = Path::new(&config.download_dir)
                    .join(".tmp")
                    .display()
                    .to_string();
            }
            return Ok(config);
        }
        let download_dir = default_download_dir(user_downloads);
        let config = Config {
            temp_dir: download_dir.join(".tmp").display().to_string(),
            download_dir: download_dir.display().to_string(),
            ..Config::default()
        };
        std::fs::create_dir_all(&self.app_data)?;
        config.save(&self.config_path)?;
        Ok(config)
    }
}

/// `<user downloads>/Krake`.
pub fn default_download_dir(user_downloads: &Path) -> PathBuf {
    user_downloads.join("Krake")
}

/// Bind `127.0.0.1` on the stored port, or on a free one when it is taken;
/// the port in use is stored (AC-2).
pub async fn bind_listener(
    state_path: &Path,
) -> std::io::Result<(tokio::net::TcpListener, DesktopState)> {
    let mut state = DesktopState::load(state_path);
    let stored = match state.port {
        Some(port) => tokio::net::TcpListener::bind(("127.0.0.1", port))
            .await
            .ok(),
        None => None,
    };
    let listener = match stored {
        Some(listener) => listener,
        None => tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?,
    };
    let port = listener.local_addr()?.port();
    if state.port != Some(port) {
        state.port = Some(port);
        if let Err(e) = state.save(state_path) {
            tracing::warn!("Could not store the port in {}: {e}", state_path.display());
        }
    }
    Ok((listener, state))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ac6_every_file_lives_under_app_data() {
        let paths = DesktopPaths::new(Path::new("/data/krake"));
        for p in [
            &paths.config_path,
            &paths.db_path,
            &paths.plugins_dir,
            &paths.cache_dir,
            &paths.state_path,
        ] {
            assert!(p.starts_with("/data/krake"), "{}", p.display());
        }
        assert_eq!(paths.db_path, Path::new("/data/krake/krake.db"));
    }

    #[test]
    fn ac6_first_start_downloads_go_to_the_user_downloads_folder() {
        let dir = tempfile::tempdir().unwrap();
        let paths = DesktopPaths::new(&dir.path().join("app"));
        let downloads = dir.path().join("Downloads");
        let config = paths.load_config(&downloads).unwrap();
        assert_eq!(Path::new(&config.download_dir), downloads.join("Krake"));
        assert_eq!(
            Path::new(&config.temp_dir),
            downloads.join("Krake").join(".tmp")
        );
        assert!(paths.config_path.is_file(), "first start writes the config");

        // A later start reads the stored config.
        let mut changed = config.clone();
        changed.download_dir = dir.path().join("elsewhere").display().to_string();
        changed.save(&paths.config_path).unwrap();
        let again = paths.load_config(&downloads).unwrap();
        assert_eq!(again.download_dir, changed.download_dir);
    }

    #[test]
    fn ac6_relative_folders_never_point_at_the_working_directory() {
        let dir = tempfile::tempdir().unwrap();
        let paths = DesktopPaths::new(&dir.path().join("app"));
        std::fs::create_dir_all(&paths.app_data).unwrap();
        Config::default().save(&paths.config_path).unwrap();
        let downloads = dir.path().join("Downloads");
        let config = paths.load_config(&downloads).unwrap();
        assert!(Path::new(&config.download_dir).is_absolute());
        assert!(Path::new(&config.temp_dir).is_absolute());
    }

    #[tokio::test]
    async fn ac2_stored_port_is_reused_and_a_taken_one_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let state_path = dir.path().join(DESKTOP_STATE_FILE);

        // First start: a free port, stored.
        let (first, state) = bind_listener(&state_path).await.unwrap();
        let port = first.local_addr().unwrap().port();
        assert_eq!(first.local_addr().unwrap().ip().to_string(), "127.0.0.1");
        assert_eq!(state.port, Some(port));
        assert_eq!(DesktopState::load(&state_path).port, Some(port));
        drop(first);

        // Next start: the same port.
        let (second, _) = bind_listener(&state_path).await.unwrap();
        assert_eq!(second.local_addr().unwrap().port(), port);

        // Taken by someone else: another port, stored.
        let (third, state) = bind_listener(&state_path).await.unwrap();
        let other = third.local_addr().unwrap().port();
        assert_ne!(other, port);
        assert_eq!(state.port, Some(other));
        assert_eq!(DesktopState::load(&state_path).port, Some(other));
        drop(second);
    }
}
