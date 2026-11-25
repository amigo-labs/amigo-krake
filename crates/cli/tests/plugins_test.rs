//! `krake plugins test <plugin>` runs the plugin it was given, not whichever
//! plugin of the same category folder happens to load first.
//!
//! Needs `krake-plugin-host` next to the `krake` binary (built by
//! `cargo test --workspace`).

use std::path::Path;
use std::process::Command;

fn write_plugin(dir: &Path, id: &str) {
    std::fs::create_dir_all(dir).unwrap();
    let source = format!(
        r#"module.exports = {{
    id: "{id}",
    name: "Plugin {id}",
    version: "1.0.0",
    apiVersion: 1,
    urlPattern: "https?://{id}\\.example/.+",
    resolve(url) {{ return {{ name: "{id}", downloads: [] }}; }},
}};
"#
    );
    std::fs::write(dir.join("plugin.js"), source).unwrap();
}

#[test]
fn tests_the_given_plugin_of_a_category() {
    let tmp = tempfile::tempdir().unwrap();
    let category = tmp.path().join("plugins").join("hosters");
    for id in ["alpha", "beta", "gamma"] {
        write_plugin(&category.join(id), id);
    }

    for id in ["alpha", "beta", "gamma"] {
        let plugin = category.join(id).join("plugin.js");
        let out = Command::new(env!("CARGO_BIN_EXE_krake"))
            .args(["plugins", "test"])
            .arg(&plugin)
            // Does not match the urlPattern, so nothing reaches the network.
            .arg("https://unrelated.invalid/x")
            .env("KRAKE_CONFIG_DIR", tmp.path())
            .env("NO_COLOR", "1")
            .output()
            .expect("run krake");
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stdout.contains(&format!("Plugin: Plugin {id} v1.0.0 ({id})")),
            "expected plugin {id}\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );
    }
}
