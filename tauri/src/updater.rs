//! The desktop updater on top of tauri-plugin-updater
//! (`docs/specs/desktop-app.md`, AC-13 to AC-16). When and whether to check,
//! install or announce is `krake_server::desktop::update::UpdateController`;
//! this file only downloads, verifies and installs.
//!
//! - The signature is verified against `plugins.updater.pubkey` right after
//!   the download, before anything is installed (AC-14).
//! - macOS replaces the app bundle and Linux the AppImage at once; the new
//!   version runs at the next start. Windows keeps the verified installer in
//!   memory and runs it (passive) once tray Quit has ended the app (AC-15).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use krake_server::desktop::update::{UpdateBackend, UpdateController, can_self_update};
use krake_server::update_api::DesktopUpdateInfo;
use tauri::AppHandle;
use tauri_plugin_updater::{Update, UpdaterExt};

/// The app's updater: the controller over the Tauri backend.
pub type TauriUpdater = UpdateController<TauriBackend>;

/// Build the updater for this copy.
pub fn new_updater(app: AppHandle, state_path: PathBuf, interval_hours: u64) -> TauriUpdater {
    let allowed = can_self_update(
        cfg!(debug_assertions),
        std::env::consts::OS,
        std::env::var("APPIMAGE").ok().as_deref(),
    );
    UpdateController::new(
        TauriBackend {
            app,
            last: Mutex::new(None),
            held: Mutex::new(None),
        },
        state_path,
        allowed,
        interval_hours,
    )
}

/// Start the background checks on the app's runtime.
pub fn start_background_checks(updater: &Arc<TauriUpdater>) {
    tauri::async_runtime::spawn(updater.clone().run_background());
}

pub struct TauriBackend {
    app: AppHandle,
    /// The update the last check found, so the download needs no second
    /// request to `latest.json`.
    last: Mutex<Option<Update>>,
    /// Windows: the verified installer, run when the app ends.
    held: Mutex<Option<(Update, Vec<u8>)>>,
}

impl TauriBackend {
    async fn fetch(&self) -> Result<Option<Update>, String> {
        let updater = self.app.updater().map_err(|e| e.to_string())?;
        updater.check().await.map_err(|e| e.to_string())
    }
}

#[async_trait::async_trait]
impl UpdateBackend for TauriBackend {
    async fn check(&self) -> Result<Option<DesktopUpdateInfo>, String> {
        let found = self.fetch().await?;
        let info = found.as_ref().map(|u| DesktopUpdateInfo {
            version: u.version.clone(),
            notes: u.body.clone(),
        });
        *self.last.lock().unwrap_or_else(|e| e.into_inner()) = found;
        Ok(info)
    }

    async fn download_and_install(&self, info: &DesktopUpdateInfo) -> Result<(), String> {
        let cached = self
            .last
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .filter(|u| u.version == info.version);
        let update = match cached {
            Some(update) => update,
            None => self
                .fetch()
                .await?
                .filter(|u| u.version == info.version)
                .ok_or_else(|| format!("Krake {} is no longer offered", info.version))?,
        };
        let bytes = update
            .download(|_, _| {}, || {})
            .await
            .map_err(|e| e.to_string())?;
        // AC-14: nothing is installed or held without a valid signature.
        update.verify(&bytes).map_err(|e| {
            tracing::error!("Update {} rejected: {e}", update.version);
            format!("signature check failed: {e}")
        })?;
        if cfg!(windows) {
            *self.held.lock().unwrap_or_else(|e| e.into_inner()) = Some((update, bytes));
            return Ok(());
        }
        update.install(&bytes).map_err(|e| e.to_string())
    }

    fn on_exit(&self) {
        if let Some((update, bytes)) = self.held.lock().unwrap_or_else(|e| e.into_inner()).take()
            && let Err(e) = update.install(&bytes)
        {
            tracing::error!("Could not run the installer of {}: {e}", update.version);
        }
    }
}
