//! Registry build and signing (docs/specs/plugin-registry.md, AC-01 to AC-09).
//!
//! Plugin trees are written into temp dirs; every plugin is loaded through
//! the real plugin host, like the publish job does.

use std::path::{Path, PathBuf};

use ed25519_dalek::SigningKey;
use krake_plugin_runtime::loader::PluginLoader;
use krake_plugin_runtime::registry::{RegistryConfig, RegistryIndex, refresh_index};
use krake_plugin_runtime::registry_build::{
    BuildOptions, INDEX_FILE, RegistryBuildError, SIGNATURE_FILE, build_registry, sign_index,
    sign_index_file,
};
use krake_plugin_runtime::sandbox::SandboxLimits;
use sha2::{Digest, Sha256};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const BASE: &str = "https://registry.test";

fn plugin_src(id: &str, version: &str) -> String {
    format!(
        r#"module.exports = {{
    id: "{id}", name: "Name {id}", version: "{version}", apiVersion: 1,
    urlPattern: "https?://{id}\\.test/.+",
    resolve(url) {{ return {{ name: "x", downloads: [] }}; }},
}};
"#
    )
}

fn write(root: &Path, rel: &str, body: impl AsRef<[u8]>) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, body).unwrap();
}

fn loader(dir: &Path) -> PluginLoader {
    PluginLoader::new(dir.to_path_buf(), SandboxLimits::default()).unwrap()
}

fn opts(plugins: &Path, out: &Path, published: Option<&Path>) -> BuildOptions {
    BuildOptions {
        plugins_dir: plugins.to_path_buf(),
        out_dir: out.to_path_buf(),
        base_url: BASE.into(),
        published: published.map(Path::to_path_buf),
    }
}

async fn build(
    plugins: &Path,
    out: &Path,
    published: Option<&Path>,
) -> Result<RegistryIndex, RegistryBuildError> {
    let l = loader(plugins);
    build_registry(&l, &opts(plugins, out, published))
        .await
        .map(|r| r.index)
}

fn read_index(dir: &Path) -> RegistryIndex {
    serde_json::from_slice(&std::fs::read(dir.join(INDEX_FILE)).unwrap()).unwrap()
}

fn failure_text(err: RegistryBuildError) -> String {
    match err {
        RegistryBuildError::Plugins(f) => f
            .iter()
            .map(|f| format!("{}: {}", f.plugin, f.reason))
            .collect::<Vec<_>>()
            .join("\n"),
        other => panic!("expected plugin failures, got {other}"),
    }
}

fn tmp() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let t = tempfile::tempdir().unwrap();
    let plugins = t.path().join("plugins");
    let out = t.path().join("out");
    std::fs::create_dir_all(&plugins).unwrap();
    (t, plugins, out)
}

#[tokio::test]
async fn ac01_one_entry_per_plugin_in_the_three_categories() {
    let (_t, plugins, out) = tmp();
    write(
        &plugins,
        "hosters/alpha/plugin.ts",
        plugin_src("alpha", "1.0.0"),
    );
    write(&plugins, "hosters/alpha/plugin.spec.ts", "export {};");
    write(
        &plugins,
        "multi-hosters/beta/plugin.js",
        plugin_src("beta", "1.0.0"),
    );
    write(
        &plugins,
        "extractors/gamma/plugin.ts",
        plugin_src("gamma", "1.0.0"),
    );
    write(
        &plugins,
        "template/plugin.ts",
        plugin_src("template", "1.0.0"),
    );
    write(&plugins, "types/krake.d.ts", "declare const krake: any;");

    let index = build(&plugins, &out, None).await.unwrap();
    assert_eq!(index.schema_version, 1);
    let ids: Vec<_> = index.plugins.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["alpha", "beta", "gamma"]);
    assert_eq!(read_index(&out).plugins.len(), 3);
}

#[tokio::test]
async fn ac02_entry_fields_equal_the_declared_values() {
    let (_t, plugins, out) = tmp();
    write(
        &plugins,
        "multi-hosters/full/plugin.ts",
        r#"module.exports = {
    id: "full", name: "Full Plugin", version: "2.3.4", apiVersion: 1,
    description: "Does everything", author: "amigo-labs",
    pluginType: "multi-hoster",
    permissions: { domains: ["api.full.test", "*.cdn.full.test"] },
    urlPattern: "https?://full\\.test/.+",
    resolve(url: string) { return { name: "x", downloads: [] }; },
};
"#,
    );
    write(
        &plugins,
        "hosters/bare/plugin.ts",
        r#"module.exports = {
    id: "bare", name: "Bare", version: "0.1.0",
    urlPattern: "https?://bare\\.test/.+",
    resolve(url) { return { name: "x", downloads: [] }; },
};
"#,
    );

    let index = build(&plugins, &out, None).await.unwrap();
    let full = index.plugins.iter().find(|p| p.id == "full").unwrap();
    assert_eq!(full.name, "Full Plugin");
    assert_eq!(full.version, "2.3.4");
    assert_eq!(full.description, "Does everything");
    assert_eq!(full.author, "amigo-labs");
    assert_eq!(full.url_pattern, "https?://full\\.test/.+");
    assert_eq!(full.api_version, Some(1));
    assert_eq!(
        full.declared_domains().unwrap(),
        ["api.full.test", "*.cdn.full.test"]
    );
    assert_eq!(full.tags, ["multi-hoster"]);
    assert_eq!(full.min_app_version, None);

    let bare = index.plugins.iter().find(|p| p.id == "bare").unwrap();
    assert_eq!(bare.description, "");
    assert_eq!(bare.author, "");
    assert_eq!(bare.api_version, Some(1));
    assert!(bare.permissions.is_none());
    assert_eq!(bare.tags, ["hoster"]);
}

#[tokio::test]
async fn ac03_download_url_path_and_sha256_match_the_written_artifact() {
    let (_t, plugins, out) = tmp();
    let ts = plugin_src("alpha", "1.2.0");
    let js = plugin_src("beta", "0.9.0");
    write(&plugins, "hosters/alpha/plugin.ts", &ts);
    write(&plugins, "hosters/beta/plugin.js", &js);

    let index = build(&plugins, &out, None).await.unwrap();
    for (id, version, ext, src) in [("alpha", "1.2.0", "ts", &ts), ("beta", "0.9.0", "js", &js)] {
        let e = index.plugins.iter().find(|p| p.id == id).unwrap();
        let rel = format!("plugins/{id}/{version}/plugin.{ext}");
        assert_eq!(e.download_url, format!("{BASE}/{rel}"));
        let written = std::fs::read(out.join(&rel)).unwrap();
        assert_eq!(written, src.as_bytes());
        assert_eq!(e.sha256, hex::encode(Sha256::digest(&written)));
    }
}

async fn assert_build_fails(rel: &str, body: impl AsRef<[u8]>, expect_name: &str, expect: &str) {
    let (_t, plugins, out) = tmp();
    write(
        &plugins,
        "hosters/good/plugin.ts",
        plugin_src("good", "1.0.0"),
    );
    write(&plugins, rel, body);
    let err = build(&plugins, &out, None).await.unwrap_err();
    let text = failure_text(err);
    assert!(text.contains(expect_name), "{text}");
    assert!(text.contains(expect), "{text}");
    assert!(!out.exists(), "output must stay untouched on failure");
}

#[tokio::test]
async fn ac04_plugin_that_does_not_load_fails_the_build() {
    assert_build_fails(
        "hosters/broken/plugin.ts",
        "module.exports = { id: ",
        "broken",
        "does not load",
    )
    .await;
}

#[tokio::test]
async fn ac04_invalid_id_fails_the_build() {
    assert_build_fails(
        "hosters/bad/plugin.ts",
        plugin_src("Bad_Id", "1.0.0"),
        "Bad_Id",
        "invalid plugin id",
    )
    .await;
}

#[tokio::test]
async fn ac04_non_semver_version_fails_the_build() {
    assert_build_fails(
        "hosters/ver/plugin.ts",
        plugin_src("ver", "1.0"),
        "ver",
        "not SemVer",
    )
    .await;
}

#[tokio::test]
async fn ac04_unsupported_host_api_fails_the_build() {
    let src = plugin_src("future", "1.0.0").replace("apiVersion: 1", "apiVersion: 99");
    assert_build_fails("hosters/future/plugin.ts", src, "future", "host API 99").await;
}

#[tokio::test]
async fn ac04_invalid_url_pattern_fails_the_build() {
    let src = plugin_src("rx", "1.0.0").replace(r"https?://rx\\.test/.+", "(unclosed");
    assert_build_fails("hosters/rx/plugin.ts", src, "rx", "urlPattern").await;
}

#[tokio::test]
async fn ac04_non_utf8_source_fails_the_build() {
    let mut bytes = plugin_src("bin", "1.0.0").into_bytes();
    bytes.extend_from_slice(&[0xff, 0xfe, 0xfd]);
    assert_build_fails("hosters/bin/plugin.ts", bytes, "hosters/bin", "not UTF-8").await;
}

#[tokio::test]
async fn ac04_oversized_source_fails_the_build() {
    let mut src = plugin_src("huge", "1.0.0");
    src.push_str(&format!("// {}\n", "x".repeat(2 * 1024 * 1024)));
    assert_build_fails("hosters/huge/plugin.ts", src, "hosters/huge", "at most").await;
}

#[tokio::test]
async fn ac04_duplicate_id_fails_the_build_and_names_every_failure() {
    let (_t, plugins, out) = tmp();
    write(
        &plugins,
        "hosters/one/plugin.ts",
        plugin_src("same", "1.0.0"),
    );
    write(
        &plugins,
        "multi-hosters/two/plugin.ts",
        plugin_src("same", "1.0.0"),
    );
    write(&plugins, "hosters/ver/plugin.ts", plugin_src("ver", "x"));
    let text = failure_text(build(&plugins, &out, None).await.unwrap_err());
    assert!(
        text.contains("same") && text.contains("also declared"),
        "{text}"
    );
    assert!(text.contains("ver") && text.contains("SemVer"), "{text}");
    assert!(!out.exists());
}

/// Publish `version` of `alpha` (plus a stable `beta`) into a fresh registry
/// tree, returning it.
async fn published_registry(t: &Path, version: &str) -> PathBuf {
    let plugins = t.join("pub-src");
    write(
        &plugins,
        "hosters/alpha/plugin.ts",
        plugin_src("alpha", version),
    );
    write(
        &plugins,
        "hosters/beta/plugin.ts",
        plugin_src("beta", "1.0.0"),
    );
    let out = t.join("published");
    build(&plugins, &out, None).await.unwrap();
    out
}

#[tokio::test]
async fn ac05_raised_version_keeps_every_published_file() {
    let (t, plugins, out) = tmp();
    let published = published_registry(t.path(), "1.0.0").await;
    write(&published, "extra/keep.txt", "kept");
    write(
        &plugins,
        "hosters/alpha/plugin.ts",
        plugin_src("alpha", "1.1.0"),
    );
    write(
        &plugins,
        "hosters/beta/plugin.ts",
        plugin_src("beta", "1.0.0"),
    );

    let index = build(&plugins, &out, Some(&published)).await.unwrap();
    let alpha = index.plugins.iter().find(|p| p.id == "alpha").unwrap();
    assert_eq!(alpha.version, "1.1.0");
    for rel in [
        "plugins/alpha/1.0.0/plugin.ts",
        "plugins/beta/1.0.0/plugin.ts",
        "extra/keep.txt",
    ] {
        assert_eq!(
            std::fs::read(out.join(rel)).unwrap(),
            std::fs::read(published.join(rel)).unwrap(),
            "{rel}"
        );
    }
    assert!(out.join("plugins/alpha/1.1.0/plugin.ts").exists());
}

#[tokio::test]
async fn ac05_same_version_with_different_source_fails() {
    let (t, plugins, out) = tmp();
    let published = published_registry(t.path(), "1.0.0").await;
    let changed = plugin_src("alpha", "1.0.0").replace("Name alpha", "Renamed");
    write(&plugins, "hosters/alpha/plugin.ts", changed);
    write(
        &plugins,
        "hosters/beta/plugin.ts",
        plugin_src("beta", "1.0.0"),
    );
    let text = failure_text(build(&plugins, &out, Some(&published)).await.unwrap_err());
    assert!(
        text.contains("alpha") && text.contains("already published"),
        "{text}"
    );
    assert!(!out.exists());
}

#[tokio::test]
async fn ac05_lower_version_fails() {
    let (t, plugins, out) = tmp();
    let published = published_registry(t.path(), "2.0.0").await;
    write(
        &plugins,
        "hosters/alpha/plugin.ts",
        plugin_src("alpha", "1.9.0"),
    );
    write(
        &plugins,
        "hosters/beta/plugin.ts",
        plugin_src("beta", "1.0.0"),
    );
    let text = failure_text(build(&plugins, &out, Some(&published)).await.unwrap_err());
    assert!(text.contains("alpha") && text.contains("lower"), "{text}");
}

#[tokio::test]
async fn ac05_version_deleted_from_index_but_still_published_is_checked() {
    // A plugin removed from the index and later re-added at the same
    // version: its artifact still exists, so different bytes are refused.
    let (t, plugins, out) = tmp();
    let published = published_registry(t.path(), "1.0.0").await;
    let mut idx = read_index(&published);
    idx.plugins.retain(|p| p.id != "alpha");
    std::fs::write(
        published.join(INDEX_FILE),
        serde_json::to_vec(&idx).unwrap(),
    )
    .unwrap();
    write(
        &plugins,
        "hosters/alpha/plugin.ts",
        plugin_src("alpha", "1.0.0").replace("Name", "Other"),
    );
    let text = failure_text(build(&plugins, &out, Some(&published)).await.unwrap_err());
    assert!(text.contains("already published"), "{text}");
}

#[tokio::test]
async fn ac05_inconsistent_published_registry_fails() {
    for breakage in ["missing-index", "missing-artifact", "tampered-artifact"] {
        let (t, plugins, out) = tmp();
        let published = published_registry(t.path(), "1.0.0").await;
        match breakage {
            "missing-index" => std::fs::remove_file(published.join(INDEX_FILE)).unwrap(),
            "missing-artifact" => {
                std::fs::remove_file(published.join("plugins/beta/1.0.0/plugin.ts")).unwrap()
            }
            _ => write(&published, "plugins/beta/1.0.0/plugin.ts", "tampered"),
        }
        write(
            &plugins,
            "hosters/alpha/plugin.ts",
            plugin_src("alpha", "1.0.0"),
        );
        let err = build(&plugins, &out, Some(&published)).await.unwrap_err();
        assert!(
            matches!(err, RegistryBuildError::Published(_)),
            "{breakage}: {err}"
        );
        assert!(!out.exists(), "{breakage}");
    }
}

#[tokio::test]
async fn ac06_empty_plugin_tree_fails() {
    let (_t, plugins, out) = tmp();
    write(
        &plugins,
        "template/plugin.ts",
        plugin_src("template", "1.0.0"),
    );
    let err = build(&plugins, &out, None).await.unwrap_err();
    assert!(matches!(err, RegistryBuildError::NoPlugins(_)), "{err}");
    assert!(!out.join(INDEX_FILE).exists());
}

fn test_key() -> (String, String) {
    let seed = [7u8; 32];
    let key = SigningKey::from_bytes(&seed);
    (
        hex::encode(seed),
        hex::encode(key.verifying_key().to_bytes()),
    )
}

#[tokio::test]
async fn ac07_build_and_signature_are_deterministic() {
    let (t, plugins, out) = tmp();
    write(
        &plugins,
        "hosters/zeta/plugin.ts",
        plugin_src("zeta", "1.0.0"),
    );
    write(
        &plugins,
        "hosters/alpha/plugin.ts",
        plugin_src("alpha", "1.0.0"),
    );
    let out2 = t.path().join("out2");
    build(&plugins, &out, None).await.unwrap();
    build(&plugins, &out2, None).await.unwrap();
    let a = std::fs::read(out.join(INDEX_FILE)).unwrap();
    let b = std::fs::read(out2.join(INDEX_FILE)).unwrap();
    assert_eq!(a, b);
    let (seed, pubkey) = test_key();
    assert_eq!(
        sign_index(&a, Some(&seed), Some(&pubkey)).unwrap(),
        sign_index(&b, Some(&seed), Some(&pubkey)).unwrap()
    );
}

#[tokio::test]
async fn ac08_signed_index_is_accepted_by_the_registry_client() {
    let (_t, plugins, out) = tmp();
    write(
        &plugins,
        "hosters/alpha/plugin.ts",
        plugin_src("alpha", "1.0.0"),
    );
    build(&plugins, &out, None).await.unwrap();
    let (seed, pubkey) = test_key();
    sign_index_file(&out, Some(&seed), Some(&pubkey)).unwrap();

    let mock = MockServer::start().await;
    for file in [INDEX_FILE, SIGNATURE_FILE] {
        Mock::given(method("GET"))
            .and(path(format!("/{file}")))
            .respond_with(
                ResponseTemplate::new(200).set_body_bytes(std::fs::read(out.join(file)).unwrap()),
            )
            .mount(&mock)
            .await;
    }
    let mut verifier = [0u8; 32];
    verifier.copy_from_slice(&hex::decode(&pubkey).unwrap());
    let mut config = RegistryConfig::for_index(format!("{}/{INDEX_FILE}", mock.uri()), false)
        .with_index_verifier(verifier)
        .unwrap();
    config.cache_path = None;
    let index = refresh_index(&reqwest::Client::new(), &config)
        .await
        .unwrap();
    assert_eq!(index.plugins[0].id, "alpha");
}

#[test]
fn ac09_signing_refuses_bad_or_missing_key_material() {
    let (seed, pubkey) = test_key();
    let other = hex::encode(
        SigningKey::from_bytes(&[9u8; 32])
            .verifying_key()
            .to_bytes(),
    );
    let cases: [(Option<&str>, Option<&str>); 6] = [
        (Some(&seed), Some(&other)),
        (None, Some(&pubkey)),
        (Some(&seed), None),
        (Some("abc"), Some(&pubkey)),
        (Some(&seed), Some("zz")),
        (Some(""), Some(&pubkey)),
    ];
    for (s, p) in cases {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(INDEX_FILE), b"{}").unwrap();
        let err = sign_index_file(dir.path(), s, p).unwrap_err();
        assert!(
            matches!(err, RegistryBuildError::Signing(_)),
            "{s:?}/{p:?}: {err}"
        );
        assert!(!dir.path().join(SIGNATURE_FILE).exists());
    }
}
