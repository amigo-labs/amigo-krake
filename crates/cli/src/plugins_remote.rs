//! `krake plugins search` / `krake plugins update` against the paired server
//! (docs/specs/plugin-registry.md, AC-18 to AC-21).
//!
//! Both read the server's registry view: `GET /api/v1/updates/plugins/available`
//! (fails with 503 when the registry is unavailable) and, for updates,
//! `GET /api/v1/updates/check`. Updates are applied one plugin at a time via
//! `POST /api/v1/updates/plugins/{id}`, so each widened-access update can be
//! approved separately and every result is reported.

use anyhow::{Context, bail};
use serde::Deserialize;

/// A paired server to talk to.
pub struct Target {
    pub name: String,
    pub base: String,
    pub token: String,
}

/// The paired server named `alias`, or the default one.
pub fn target(all: &crate::remotes::Remotes, alias: Option<&str>) -> anyhow::Result<Target> {
    let Some((name, remote)) = crate::remotes::resolve(all, alias) else {
        bail!("No paired server. Pair with `krake login <url>` first, or pass --remote <alias>.");
    };
    Ok(Target {
        base: remote.url.trim_end_matches('/').to_string(),
        name,
        token: remote.token,
    })
}

#[derive(Debug, Clone, Deserialize)]
pub struct AvailablePlugin {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub installed: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct CheckResponse {
    #[serde(default)]
    plugins: Vec<PendingUpdate>,
}

#[derive(Debug, Clone, Deserialize)]
struct PendingUpdate {
    plugin_id: String,
    current_version: Option<String>,
    available_version: String,
    #[serde(default)]
    is_new: bool,
    #[serde(default)]
    added_domains: Option<Vec<String>>,
    #[serde(default)]
    requires_approval: bool,
}

/// What `update` did, per plugin.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct UpdateSummary {
    /// (id, old version, new version)
    pub updated: Vec<(String, String, String)>,
    /// (id, added domains) — widened access that was not approved.
    pub skipped: Vec<(String, Vec<String>)>,
    /// (id, error)
    pub failed: Vec<(String, String)>,
}

/// Decides whether an update that widens network access may be applied.
pub trait Approver {
    fn approve(&mut self, id: &str, added: &[String]) -> bool;
}

fn client() -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder().user_agent("krake").build()?)
}

async fn available(http: &reqwest::Client, t: &Target) -> anyhow::Result<Vec<AvailablePlugin>> {
    let resp = http
        .get(format!("{}/api/v1/updates/plugins/available", t.base))
        .bearer_auth(&t.token)
        .send()
        .await
        .with_context(|| format!("cannot reach server '{}' at {}", t.name, t.base))?;
    let status = resp.status();
    if status == reqwest::StatusCode::SERVICE_UNAVAILABLE
        || status == reqwest::StatusCode::BAD_GATEWAY
    {
        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        bail!(
            "the plugin registry of '{}' is unavailable: {}",
            t.name,
            body["error"].as_str().unwrap_or("no details")
        );
    }
    if !status.is_success() {
        bail!("server '{}' answered HTTP {status}", t.name);
    }
    Ok(resp.json().await?)
}

/// Registry plugins whose id, name, description or a tag contains `query`
/// (case-insensitive), ordered by id.
pub async fn search(t: &Target, query: &str) -> anyhow::Result<Vec<AvailablePlugin>> {
    let q = query.to_lowercase();
    let mut hits: Vec<_> = available(&client()?, t)
        .await?
        .into_iter()
        .filter(|p| {
            p.id.to_lowercase().contains(&q)
                || p.name.to_lowercase().contains(&q)
                || p.description.to_lowercase().contains(&q)
                || p.tags.iter().any(|tag| tag.to_lowercase().contains(&q))
        })
        .collect();
    hits.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(hits)
}

/// Print search results.
pub fn print_search(hits: &[AvailablePlugin]) {
    if hits.is_empty() {
        println!("No plugins match.");
        return;
    }
    for p in hits {
        let mark = if p.installed { "  [installed]" } else { "" };
        println!("{:<24} {:<10} {}{mark}", p.id, p.version, p.name);
    }
}

/// Update `id`, or every installed plugin with a newer registry version.
pub async fn update(
    t: &Target,
    id: Option<&str>,
    approver: &mut dyn Approver,
) -> anyhow::Result<UpdateSummary> {
    let http = client()?;
    // Fails with a clear message when the registry is unavailable.
    let registry = available(&http, t).await?;

    let check: CheckResponse = http
        .get(format!("{}/api/v1/updates/check", t.base))
        .bearer_auth(&t.token)
        .send()
        .await
        .with_context(|| format!("cannot reach server '{}' at {}", t.name, t.base))?
        .error_for_status()?
        .json()
        .await?;
    let mut pending: Vec<PendingUpdate> = check.plugins.into_iter().filter(|u| !u.is_new).collect();

    if let Some(id) = id {
        match registry.iter().find(|p| p.id == id) {
            None => bail!("plugin {id} is not in the registry of '{}'", t.name),
            Some(p) if !p.installed => bail!("plugin {id} is not installed on '{}'", t.name),
            _ => {}
        }
        pending.retain(|u| u.plugin_id == id);
        if pending.is_empty() {
            bail!(
                "plugin {id} has no newer version in the registry of '{}'",
                t.name
            );
        }
    }

    let mut summary = UpdateSummary::default();
    for u in pending {
        let added = u.added_domains.clone().unwrap_or_default();
        let approved = if u.requires_approval {
            if !approver.approve(&u.plugin_id, &added) {
                summary.skipped.push((u.plugin_id, added));
                continue;
            }
            true
        } else {
            false
        };
        let resp = http
            .post(format!(
                "{}/api/v1/updates/plugins/{}",
                t.base,
                crate::urlencoding_path(&u.plugin_id)
            ))
            .bearer_auth(&t.token)
            .json(&serde_json::json!({ "approve_permissions": approved }))
            .send()
            .await;
        let old = u.current_version.clone().unwrap_or_else(|| "?".into());
        match resp {
            Ok(r) if r.status().is_success() => {
                let body: serde_json::Value = r.json().await.unwrap_or_default();
                let new = body["version"]
                    .as_str()
                    .unwrap_or(&u.available_version)
                    .to_string();
                summary.updated.push((u.plugin_id, old, new));
            }
            Ok(r) => {
                let status = r.status();
                let body: serde_json::Value = r.json().await.unwrap_or_default();
                let err = body["error"].as_str().unwrap_or("unknown error");
                summary
                    .failed
                    .push((u.plugin_id, format!("HTTP {status}: {err}")));
            }
            Err(e) => summary.failed.push((u.plugin_id, e.to_string())),
        }
    }
    Ok(summary)
}

/// Print an update summary; an error when any update failed.
pub fn report_update(summary: &UpdateSummary) -> anyhow::Result<()> {
    if summary.updated.is_empty() && summary.skipped.is_empty() && summary.failed.is_empty() {
        println!("All plugins are up to date.");
    }
    for (id, old, new) in &summary.updated {
        println!("Updated {id}: {old} -> {new}");
    }
    for (id, added) in &summary.skipped {
        println!(
            "Skipped {id}: the update would allow new hosts ({}). Re-run with --yes to approve.",
            added.join(", ")
        );
    }
    for (id, err) in &summary.failed {
        println!("Failed {id}: {err}");
    }
    if !summary.failed.is_empty() {
        bail!("{} plugin update(s) failed", summary.failed.len());
    }
    Ok(())
}

/// Approves per `--yes`, or asks on a terminal; skips when stdin is not one.
pub struct CliApprover {
    pub yes: bool,
}

impl Approver for CliApprover {
    fn approve(&mut self, id: &str, added: &[String]) -> bool {
        use std::io::{IsTerminal, Write};
        println!("Update of {id} allows network access to new hosts:");
        for d in added {
            let shown = if d == "*" { "ANY public host" } else { d };
            println!("  - {shown}");
        }
        if self.yes {
            return true;
        }
        if !std::io::stdin().is_terminal() {
            return false;
        }
        print!("Allow this access and update {id}? [y/N] ");
        let _ = std::io::stdout().flush();
        let mut answer = String::new();
        if std::io::stdin().read_line(&mut answer).is_err() {
            return false;
        }
        matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Json;
    use axum::extract::{Path, State};
    use axum::http::StatusCode;
    use axum::routing::{get, post};
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Mock {
        registry_down: bool,
        fail: Vec<&'static str>,
        /// (id, approve_permissions) of every update call.
        calls: Arc<Mutex<Vec<(String, bool)>>>,
    }

    fn available_json() -> serde_json::Value {
        serde_json::json!([
            {"id":"alldebrid","name":"AllDebrid","version":"1.1.0","description":"Premium link generator","author":"a","tags":["multi-hoster"],"installed":true,"api_version":1,"domains":["api.alldebrid.com"]},
            {"id":"youtube","name":"YouTube","version":"1.2.0","description":"Video extractor","author":"a","tags":["hoster"],"installed":true,"api_version":1,"domains":null},
            {"id":"xfilesharing","name":"XFileSharing","version":"1.0.0","description":"Many hosters","author":"a","tags":["hoster"],"installed":true,"api_version":1,"domains":null},
            {"id":"premiumize","name":"Premiumize","version":"1.0.0","description":"Premium","author":"a","tags":["multi-hoster"],"installed":false,"api_version":1,"domains":[]}
        ])
    }

    fn check_json() -> serde_json::Value {
        serde_json::json!({"core":{},"plugins":[
            {"plugin_id":"alldebrid","current_version":"1.0.0","available_version":"1.1.0","is_new":false,"declared_domains":["api.alldebrid.com"],"added_domains":null,"requires_approval":false},
            {"plugin_id":"youtube","current_version":"1.1.0","available_version":"1.2.0","is_new":false,"declared_domains":null,"added_domains":["*"],"requires_approval":true},
            {"plugin_id":"premiumize","current_version":null,"available_version":"1.0.0","is_new":true,"declared_domains":[],"added_domains":null,"requires_approval":true}
        ]})
    }

    async fn serve(mock: Mock) -> Target {
        let app = axum::Router::new()
            .route(
                "/api/v1/updates/plugins/available",
                get(|State(m): State<Mock>| async move {
                    if m.registry_down {
                        (
                            StatusCode::SERVICE_UNAVAILABLE,
                            Json(serde_json::json!({"error":"Registry unavailable: HTTP 404"})),
                        )
                    } else {
                        (StatusCode::OK, Json(available_json()))
                    }
                }),
            )
            .route(
                "/api/v1/updates/check",
                get(|| async { Json(check_json()) }),
            )
            .route(
                "/api/v1/updates/plugins/{id}",
                post(
                    |State(m): State<Mock>,
                     Path(id): Path<String>,
                     Json(body): Json<serde_json::Value>| async move {
                        let approved = body["approve_permissions"].as_bool().unwrap_or(false);
                        m.calls.lock().unwrap().push((id.clone(), approved));
                        if m.fail.contains(&id.as_str()) {
                            return (
                                StatusCode::BAD_GATEWAY,
                                Json(serde_json::json!({"error":"Checksum mismatch"})),
                            );
                        }
                        let version = if id == "youtube" { "1.2.0" } else { "1.1.0" };
                        (
                            StatusCode::OK,
                            Json(serde_json::json!({"id": id, "version": version})),
                        )
                    },
                ),
            )
            .with_state(mock);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Target {
            name: "home".into(),
            base: format!("http://{addr}"),
            token: "t".into(),
        }
    }

    struct Fixed(bool, Vec<String>);
    impl Approver for Fixed {
        fn approve(&mut self, id: &str, _added: &[String]) -> bool {
            self.1.push(id.to_string());
            self.0
        }
    }

    #[tokio::test]
    async fn ac18_search_matches_id_name_description_and_tags() {
        let t = serve(Mock::default()).await;
        let ids = |hits: Vec<AvailablePlugin>| hits.into_iter().map(|p| p.id).collect::<Vec<_>>();
        assert_eq!(ids(search(&t, "DEBRID").await.unwrap()), ["alldebrid"]);
        assert_eq!(ids(search(&t, "video").await.unwrap()), ["youtube"]);
        assert_eq!(
            ids(search(&t, "multi-hoster").await.unwrap()),
            ["alldebrid", "premiumize"]
        );
        let hits = search(&t, "premiumize").await.unwrap();
        assert_eq!(hits[0].version, "1.0.0");
        assert!(!hits[0].installed);
        assert!(search(&t, "nothing-like-this").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn ac19_update_all_reports_each_plugin_and_skips_new_ones() {
        let mock = Mock::default();
        let calls = mock.calls.clone();
        let t = serve(mock).await;
        let s = update(&t, None, &mut Fixed(true, vec![])).await.unwrap();
        assert_eq!(
            s.updated,
            [
                ("alldebrid".into(), "1.0.0".into(), "1.1.0".into()),
                ("youtube".into(), "1.1.0".into(), "1.2.0".into())
            ]
        );
        // Not-installed plugins are never installed by update.
        assert!(
            !calls
                .lock()
                .unwrap()
                .iter()
                .any(|(id, _)| id == "premiumize")
        );
        assert!(report_update(&s).is_ok());
    }

    #[tokio::test]
    async fn ac19_failed_update_is_reported_and_fails_the_command() {
        let mock = Mock {
            fail: vec!["alldebrid"],
            ..Mock::default()
        };
        let t = serve(mock).await;
        let s = update(&t, None, &mut Fixed(true, vec![])).await.unwrap();
        assert_eq!(s.failed.len(), 1);
        assert_eq!(s.failed[0].0, "alldebrid");
        assert_eq!(s.updated.len(), 1, "the other plugin is still updated");
        assert!(report_update(&s).is_err());
    }

    #[tokio::test]
    async fn ac20_widened_access_needs_approval() {
        // Declined: the widening update is skipped, not sent, and not a failure.
        let mock = Mock::default();
        let calls = mock.calls.clone();
        let t = serve(mock).await;
        let mut asked = Fixed(false, vec![]);
        let s = update(&t, None, &mut asked).await.unwrap();
        assert_eq!(asked.1, ["youtube"], "only the widening update asks");
        assert_eq!(s.skipped, [("youtube".to_string(), vec!["*".to_string()])]);
        assert_eq!(*calls.lock().unwrap(), [("alldebrid".to_string(), false)]);
        assert!(report_update(&s).is_ok());

        // Approved: sent with the approval flag.
        let mock = Mock::default();
        let calls = mock.calls.clone();
        let t = serve(mock).await;
        update(&t, Some("youtube"), &mut Fixed(true, vec![]))
            .await
            .unwrap();
        assert_eq!(*calls.lock().unwrap(), [("youtube".to_string(), true)]);
    }

    #[test]
    fn ac20_without_yes_and_without_a_terminal_the_update_is_skipped() {
        use std::io::IsTerminal;
        // Test runners never give the test a terminal on stdin.
        assert!(!std::io::stdin().is_terminal());
        assert!(!CliApprover { yes: false }.approve("youtube", &["*".into()]));
    }

    #[test]
    fn ac20_yes_flag_approves_without_asking() {
        assert!(CliApprover { yes: true }.approve("youtube", &["*".into()]));
    }

    #[tokio::test]
    async fn ac21_unavailable_registry_and_unknown_plugins_are_errors() {
        let t = serve(Mock {
            registry_down: true,
            ..Mock::default()
        })
        .await;
        let err = search(&t, "x").await.unwrap_err().to_string();
        assert!(
            err.contains("registry") && err.contains("unavailable"),
            "{err}"
        );
        let err = update(&t, None, &mut Fixed(true, vec![]))
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("unavailable"), "{err}");

        let t = serve(Mock::default()).await;
        for (id, expect) in [
            ("nope", "not in the registry"),
            ("premiumize", "not installed"),
            ("xfilesharing", "no newer version"),
        ] {
            let err = update(&t, Some(id), &mut Fixed(true, vec![]))
                .await
                .unwrap_err()
                .to_string();
            assert!(err.contains(expect), "{id}: {err}");
        }
    }

    #[test]
    fn ac21_no_paired_server_is_an_error() {
        let err = target(&crate::remotes::Remotes::default(), None)
            .err()
            .unwrap()
            .to_string();
        assert!(err.contains("No paired server"), "{err}");
    }

    #[tokio::test]
    async fn ac21_unreachable_server_is_named() {
        let t = Target {
            name: "home".into(),
            base: "http://127.0.0.1:9".into(),
            token: "t".into(),
        };
        let err = format!("{:#}", search(&t, "x").await.unwrap_err());
        assert!(err.contains("cannot reach server 'home'"), "{err}");
    }
}
