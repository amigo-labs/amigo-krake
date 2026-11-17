//! `krake plugins registry …` — maintainer commands that build, sign, check
//! and publish the plugin registry (docs/specs/plugin-registry.md). Hidden
//! from the user-facing help; the publish workflow and the pull-request check
//! call them.
//!
//! The published registry is the branch `registry` of this repository, served
//! as static files at the base URL. Publishing checks that branch out into a
//! temporary git worktree, builds the full site tree on top of it, signs the
//! index and pushes one commit. A plain (non-force) push means a run that lost
//! a race fails instead of overwriting the other run's commit.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, bail};
use clap::Subcommand;
use krake_plugin_runtime::loader::PluginLoader;
use krake_plugin_runtime::registry_build::{
    self, BuildOptions, BuildReport, sign_index_file, verify_index_file,
};
use krake_plugin_runtime::sandbox::SandboxLimits;

/// Where the registry is served unless `--base-url` says otherwise.
pub const DEFAULT_BASE_URL: &str = "https://krake-registry.amigo-labs.dev";
/// Environment variable holding the Ed25519 signing seed (hex).
pub const SIGNING_KEY_ENV: &str = "KRAKE_UPDATE_SIGNING_KEY";
/// Environment variable holding the pinned public key (hex).
pub const PUBKEY_ENV: &str = "KRAKE_UPDATE_PUBKEY_HEX";

#[derive(Subcommand)]
pub enum RegistryAction {
    /// Build index.json and the versioned artifacts into an empty directory.
    Build {
        #[arg(long, default_value = "plugins")]
        plugins: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value = DEFAULT_BASE_URL)]
        base_url: String,
        /// Checkout of the previously published registry.
        #[arg(long)]
        published: Option<PathBuf>,
    },
    /// Sign <dir>/index.json into <dir>/index.json.sig
    /// ($KRAKE_UPDATE_SIGNING_KEY, $KRAKE_UPDATE_PUBKEY_HEX).
    Sign { dir: PathBuf },
    /// Check the plugin tree against the published branch without publishing:
    /// fails when a plugin changed without a higher version.
    Check {
        #[arg(long, default_value = "plugins")]
        plugins: PathBuf,
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        #[arg(long, default_value = "origin")]
        remote: String,
        #[arg(long, default_value = "registry")]
        branch: String,
        #[arg(long, default_value = DEFAULT_BASE_URL)]
        base_url: String,
    },
    /// Build, sign and push the registry to the published branch.
    Publish {
        #[arg(long, default_value = "plugins")]
        plugins: PathBuf,
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        #[arg(long, default_value = "origin")]
        remote: String,
        #[arg(long, default_value = "registry")]
        branch: String,
        #[arg(long, default_value = DEFAULT_BASE_URL)]
        base_url: String,
        /// Commit message for the registry commit.
        #[arg(long, default_value = "registry: publish plugins")]
        message: String,
    },
}

pub async fn run(action: RegistryAction) -> anyhow::Result<()> {
    match action {
        RegistryAction::Build {
            plugins,
            out,
            base_url,
            published,
        } => {
            let report = build(&plugins, &out, &base_url, published.as_deref()).await?;
            print_report(&report);
        }
        RegistryAction::Sign { dir } => {
            sign_index_file(
                &dir,
                env(SIGNING_KEY_ENV).as_deref(),
                env(PUBKEY_ENV).as_deref(),
            )?;
            println!("Signed {}", dir.join(registry_build::INDEX_FILE).display());
        }
        RegistryAction::Check {
            plugins,
            repo,
            remote,
            branch,
            base_url,
        } => {
            let checkout = Checkout::prepare(&repo, &remote, &branch)?;
            if !checkout.existed {
                println!("Branch '{branch}' is not published yet; every plugin is new.");
            }
            let scratch = tempfile::tempdir()?;
            let report = build(
                &plugins,
                &scratch.path().join("out"),
                &base_url,
                checkout.existed.then_some(checkout.dir()),
            )
            .await?;
            print_report(&report);
            println!("Registry check passed.");
        }
        RegistryAction::Publish {
            plugins,
            repo,
            remote,
            branch,
            base_url,
            message,
        } => {
            let seed = env(SIGNING_KEY_ENV);
            let pinned = env(PUBKEY_ENV);
            let checkout = Checkout::prepare(&repo, &remote, &branch)?;
            let scratch = tempfile::tempdir()?;
            let out = scratch.path().join("out");
            let report = build(
                &plugins,
                &out,
                &base_url,
                checkout.existed.then_some(checkout.dir()),
            )
            .await?;
            print_report(&report);
            sign_index_file(&out, seed.as_deref(), pinned.as_deref())?;
            match checkout.commit_and_push(&out, pinned.as_deref(), &message)? {
                Some(sha) => println!("Published {sha} to '{branch}'."),
                None => println!("Registry unchanged; nothing to publish."),
            }
        }
    }
    Ok(())
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

async fn build(
    plugins: &Path,
    out: &Path,
    base_url: &str,
    published: Option<&Path>,
) -> anyhow::Result<BuildReport> {
    let loader = PluginLoader::new(plugins.to_path_buf(), SandboxLimits::default())
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let opts = BuildOptions {
        plugins_dir: plugins.to_path_buf(),
        out_dir: out.to_path_buf(),
        base_url: base_url.to_string(),
        published: published.map(Path::to_path_buf),
    };
    Ok(registry_build::build_registry(&loader, &opts).await?)
}

fn print_report(report: &BuildReport) {
    for p in &report.index.plugins {
        println!("  {} {}", p.id, p.version);
    }
    for rel in &report.new_artifacts {
        println!("  new: {rel}");
    }
}

/// Run git in `dir`; stdout on success, an error with stderr otherwise.
fn git(dir: &Path, args: &[&str]) -> anyhow::Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .with_context(|| format!("running git {}", args.join(" ")))?;
    if !out.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The published branch checked out into a temporary worktree of `repo`.
/// Removed again on drop.
pub struct Checkout {
    repo: PathBuf,
    remote: String,
    branch: String,
    /// The branch existed on the remote when it was read.
    pub existed: bool,
    orphan: Option<String>,
    _tmp: tempfile::TempDir,
    worktree: PathBuf,
}

impl Checkout {
    pub fn prepare(repo: &Path, remote: &str, branch: &str) -> anyhow::Result<Self> {
        let tmp = tempfile::tempdir()?;
        let worktree = tmp.path().join("registry");
        let wt = worktree
            .to_str()
            .context("non-UTF-8 temp path")?
            .to_string();

        let probe = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["ls-remote", "--exit-code", "--heads", remote, branch])
            .output()
            .context("running git ls-remote")?;
        let existed = match probe.status.code() {
            Some(0) => true,
            Some(2) => false,
            _ => bail!(
                "cannot read branch '{branch}' from '{remote}': {}",
                String::from_utf8_lossy(&probe.stderr).trim()
            ),
        };

        let mut orphan = None;
        if existed {
            git(
                repo,
                &[
                    "fetch",
                    "--no-tags",
                    remote,
                    &format!("refs/heads/{branch}"),
                ],
            )?;
            git(repo, &["worktree", "add", "--detach", &wt, "FETCH_HEAD"])?;
        } else {
            git(repo, &["worktree", "add", "--detach", &wt, "HEAD"])?;
            let name = format!("{branch}-publish-{}", std::process::id());
            git(&worktree, &["switch", "--orphan", &name])?;
            orphan = Some(name);
        }

        Ok(Self {
            repo: repo.to_path_buf(),
            remote: remote.to_string(),
            branch: branch.to_string(),
            existed,
            orphan,
            _tmp: tmp,
            worktree,
        })
    }

    /// The checked-out published tree.
    pub fn dir(&self) -> &Path {
        &self.worktree
    }

    /// Replace the worktree with `site`, commit and push it. The signature in
    /// `site` must verify against `pinned` before anything is committed.
    /// Returns the new commit, or `None` when nothing changed.
    pub fn commit_and_push(
        &self,
        site: &Path,
        pinned: Option<&str>,
        message: &str,
    ) -> anyhow::Result<Option<String>> {
        verify_index_file(site, pinned)?;

        for entry in std::fs::read_dir(&self.worktree)? {
            let entry = entry?;
            if entry.file_name() == ".git" {
                continue;
            }
            if entry.file_type()?.is_dir() {
                std::fs::remove_dir_all(entry.path())?;
            } else {
                std::fs::remove_file(entry.path())?;
            }
        }
        copy_dir(site, &self.worktree)?;

        git(&self.worktree, &["add", "-A"])?;
        let unchanged = Command::new("git")
            .arg("-C")
            .arg(&self.worktree)
            .args(["diff", "--cached", "--quiet"])
            .status()?
            .success();
        if unchanged && self.existed {
            return Ok(None);
        }
        git(
            &self.worktree,
            &[
                "-c",
                "user.name=krake-registry",
                "-c",
                "user.email=41898282+github-actions[bot]@users.noreply.github.com",
                "commit",
                "--quiet",
                "-m",
                message,
            ],
        )?;
        let sha = git(&self.worktree, &["rev-parse", "HEAD"])?;
        // No force: a branch that moved since `prepare` rejects the push.
        git(
            &self.worktree,
            &[
                "push",
                "--quiet",
                &self.remote,
                &format!("HEAD:refs/heads/{}", self.branch),
            ],
        )?;
        Ok(Some(sha))
    }
}

impl Drop for Checkout {
    fn drop(&mut self) {
        let wt = self.worktree.to_string_lossy().to_string();
        let _ = git(&self.repo, &["worktree", "remove", "--force", &wt]);
        if let Some(name) = &self.orphan {
            let _ = git(&self.repo, &["branch", "-D", name]);
        }
    }
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            std::fs::create_dir_all(&dest)?;
            copy_dir(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use krake_plugin_runtime::registry_build::{INDEX_FILE, SIGNATURE_FILE, public_key_hex};

    const SEED: &str = "0707070707070707070707070707070707070707070707070707070707070707";

    fn sh(dir: &Path, args: &[&str]) -> String {
        git(dir, args).unwrap()
    }

    /// A bare "origin" and a clone with one commit on main.
    fn origin_and_clone() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let t = tempfile::tempdir().unwrap();
        let origin = t.path().join("origin.git");
        let clone = t.path().join("clone");
        std::fs::create_dir_all(&origin).unwrap();
        sh(&origin, &["init", "--quiet", "--bare"]);
        sh(
            t.path(),
            &["clone", "--quiet", origin.to_str().unwrap(), "clone"],
        );
        std::fs::write(clone.join("README"), "main").unwrap();
        sh(&clone, &["add", "README"]);
        sh(
            &clone,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "--quiet",
                "-m",
                "init",
            ],
        );
        sh(
            &clone,
            &["push", "--quiet", "origin", "HEAD:refs/heads/main"],
        );
        (t, origin, clone)
    }

    /// A signed site tree with the given artifact files.
    fn site(root: &Path, artifacts: &[(&str, &str)], index: &str) -> PathBuf {
        let dir = root.join(format!("site-{}", index.len()));
        let _ = std::fs::remove_dir_all(&dir);
        for (rel, body) in artifacts {
            let p = dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        }
        std::fs::write(dir.join(INDEX_FILE), index).unwrap();
        let pubkey = public_key_hex(SEED).unwrap();
        sign_index_file(&dir, Some(SEED), Some(&pubkey)).unwrap();
        dir
    }

    fn files_on(origin: &Path, branch: &str) -> Vec<String> {
        let out = sh(origin, &["ls-tree", "-r", "--name-only", branch]);
        out.lines().map(str::to_string).collect()
    }

    fn commit_count(origin: &Path, branch: &str) -> usize {
        sh(origin, &["rev-list", "--count", branch])
            .parse()
            .unwrap()
    }

    #[test]
    fn ac10_publish_creates_extends_and_skips_unchanged() {
        let (t, origin, clone) = origin_and_clone();
        let pubkey = public_key_hex(SEED).unwrap();

        // 1. Branch missing: created with one commit holding everything.
        let v1 = site(
            t.path(),
            &[("plugins/a/1.0.0/plugin.ts", "v1")],
            "{\"v\":1}",
        );
        let c = Checkout::prepare(&clone, "origin", "registry").unwrap();
        assert!(!c.existed);
        assert!(
            c.commit_and_push(&v1, Some(&pubkey), "publish")
                .unwrap()
                .is_some()
        );
        drop(c);
        assert_eq!(commit_count(&origin, "registry"), 1);
        assert_eq!(
            files_on(&origin, "registry"),
            [INDEX_FILE, SIGNATURE_FILE, "plugins/a/1.0.0/plugin.ts"]
        );

        // 2. Version raised: one more commit, earlier artifact kept.
        let v2 = site(
            t.path(),
            &[
                ("plugins/a/1.0.0/plugin.ts", "v1"),
                ("plugins/a/1.1.0/plugin.ts", "v2"),
            ],
            "{\"v\":22}",
        );
        let c = Checkout::prepare(&clone, "origin", "registry").unwrap();
        assert!(c.existed);
        assert_eq!(
            std::fs::read_to_string(c.dir().join("plugins/a/1.0.0/plugin.ts")).unwrap(),
            "v1"
        );
        assert!(
            c.commit_and_push(&v2, Some(&pubkey), "publish")
                .unwrap()
                .is_some()
        );
        drop(c);
        assert_eq!(commit_count(&origin, "registry"), 2);
        let changed = sh(
            &origin,
            &["diff", "--name-status", "registry~1", "registry"],
        );
        assert_eq!(
            changed.lines().collect::<Vec<_>>(),
            [
                "M\tindex.json",
                "M\tindex.json.sig",
                "A\tplugins/a/1.1.0/plugin.ts"
            ]
        );

        // 3. Nothing differs: no commit.
        let c = Checkout::prepare(&clone, "origin", "registry").unwrap();
        assert!(
            c.commit_and_push(&v2, Some(&pubkey), "publish")
                .unwrap()
                .is_none()
        );
        drop(c);
        assert_eq!(commit_count(&origin, "registry"), 2);
        // Worktrees are cleaned up.
        assert_eq!(sh(&clone, &["worktree", "list"]).lines().count(), 1);
    }

    #[test]
    fn ac11_moved_branch_fails_without_overwriting_then_rerun_succeeds() {
        let (t, origin, clone) = origin_and_clone();
        let pubkey = public_key_hex(SEED).unwrap();
        let v1 = site(
            t.path(),
            &[("plugins/a/1.0.0/plugin.ts", "v1")],
            "{\"v\":1}",
        );
        Checkout::prepare(&clone, "origin", "registry")
            .unwrap()
            .commit_and_push(&v1, Some(&pubkey), "publish")
            .unwrap();

        // Run A reads the branch; run B pushes first.
        let a = Checkout::prepare(&clone, "origin", "registry").unwrap();
        let v2 = site(
            t.path(),
            &[
                ("plugins/a/1.0.0/plugin.ts", "v1"),
                ("plugins/b/1.0.0/plugin.ts", "b"),
            ],
            "{\"v\":222}",
        );
        let b = Checkout::prepare(&clone, "origin", "registry").unwrap();
        b.commit_and_push(&v2, Some(&pubkey), "publish b").unwrap();
        drop(b);
        let tip = sh(&origin, &["rev-parse", "registry"]);

        let v3 = site(
            t.path(),
            &[
                ("plugins/a/1.0.0/plugin.ts", "v1"),
                ("plugins/a/2.0.0/plugin.ts", "v3"),
            ],
            "{\"v\":3333}",
        );
        assert!(a.commit_and_push(&v3, Some(&pubkey), "publish a").is_err());
        drop(a);
        assert_eq!(sh(&origin, &["rev-parse", "registry"]), tip);

        // Rerun builds on the new tip.
        let again = Checkout::prepare(&clone, "origin", "registry").unwrap();
        assert!(again.dir().join("plugins/b/1.0.0/plugin.ts").exists());
        assert!(
            again
                .commit_and_push(&v3, Some(&pubkey), "publish a")
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn ac11_unverifiable_signature_is_never_committed() {
        let (t, origin, clone) = origin_and_clone();
        let pubkey = public_key_hex(SEED).unwrap();
        let v1 = site(
            t.path(),
            &[("plugins/a/1.0.0/plugin.ts", "v1")],
            "{\"v\":1}",
        );
        // Index changed after signing: the signature no longer matches.
        std::fs::write(v1.join(INDEX_FILE), "{\"v\":2}").unwrap();
        let c = Checkout::prepare(&clone, "origin", "registry").unwrap();
        assert!(c.commit_and_push(&v1, Some(&pubkey), "publish").is_err());
        drop(c);
        assert!(git(&origin, &["rev-parse", "--verify", "registry"]).is_err());
    }
}
