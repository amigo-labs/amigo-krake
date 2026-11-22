//! `<app_data>/desktop.json`: what the desktop app remembers between runs.

use std::path::Path;

use serde::{Deserialize, Serialize};

pub use crate::update_api::UpdateMode;

/// The file name of the desktop state inside the app data folder.
pub const DESKTOP_STATE_FILE: &str = "desktop.json";

/// `<app_data>/desktop.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct DesktopState {
    /// Last port the embedded server used (AC-2).
    pub port: Option<u16>,
    /// Unix seconds of the last background update check (AC-13).
    pub last_update_check: u64,
    /// The user's update choice; `None` until confirmed (AC-25, AC-26).
    pub update_mode: Option<UpdateMode>,
    /// Last version announced in "notify" (AC-28).
    pub announced_version: Option<String>,
}

impl DesktopState {
    /// Read `path`; a missing or damaged file gives the defaults.
    pub fn load(path: &Path) -> Self {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                tracing::warn!(
                    "{} is damaged ({e}); starting from defaults",
                    path.display()
                );
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    /// Write `path` through a temporary file and a rename, so a crash never
    /// leaves half a file.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_keeps_every_field() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DESKTOP_STATE_FILE);
        let state = DesktopState {
            port: Some(4242),
            last_update_check: 1_700_000_000,
            update_mode: Some(UpdateMode::Notify),
            announced_version: Some("1.2.3".into()),
        };
        state.save(&path).unwrap();
        assert_eq!(DesktopState::load(&path), state);
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn missing_or_damaged_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DESKTOP_STATE_FILE);
        assert_eq!(DesktopState::load(&path), DesktopState::default());
        std::fs::write(&path, b"{ not json").unwrap();
        assert_eq!(DesktopState::load(&path), DesktopState::default());
    }

    #[test]
    fn ac25_a_port_without_update_mode_means_no_choice() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DESKTOP_STATE_FILE);
        std::fs::write(&path, br#"{"port": 1516}"#).unwrap();
        let state = DesktopState::load(&path);
        assert_eq!(state.port, Some(1516));
        assert_eq!(state.update_mode, None);
    }

    #[test]
    fn update_mode_is_stored_lowercase() {
        let state = DesktopState {
            update_mode: Some(UpdateMode::Off),
            ..Default::default()
        };
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains(r#""update_mode":"off""#), "{json}");
    }
}
