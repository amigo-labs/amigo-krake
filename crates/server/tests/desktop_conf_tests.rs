//! The desktop bundle configuration the release depends on
//! (`docs/specs/desktop-app.md`). Read from `tauri/tauri.conf.json`, so the
//! workspace tests guard it without building the Tauri app.

fn conf() -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tauri/tauri.conf.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn ac7_the_plugin_host_is_bundled_as_a_sidecar() {
    let conf = conf();
    let external = conf["bundle"]["externalBin"].as_array().unwrap();
    assert!(
        external.iter().any(|b| b == "binaries/krake-plugin-host"),
        "{external:?}"
    );
}

#[test]
fn the_window_is_created_at_runtime() {
    let conf = conf();
    assert!(
        conf["app"]["windows"]
            .as_array()
            .is_none_or(|w| w.is_empty()),
        "the window is opened at the server URL at runtime"
    );
}

#[test]
fn ac10_ac11_deep_link_schemes_are_registered() {
    let conf = conf();
    let schemes = conf["plugins"]["deep-link"]["desktop"]["schemes"]
        .as_array()
        .unwrap();
    for scheme in ["krake", "amigo", "magnet"] {
        assert!(schemes.iter().any(|s| s == scheme), "{scheme}: {schemes:?}");
    }
}

#[test]
fn no_ipc_capability_is_granted() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tauri/capabilities");
    assert!(
        !dir.exists() || std::fs::read_dir(&dir).unwrap().next().is_none(),
        "the remote origin must get no Tauri IPC"
    );
    let conf = conf();
    assert!(
        conf["app"]["security"]["capabilities"]
            .as_array()
            .is_none_or(|c| c.is_empty())
    );
}
