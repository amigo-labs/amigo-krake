//! Desktop-app logic that needs no Tauri (`docs/specs/desktop-app.md`):
//! `desktop.json`, the app's paths, the listener, deep links and the update
//! controller. The Tauri app in `tauri/` wires these to windows, the tray
//! and tauri-plugin-updater; keeping them here lets the workspace tests run
//! them.

pub mod links;
pub mod paths;
pub mod state;
pub mod tray;
