//! Publisher side of the plugin registry: build `index.json` and the
//! versioned plugin artifacts from a plugin source tree, and sign the index.
//!
//! The client side ([`crate::registry`]) fetches what this module produces.
//! Every plugin is loaded with the real runtime ([`PluginLoader::inspect_plugin`])
//! so the index states exactly what a server will see, and a plugin that does
//! not load is never published. See `docs/specs/plugin-registry.md`.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};

use crate::loader::PluginLoader;
use crate::registry::{MAX_PLUGIN_ARTIFACT_BYTES, RegistryIndex, RegistryPlugin};
use crate::types::{PluginPermissions, PluginType};

/// Category directories under the plugin root that hold publishable plugins.
pub const PLUGIN_CATEGORIES: [&str; 3] = ["hosters", "multi-hosters", "extractors"];

/// Name of the index file at the registry root.
pub const INDEX_FILE: &str = "index.json";
/// Name of the detached signature next to the index.
pub const SIGNATURE_FILE: &str = "index.json.sig";

/// Errors of the registry build and signing steps.
#[derive(Debug, thiserror::Error)]
pub enum RegistryBuildError {
    /// One or more plugins cannot be published; each entry names the plugin
    /// (id or path) and the reason.
    #[error("{} plugin(s) cannot be published:\n{}", .0.len(), format_failures(.0))]
    Plugins(Vec<PluginFailure>),
    /// The plugin tree holds no plugin at all.
    #[error("no plugins found under {0}")]
    NoPlugins(String),
    /// The previously published registry is incomplete or does not match its
    /// own index.
    #[error("published registry is inconsistent: {0}")]
    Published(String),
    /// Signing key material is missing, malformed or does not match.
    #[error("signing failed: {0}")]
    Signing(String),
    /// Invalid arguments (base URL, output directory).
    #[error("{0}")]
    Invalid(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// A plugin that cannot be published, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginFailure {
    /// Plugin id when known, otherwise its source path.
    pub plugin: String,
    pub reason: String,
}

fn format_failures(failures: &[PluginFailure]) -> String {
    failures
        .iter()
        .map(|f| format!("  - {}: {}", f.plugin, f.reason))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Inputs of [`build_registry`].
#[derive(Debug, Clone)]
pub struct BuildOptions {
    /// Plugin root containing the category directories (`plugins/`).
    pub plugins_dir: PathBuf,
    /// Output directory; must not exist or be empty. Receives the complete
    /// site tree: every published file plus the new artifacts and index.
    pub out_dir: PathBuf,
    /// Absolute `https` URL the registry is served from, no trailing slash.
    pub base_url: String,
    /// Checkout of the previously published registry, if any.
    pub published: Option<PathBuf>,
}

/// What a successful build produced.
#[derive(Debug, Clone)]
pub struct BuildReport {
    pub index: RegistryIndex,
    /// Artifacts written for the first time (relative paths).
    pub new_artifacts: Vec<String>,
}

/// A plugin source found in the tree, before loading.
struct Candidate {
    path: PathBuf,
    ext: &'static str,
    bytes: Vec<u8>,
}

/// Find every plugin: a directory under one of [`PLUGIN_CATEGORIES`] holding
/// `plugin.ts` or `plugin.js` (`.ts` wins when both exist, like the loader).
fn find_candidates(plugins_dir: &Path) -> Result<Vec<(PathBuf, &'static str)>, std::io::Error> {
    let mut found = Vec::new();
    for category in PLUGIN_CATEGORIES {
        let dir = plugins_dir.join(category);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut dirs: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort();
        for dir in dirs {
            for ext in ["ts", "js"] {
                let file = dir.join(format!("plugin.{ext}"));
                if file.is_file() {
                    found.push((file, ext));
                    break;
                }
            }
        }
    }
    Ok(found)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn validate_base_url(base_url: &str) -> Result<(), RegistryBuildError> {
    let parsed = url::Url::parse(base_url)
        .map_err(|e| RegistryBuildError::Invalid(format!("invalid base URL {base_url:?}: {e}")))?;
    if parsed.scheme() != "https" || base_url.ends_with('/') || parsed.host_str().is_none() {
        return Err(RegistryBuildError::Invalid(format!(
            "base URL must be an absolute https URL without a trailing slash, got {base_url:?}"
        )));
    }
    Ok(())
}

/// Relative path of an artifact inside the registry tree.
pub fn artifact_path(id: &str, version: &str, ext: &str) -> String {
    format!("plugins/{id}/{version}/plugin.{ext}")
}

fn plugin_type_tag(t: &PluginType) -> &'static str {
    match t {
        PluginType::MultiHoster => "multi-hoster",
        PluginType::Hoster => "hoster",
        PluginType::Generic => "generic",
    }
}

/// The previously published registry, verified against its own index.
struct Published {
    root: PathBuf,
    /// Published (index) version per plugin id.
    versions: HashMap<String, semver::Version>,
}

impl Published {
    fn load(root: &Path) -> Result<Self, RegistryBuildError> {
        let index_path = root.join(INDEX_FILE);
        let raw = std::fs::read(&index_path).map_err(|e| {
            RegistryBuildError::Published(format!("cannot read {}: {e}", index_path.display()))
        })?;
        let index: RegistryIndex = serde_json::from_slice(&raw)
            .map_err(|e| RegistryBuildError::Published(format!("invalid {INDEX_FILE}: {e}")))?;
        let mut versions = HashMap::new();
        for entry in &index.plugins {
            let rel = url::Url::parse(&entry.download_url)
                .map(|u| u.path().trim_start_matches('/').to_string())
                .map_err(|e| {
                    RegistryBuildError::Published(format!(
                        "{}: invalid download_url {:?}: {e}",
                        entry.id, entry.download_url
                    ))
                })?;
            let bytes = std::fs::read(root.join(&rel)).map_err(|_| {
                RegistryBuildError::Published(format!("{}: artifact {rel} is missing", entry.id))
            })?;
            if sha256_hex(&bytes) != entry.sha256.to_lowercase() {
                return Err(RegistryBuildError::Published(format!(
                    "{}: artifact {rel} does not match its sha256",
                    entry.id
                )));
            }
            let version = semver::Version::parse(&entry.version).map_err(|e| {
                RegistryBuildError::Published(format!(
                    "{}: invalid version {:?}: {e}",
                    entry.id, entry.version
                ))
            })?;
            versions.insert(entry.id.clone(), version);
        }
        Ok(Self {
            root: root.to_path_buf(),
            versions,
        })
    }

    /// Bytes of an already published artifact of `id`@`version`, any extension.
    fn artifact(&self, id: &str, version: &str) -> Option<Vec<u8>> {
        ["ts", "js"]
            .iter()
            .find_map(|ext| std::fs::read(self.root.join(artifact_path(id, version, ext))).ok())
    }
}

/// Build the registry. Validates everything first; the output directory is
/// only written when every plugin passes.
pub async fn build_registry(
    loader: &PluginLoader,
    opts: &BuildOptions,
) -> Result<BuildReport, RegistryBuildError> {
    validate_base_url(&opts.base_url)?;
    if opts.out_dir.exists() && std::fs::read_dir(&opts.out_dir)?.next().is_some() {
        return Err(RegistryBuildError::Invalid(format!(
            "output directory {} is not empty",
            opts.out_dir.display()
        )));
    }

    let published = opts.published.as_deref().map(Published::load).transpose()?;

    let found = find_candidates(&opts.plugins_dir)?;
    if found.is_empty() {
        return Err(RegistryBuildError::NoPlugins(
            opts.plugins_dir.display().to_string(),
        ));
    }

    let mut failures = Vec::new();
    // id -> (entry, candidate); BTreeMap orders the index by id.
    let mut entries: BTreeMap<String, (RegistryPlugin, Candidate)> = BTreeMap::new();
    let mut seen_paths: HashMap<String, PathBuf> = HashMap::new();

    for (path, ext) in found {
        let shown = path.display().to_string();
        let fail = |reason: String| PluginFailure {
            plugin: shown.clone(),
            reason,
        };
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                failures.push(fail(format!("cannot read source: {e}")));
                continue;
            }
        };
        if bytes.len() > MAX_PLUGIN_ARTIFACT_BYTES {
            failures.push(fail(format!(
                "source is {} bytes, clients accept at most {MAX_PLUGIN_ARTIFACT_BYTES}",
                bytes.len()
            )));
            continue;
        }
        if std::str::from_utf8(&bytes).is_err() {
            failures.push(fail("source is not UTF-8".into()));
            continue;
        }
        let meta = match loader.inspect_plugin(&path).await {
            Ok(m) => m,
            Err(e) => {
                failures.push(fail(format!("does not load: {e}")));
                continue;
            }
        };
        let id = meta.id.clone();
        let named = |reason: String| PluginFailure {
            plugin: format!("{id} ({shown})"),
            reason,
        };
        if let Err(e) = crate::registry::validate_plugin_id(&id) {
            failures.push(named(e.to_string()));
            continue;
        }
        let version = match semver::Version::parse(&meta.version) {
            Ok(v) => v,
            Err(e) => {
                failures.push(named(format!(
                    "version {:?} is not SemVer: {e}",
                    meta.version
                )));
                continue;
            }
        };
        if !crate::is_supported_api_version(meta.api_version) {
            failures.push(named(format!(
                "requires host API {}, this build supports {}",
                meta.api_version,
                crate::HOST_API_VERSION
            )));
            continue;
        }
        if let Err(e) = regex::Regex::new(&meta.url_pattern) {
            failures.push(named(format!("urlPattern is not a valid regex: {e}")));
            continue;
        }
        if let Some(other) = seen_paths.get(&id) {
            failures.push(named(format!("id is also declared by {}", other.display())));
            entries.remove(&id);
            continue;
        }
        seen_paths.insert(id.clone(), path.clone());

        if let Some(published) = &published {
            if let Some(prev) = published.versions.get(&id)
                && version < *prev
            {
                failures.push(named(format!(
                    "version {version} is lower than the published {prev}"
                )));
                continue;
            }
            if let Some(prev_bytes) = published.artifact(&id, &meta.version)
                && prev_bytes != bytes
            {
                failures.push(named(format!(
                    "version {} is already published with a different source; raise the version",
                    meta.version
                )));
                continue;
            }
        }

        let rel = artifact_path(&id, &meta.version, ext);
        let entry = RegistryPlugin {
            id: id.clone(),
            name: meta.name.clone(),
            version: meta.version.clone(),
            description: meta.description.clone().unwrap_or_default(),
            author: meta.author.clone().unwrap_or_default(),
            url_pattern: meta.url_pattern.clone(),
            min_app_version: None,
            sha256: sha256_hex(&bytes),
            download_url: format!("{}/{rel}", opts.base_url),
            tags: vec![plugin_type_tag(&meta.plugin_type).to_string()],
            api_version: Some(meta.api_version),
            permissions: meta
                .permissions
                .domains
                .as_ref()
                .map(|d| PluginPermissions {
                    domains: Some(d.clone()),
                }),
        };
        entries.insert(id, (entry, Candidate { path, ext, bytes }));
    }

    if !failures.is_empty() {
        return Err(RegistryBuildError::Plugins(failures));
    }

    // Everything validated: write the output.
    std::fs::create_dir_all(&opts.out_dir)?;
    if let Some(published) = &published {
        copy_tree(&published.root, &opts.out_dir, true)?;
    }
    let mut new_artifacts = Vec::new();
    for (entry, candidate) in entries.values() {
        let rel = artifact_path(&entry.id, &entry.version, candidate.ext);
        let dest = opts.out_dir.join(&rel);
        if dest.exists() {
            continue; // identical bytes, checked above
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&dest, &candidate.bytes)?;
        tracing::debug!("new artifact {rel} from {}", candidate.path.display());
        new_artifacts.push(rel);
    }

    let index = RegistryIndex {
        schema_version: 1,
        plugins: entries.into_values().map(|(e, _)| e).collect(),
    };
    std::fs::write(opts.out_dir.join(INDEX_FILE), index_bytes(&index))?;

    Ok(BuildReport {
        index,
        new_artifacts,
    })
}

/// The exact bytes written as `index.json`: pretty JSON plus a newline.
pub fn index_bytes(index: &RegistryIndex) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(index).expect("index serializes");
    bytes.push(b'\n');
    bytes
}

/// Copy every published file except `.git`, the index and its signature
/// (those are regenerated).
fn copy_tree(from: &Path, to: &Path, root: bool) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".git" || (root && (name == INDEX_FILE || name == SIGNATURE_FILE)) {
            continue;
        }
        let dest = to.join(&name);
        if entry.file_type()?.is_dir() {
            std::fs::create_dir_all(&dest)?;
            copy_tree(&entry.path(), &dest, false)?;
        } else {
            std::fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}

fn parse_hex32(name: &str, value: Option<&str>) -> Result<[u8; 32], RegistryBuildError> {
    let value = value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| RegistryBuildError::Signing(format!("{name} is not set")))?;
    let bytes = hex::decode(value)
        .ok()
        .filter(|b| b.len() == 32)
        .ok_or_else(|| RegistryBuildError::Signing(format!("{name} must be 64 hex characters")))?;
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

/// Sign `index` with the Ed25519 `seed` after checking that the seed derives
/// `pinned` — the key the binaries verify with. Returns the lowercase hex
/// signature, the format [`crate::registry::verify_ed25519`] reads.
pub fn sign_index(
    index: &[u8],
    seed_hex: Option<&str>,
    pinned_hex: Option<&str>,
) -> Result<String, RegistryBuildError> {
    let seed = parse_hex32("signing seed", seed_hex)?;
    let pinned = parse_hex32("pinned public key", pinned_hex)?;
    let key = SigningKey::from_bytes(&seed);
    let derived: VerifyingKey = key.verifying_key();
    if derived.to_bytes() != pinned {
        return Err(RegistryBuildError::Signing(format!(
            "the signing seed does not match the pinned public key (derived {}, pinned {})",
            hex::encode(derived.to_bytes()),
            hex::encode(pinned)
        )));
    }
    Ok(hex::encode(key.sign(index).to_bytes()))
}

/// Sign `<dir>/index.json` and write `<dir>/index.json.sig`. Nothing is
/// written when signing fails.
pub fn sign_index_file(
    dir: &Path,
    seed_hex: Option<&str>,
    pinned_hex: Option<&str>,
) -> Result<(), RegistryBuildError> {
    let index = std::fs::read(dir.join(INDEX_FILE))?;
    let signature = sign_index(&index, seed_hex, pinned_hex)?;
    std::fs::write(dir.join(SIGNATURE_FILE), format!("{signature}\n"))?;
    Ok(())
}

/// Check `<dir>/index.json.sig` against `<dir>/index.json` with the pinned
/// public key.
pub fn verify_index_file(dir: &Path, pinned_hex: Option<&str>) -> Result<(), RegistryBuildError> {
    let pinned = parse_hex32("pinned public key", pinned_hex)?;
    let index = std::fs::read(dir.join(INDEX_FILE))?;
    let signature = std::fs::read_to_string(dir.join(SIGNATURE_FILE))?;
    crate::registry::verify_ed25519(&index, &signature, &pinned)
        .map_err(|e| RegistryBuildError::Signing(format!("index signature does not verify: {e}")))
}

/// Public key (hex) derived from an Ed25519 signing seed (hex).
pub fn public_key_hex(seed_hex: &str) -> Result<String, RegistryBuildError> {
    let seed = parse_hex32("signing seed", Some(seed_hex))?;
    Ok(hex::encode(
        SigningKey::from_bytes(&seed).verifying_key().to_bytes(),
    ))
}
