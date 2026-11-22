//! Krake desktop app (`docs/specs/desktop-app.md`).
//!
//! Runs the same server stack as `krake-server` on `127.0.0.1`
//! (`krake_server::embedded::serve`) and opens its window at that origin,
//! signed in through a per-launch token. Closing the window keeps downloads
//! running in the tray; a second launch hands its links to the first.
//! The webview gets no Tauri IPC: everything goes through the HTTP API.
//!
//! ## Building
//!
//! ```bash
//! cargo install tauri-cli
//! pnpm build:web-ui
//! cargo build -p krake-plugin-runtime --bin krake-plugin-host --release
//! # copy it to tauri/binaries/krake-plugin-host-<target triple>
//! cd tauri && cargo tauri build
//! ```

#[cfg(feature = "tauri")]
mod app;
#[cfg(feature = "tauri")]
mod updater;

#[cfg(feature = "tauri")]
fn main() {
    app::run();
}

// When built without Tauri (regular cargo build), show instructions
#[cfg(not(feature = "tauri"))]
fn main() {
    eprintln!("Krake desktop app");
    eprintln!();
    eprintln!("This binary must be built with the Tauri CLI:");
    eprintln!("  1. cargo install tauri-cli");
    eprintln!("  2. pnpm build:web-ui");
    eprintln!("  3. cd tauri && cargo tauri build");
    eprintln!();
    eprintln!("For headless/server use, run `krake-server` instead.");
}
