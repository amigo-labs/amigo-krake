//! The desktop updater. Until P3 of `docs/specs/desktop-app.md` lands this
//! copy never updates itself: it reports so and offers nothing.

use std::path::PathBuf;

use krake_server::update_api::{DesktopModeState, DesktopUpdateInfo, DesktopUpdater, UpdateMode};
use tauri::AppHandle;

pub struct TauriUpdater {
    _app: AppHandle,
    _state_path: PathBuf,
}

impl TauriUpdater {
    pub fn new(app: AppHandle, state_path: PathBuf) -> Self {
        Self {
            _app: app,
            _state_path: state_path,
        }
    }

    pub fn start_background_checks(&self) {}

    pub fn window_shown(&self) {}

    pub fn window_hidden(&self) {}

    pub fn on_exit(&self) {}
}

#[async_trait::async_trait]
impl DesktopUpdater for TauriUpdater {
    fn can_self_update(&self) -> bool {
        false
    }
    fn pending_restart(&self) -> Option<String> {
        None
    }
    async fn check(&self) -> Result<Option<DesktopUpdateInfo>, String> {
        Ok(None)
    }
    async fn apply(&self) -> Result<String, String> {
        Err("self-update is not available yet".into())
    }
    fn mode_state(&self) -> DesktopModeState {
        DesktopModeState {
            update_mode: UpdateMode::Auto,
            update_mode_chosen: false,
            question_pending: false,
            can_self_update: false,
            pending_restart: None,
            announcement: None,
            saved: true,
        }
    }
    fn set_mode(&self, _mode: UpdateMode) -> DesktopModeState {
        self.mode_state()
    }
    fn dismiss_question(&self) {}
    fn acknowledge(&self, version: &str) -> Result<(), String> {
        Err(format!("{version} is not the pending announcement"))
    }
}
