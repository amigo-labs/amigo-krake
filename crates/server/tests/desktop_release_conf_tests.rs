//! The release side of the desktop bundle configuration
//! (`docs/specs/desktop-app.md`, AC-14 config side, AC-19, AC-32): read from
//! `tauri/tauri.conf.json`, so the workspace tests guard it without building
//! the Tauri app.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

/// The organization's minisign key; every installed copy trusts only it.
const ORG_KEY_ID: &str = "3098504C1538B8F6";

fn tauri_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tauri")
}

fn conf() -> serde_json::Value {
    let path = tauri_dir().join("tauri.conf.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Key ID of a Tauri pubkey: base64 of a minisign text whose second line is
/// base64 of `"Ed"`, the 8-byte key ID and the 32-byte key. The ID is shown
/// as the little-endian hex of its bytes.
fn minisign_key_id(tauri_pubkey: &str) -> String {
    let text = String::from_utf8(STANDARD.decode(tauri_pubkey).unwrap()).unwrap();
    let mut lines = text.lines();
    let comment = lines.next().unwrap();
    assert!(comment.starts_with("untrusted comment:"), "{comment}");
    let key = STANDARD.decode(lines.next().unwrap().trim()).unwrap();
    assert_eq!(key.len(), 42, "minisign public key length");
    assert_eq!(&key[..2], b"Ed", "minisign algorithm");
    key[2..10]
        .iter()
        .rev()
        .map(|b| format!("{b:02X}"))
        .collect()
}

#[test]
fn ac22_the_bundler_is_active() {
    // Tauri 2 defaults `bundle.active` to false: without it `cargo tauri
    // build` builds the app and silently produces no installer at all.
    assert_eq!(conf()["bundle"]["active"], true);
}

#[test]
fn ac19_the_bundle_creates_updater_artifacts() {
    assert_eq!(conf()["bundle"]["createUpdaterArtifacts"], true);
}

#[test]
fn ac14_the_updater_trusts_the_organization_key() {
    let conf = conf();
    let pubkey = conf["plugins"]["updater"]["pubkey"].as_str().unwrap();
    assert_eq!(minisign_key_id(pubkey), ORG_KEY_ID);
}

#[test]
fn ac15_windows_installs_passively_and_the_updater_has_no_v1_keys() {
    let conf = conf();
    let updater = &conf["plugins"]["updater"];
    assert_eq!(updater["windows"]["installMode"], "passive");
    assert!(updater.get("active").is_none(), "Tauri v1 leftover");
    let endpoints = updater["endpoints"].as_array().unwrap();
    assert!(
        endpoints.iter().any(|e| e
            == "https://github.com/amigo-labs/amigo-krake/releases/latest/download/latest.json"),
        "{endpoints:?}"
    );
}

#[test]
fn ac32_the_installer_hooks_exist_and_store_auto_on_a_silent_setup() {
    let conf = conf();
    let hooks = conf["bundle"]["windows"]["nsis"]["installerHooks"]
        .as_str()
        .unwrap();
    let nsh =
        std::fs::read_to_string(tauri_dir().join(hooks)).unwrap_or_else(|e| panic!("{hooks}: {e}"));
    assert!(nsh.contains("!macro NSIS_HOOK_POSTINSTALL"), "{nsh}");
    assert!(nsh.contains("IfSilent"), "{nsh}");
    assert!(
        nsh.contains(r"$APPDATA\com.amigo-labs.krake\desktop.json"),
        "the hook must write Tauri's app_data_dir for the identifier"
    );
    assert_eq!(conf["identifier"], "com.amigo-labs.krake");
    assert!(nsh.contains(r#"{"update_mode":"auto"}"#), "{nsh}");
}
