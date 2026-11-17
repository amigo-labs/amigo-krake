---
status: todo
created: 2026-11-17
size: L
design: [AGENTS.md "Plugin Registry & Updates", AGENTS.md "Sandboxing", AGENTS.md Entscheidung 17, docs/plugin-api.md, docs/hosting.md "Plugin auto-updates"]
surfaces: [plugin-api, rest, cli]
---

# Plugin registry

## Context

The client side of the plugin registry is complete: the server fetches
`index.json` plus the detached Ed25519 signature `index.json.sig`, verifies it
against a public key pinned at compile time, caches it for 24 hours, and
installs or updates plugins after checking each artifact's SHA-256 and asking
the user to approve the hosts the plugin may reach. The web UI marketplace and
`krake plugins install` are built on top of it.

What is missing is the registry itself. The default `plugin_registry_url`
points at `amigo-labs/amigo-downloader-plugins`, a repository that never
existed under that name (HTTP 404), so every marketplace is empty. Nothing
builds an index from the plugins in `plugins/`, nothing signs it, nothing
publishes it.

Decided (Entscheidung 17): the registry is served at
`https://krake-registry.amigo-labs.dev` by the Cloudflare Worker `amigo-krake`
(Workers Builds, static assets only), which deploys the branch `registry` of
this repository as static files with no build step. A
GitHub Actions job on `main` builds, signs and commits the registry content to
that branch, so the signing key never leaves GitHub Actions. The index is
signed with the same Ed25519 key as the core self-update archives
(`AMIGO_UPDATE_SIGNING_KEY`), so the binaries pin that public key for both.
Plugins stay in this monorepo (Entscheidung 1).

## Scope

### In scope

- A maintainer command that builds the registry from `plugins/`: it loads every
  plugin with the real plugin runtime and writes `index.json` and the plugin
  sources under versioned paths.
- Signing the index with the update signing key, refusing a key that does not
  match the pinned public key.
- A publish job on every push to `main` that adds new plugin versions to the
  branch `registry` and replaces the signed index.
- A pull-request check that fails when a plugin's source changed but its
  version did not increase over the published one.
- The default registry URL `https://krake-registry.amigo-labs.dev/index.json`,
  and rewriting the dead old default in existing configs.
- Release builds pin the update public key as the registry key.
- `POST /api/v1/plugins/suggest` uses the configured registry instead of the
  built-in default.
- `krake plugins search <query>` and `krake plugins update [id]` against the
  paired server, like `krake plugins install`.
- `docs/hosting.md` and `docs/plugin-api.md` describe how plugins are published.

### Out of scope

- Third-party plugin submissions, a plugin from outside this repository — because
  plugins live in the monorepo until the plugin API is stable (Entscheidung 1).
- A second, overlapping trusted key for key rotation — because rotation by
  shipping a new release is the documented, fail-safe behavior (`registry.rs`).
- Serving more than the latest version of a plugin in the index (rollback,
  version pinning) — because no client can choose a version today; older
  artifacts stay downloadable only so a cached index keeps working.
- Setting up DNS, the Cloudflare Workers project and the GitHub secret — because
  that is done by an organization owner, not in code (see Technical notes).
- Changing the index format (`schema_version` 1) — because every released
  client parses it; the builder fills the existing fields.

## Acceptance criteria

Terms: a **plugin** is a directory under `plugins/hosters`, `plugins/multi-hosters`
or `plugins/extractors` that contains `plugin.ts` or `plugin.js` (spec files
`plugin.spec.*` are not plugins). **Base URL** is an absolute `https` URL
without a trailing slash; the default is `https://krake-registry.amigo-labs.dev`.
**Published registry** is the content of the branch `registry` of this
repository; the **published version** of a plugin is its version in the
published `index.json`, and a version is **already published** when an artifact
file for that id and version exists on the branch (also for a plugin no longer
in the index). The **output** is the complete site tree the build produces:
every published file plus the new ones. **Versions** are compared by SemVer 2.0 precedence
(pre-releases sort below their release). **Same source** means byte-identical
files.

- [ ] AC-01 (event): When the registry build runs over the repository, then it
  writes an `index.json` (`schema_version` 1) with exactly one entry per plugin,
  and none for `plugins/template` or `plugins/types`.
- [ ] AC-02 (always): Each index entry's `id`, `name`, `version`, `description`,
  `author`, `url_pattern`, `api_version` and `permissions` equal the values the
  plugin's exported object declares (missing optional fields: empty string for
  `description`/`author`, `api_version` 1, no `permissions`), as checked against
  a fixture plugin whose values are fixed in the test; `tags` holds the plugin's
  `pluginType` (default `hoster`); `min_app_version` is `null`.
- [ ] AC-03 (always): Each entry's `download_url` is
  `<base URL>/plugins/<id>/<version>/plugin.<ext>` with `<ext>` the extension of
  the plugin's source file, the build writes that source byte-for-byte to the
  relative path `plugins/<id>/<version>/plugin.<ext>`, and the entry's `sha256`
  is the lowercase hex SHA-256 of those bytes.
- [ ] AC-04 (error): If a plugin fails to transpile or load, declares an id that
  `validate_plugin_id` rejects, has a version that is not
  SemVer, requires a host-API version this build cannot load, has a
  `urlPattern` that is not a valid regex, has a source that is not UTF-8 or is
  larger than the 2 MiB a client accepts, or shares its id with another plugin,
  then the build exits non-zero, names every failing plugin with its reason,
  and writes nothing to the output directory.
- [ ] AC-05 (event): When the build is given a published registry, then every
  artifact listed in its `index.json` must exist with its `sha256` (a missing
  `index.json`, a missing or mismatching artifact fails the build); a plugin
  whose version is already published with a different source, or whose version
  is lower than its published version, makes the build fail naming the plugin;
  every previously published file is copied unchanged into the output. On any
  of these failures the build writes nothing to the output directory.
- [ ] AC-06 (event): If no plugin is found, then the build exits non-zero and
  writes no `index.json`.
- [ ] AC-07 (always): Running the build twice over the same input produces a
  byte-identical `index.json`, and signing the same index twice produces the
  same signature.
- [ ] AC-08 (event): When the index is signed with a seed whose public key
  equals the pinned key, then `index.json.sig` holds a signature that the
  server's registry client accepts with that key.
- [ ] AC-09 (error): If the signing seed or the pinned public key is missing,
  is not 64 hex characters, or the seed does not derive the pinned key, then signing exits non-zero and
  writes no signature.
- [ ] AC-10 (event): When the publish step runs against a git repository whose
  `registry` branch is missing, then it creates it with one commit containing
  `index.json`, `index.json.sig` and every artifact; when it runs again after a
  plugin's version was raised, then it adds exactly one commit that adds the new
  artifact and replaces `index.json` and `index.json.sig`, keeping all earlier
  artifacts; when nothing in the output differs from the branch, then it adds no
  commit. (Tested against a local temporary repository.)
- [ ] AC-11 (error): If the remote `registry` branch moved since the publish
  step read it (another run pushed first), then the step exits non-zero without
  overwriting the remote; run again, it builds on the new tip and succeeds.
  Before pushing, the step verifies the new `index.json.sig` against the pinned
  key, so `index.json` and a matching signature always arrive in the same
  commit. (Tested against a local temporary repository.)
- [ ] AC-12 (event, CI: registry-publish): When the last push to `main` has
  been processed, then the published registry matches the plugins of that
  commit, even if pushes in between were skipped or superseded (each run
  publishes the full state of its commit, never a delta). Evidence: the
  workflow run for the merge of this feature is green and
  `<base URL>/index.json` lists the plugins of that commit.
- [ ] AC-13 (error, CI: pr): If a pull request contains a plugin whose source
  differs from the published artifact of its published version and whose
  version is not higher than that, then the pull-request
  check fails and names the plugin; a pull request that changes no plugin
  passes, and so does one where the published registry does not exist yet.
- [ ] AC-14 (always): A fresh config's `update.plugin_registry_url` is
  `<default base URL>/index.json`, and so is the registry default used without a
  config.
- [ ] AC-15 (event): When a config is loaded whose `plugin_registry_url` is the
  old default `https://raw.githubusercontent.com/amigo-labs/amigo-downloader-plugins/main/index.json`,
  then Krake uses the new default URL from then on, and the next time the config
  is saved (for any reason) the file holds the new URL; loading alone does not
  write the file. Any other custom URL is kept unchanged.
- [ ] AC-16 (always, CI: release): Release builds pin the update signing public
  key as the registry public key, and the release build fails when that key is
  missing.
- [ ] AC-17 (event): When `POST /api/v1/plugins/suggest` is called, then the
  server matches the URL against the registry it is configured with (URL and
  signature verification), not the built-in default; if that registry is
  unreachable or fails verification, the answer is "no plugin found", as for an
  unavailable registry today.
- [ ] AC-18 (event): When `krake plugins search <query>` runs with a paired
  server, then it lists the server's available registry plugins whose id, name,
  description or tags contain the query (case-insensitive), each with id,
  version and whether it is installed; with no match it prints that nothing
  matched and exits 0.
- [ ] AC-19 (event): When `krake plugins update [id]` runs with a paired server,
  then it updates that plugin, or every installed plugin with a newer version
  without an id, and prints each updated plugin with its old and new version;
  it reports every plugin it tried, and exits non-zero if any update failed.
  Updates skipped because access was not approved (AC-20) are listed but do not
  count as failures.
- [ ] AC-20 (error): If an update widens a plugin's network access, then
  `krake plugins update` shows the added hosts and applies that update only
  after the user answers yes or `--yes` is passed; when stdin is not a terminal
  and `--yes` is missing, it skips that plugin and says so.
- [ ] AC-21 (error): If no server is paired, the server cannot be reached, the
  server answers that the registry is unavailable, or `update <id>` names a
  plugin that is not installed or has no newer version in the registry,
  then `krake plugins search` / `update` exit non-zero with a message naming
  the cause.

## Technical notes

- **Builder.** A maintainer subcommand next to `krake plugins test` (proposal:
  `krake plugins registry build --plugins <dir> --out <dir> --base-url <url>
  [--published <dir>]`), not listed in the user-facing help, loading each
  plugin through `krake-plugin-runtime` exactly as the server does.
  `--published` points at a checkout of the `registry` branch (AC-05) and is
  also what the pull-request check (AC-13) uses. Signing (AC-08, AC-09) and
  publishing (AC-10, AC-11) are separate steps callable on their own; how they
  are packaged (subcommands or scripts) is up to the build.
- **Signing.** Same scheme as `scripts/sign-update-archives.sh`: lowercase hex
  of the Ed25519 signature over the exact `index.json` bytes, seed from
  `KRAKE_UPDATE_SIGNING_KEY` (GitHub secret `AMIGO_UPDATE_SIGNING_KEY`), pinned
  key `KRAKE_UPDATE_PUBKEY_HEX` (variable `AMIGO_UPDATE_PUBKEY_HEX`).
- **Release pinning.** `release-build.yml` sets
  `KRAKE_REGISTRY_PUBKEY_HEX` from `vars.AMIGO_UPDATE_PUBKEY_HEX`; the variable
  `AMIGO_REGISTRY_PUBKEY_HEX` is no longer read.
- **Branch `registry` layout** (served as the site root):
  `index.json`, `index.json.sig`, `plugins/<id>/<version>/plugin.<ts|js>`. Orphan
  branch, created by the first publish run. Plain static files; Cloudflare
  serves `.ts` with whatever type it picks, the client only checks bytes.
- **Hosting config:** the publish step writes `wrangler.jsonc` (Worker
  `amigo-krake`, the whole tree as static assets, no Worker code) and
  `.assetsignore` into the branch root, so Workers Builds deploys the branch
  as is. `main` keeps its own `wrangler.jsonc` for the unrelated
  `amigo-downloader` redirect Worker.
- **Org setup (manual, once):** Cloudflare Workers project `amigo-krake`
  connected to this repository with production branch `registry`, builds of
  other branches off, empty build command, deploy command
  `npx wrangler deploy`, root `/`, custom domain
  `krake-registry.amigo-labs.dev`. The workflow
  needs `contents: write` to push `registry`.
- **CLI.** `search` uses `GET /api/v1/updates/plugins/available`. `update`
  reads the pending updates (old/new version, `requires_approval`, added
  domains) from `GET /api/v1/updates/check`, then calls
  `POST /api/v1/updates/plugins/{id}` once per plugin with
  `approve_permissions` where the user approved — not `/update-all`, which
  neither takes approval nor reports per-plugin results. "Not installed" and
  "no newer version" are decided from that list before any call. No server
  change is needed.
- **PR check** reads the `registry` branch of this repository and compares
  versions and artifact bytes only; it needs no secret or variable, so it
  works for fork PRs. The signature is checked by the publish job, which has
  the key.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| First publish, branch `registry` does not exist yet | The job creates it as an orphan branch; the PR check treats every plugin as new and passes. |
| Plugin directory deleted from `plugins/` | Its entry disappears from the next index; its published artifacts stay on `registry` so cached indexes still resolve. Installed copies are untouched. |
| Push to `main` that changes no plugin | The build produces an identical index; the job commits nothing. |
| Version raised without source change | Published as a new version (new path, same bytes). |
| Server holds a cached index up to 24 h old after a publish | Its `download_url`s point to versioned paths that still exist, so installs keep working; the new version appears after the cache expires. |
| Registry unreachable from the PR check (branch fetch fails for a reason other than "branch does not exist") | The check fails with the fetch error rather than passing silently. |
| Plugin deleted from `plugins/`, nothing else changed | The index changes, so the publish step commits the new index and signature (AC-10). |
| Plugin authored as `plugin.js` | Published as `plugin.js`; the client accepts both. |
| Plugin declares no `permissions.domains` (e.g. xfilesharing) | Entry has no `permissions`; client shows "any public host" as today. |
| `krake plugins search` with a query matching nothing | Prints "no plugins match" and exits 0. |
| Custom `plugin_registry_url` (a fork) in the config | Kept; only the exact old default is rewritten. |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Index lists only the latest version per plugin | Clients install "the" version from the index; nothing selects older ones | A broken release can only be fixed forward with a higher version |
| `min_app_version` stays `null` | `api_version` is the compatibility gate the client already enforces; a plugin needing a new host function requires a raised `api_version` | A plugin that relies on a minor-release host function without raising `api_version` would fail at runtime on older servers |
| One key for core updates and the registry | Maintainer's choice: one secret to guard and rotate; the signed payloads differ in kind (archive vs. JSON index), so a signature for one cannot pass as the other | A leaked key compromises both channels at once, and rotating it rotates both |
| Existing release binaries keep their old registry key | They never had a working registry (the old URL is 404), so nothing they use breaks; they get the registry once they self-update | none |
| Byte-for-byte delivery through Cloudflare | Workers static assets are served unchanged; transport compression is undone by the HTTP client before the SHA-256 check | A host-side transform would surface as a checksum error, never as a silent change |
| The PR check can read branch `registry` from fork PRs | The repository is public and the check only reads git data, no secret or variable | A private repository would need a token |
| Cloudflare deploys every push to `registry` within minutes | Git-connected Workers project (Workers Builds), as the user set it up | Delay only; no correctness impact |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ac01_one_entry_per_plugin_in_the_three_categories` (crates/plugin-runtime/tests/registry_build_tests.rs) | built |  |
| AC-02 | `ac02_entry_fields_equal_the_declared_values` (crates/plugin-runtime/tests/registry_build_tests.rs) | built |  |
| AC-03 | `ac03_download_url_path_and_sha256_match_the_written_artifact` (crates/plugin-runtime/tests/registry_build_tests.rs) | built |  |
| AC-04 | `ac04_*` (9 tests) (crates/plugin-runtime/tests/registry_build_tests.rs) | built |  |
| AC-05 | `ac05_*` (5 tests) (crates/plugin-runtime/tests/registry_build_tests.rs) | built |  |
| AC-06 | `ac06_empty_plugin_tree_fails` (crates/plugin-runtime/tests/registry_build_tests.rs) | built |  |
| AC-07 | `ac07_build_and_signature_are_deterministic` (crates/plugin-runtime/tests/registry_build_tests.rs) | built |  |
| AC-08 | `ac08_signed_index_is_accepted_by_the_registry_client` (crates/plugin-runtime/tests/registry_build_tests.rs) | built |  |
| AC-09 | `ac09_signing_refuses_bad_or_missing_key_material` (crates/plugin-runtime/tests/registry_build_tests.rs) | built |  |
| AC-10 | `registry_cmd::tests::ac10_publish_creates_extends_and_skips_unchanged` (crates/cli/src/registry_cmd.rs) | built |  |
| AC-11 | `registry_cmd::tests::ac11_moved_branch_fails_without_overwriting_then_rerun_succeeds`, `ac11_unverifiable_signature_is_never_committed` (crates/cli/src/registry_cmd.rs) | built |  |
| AC-12 | `.github/workflows/registry-publish.yml` job `publish` (green run after merge + live `index.json`) | built |  |
| AC-13 | `.github/workflows/ci.yml` job `registry-check` (`krake plugins registry check`); logic covered by `ac05_*` in crates/plugin-runtime/tests/registry_build_tests.rs and `ac10_*` in crates/cli/src/registry_cmd.rs | built |  |
| AC-14 | `config::tests::ac14_default_registry_url_is_the_published_registry` (crates/core/src/config.rs), `registry::tests::ac14_default_index_url_is_the_published_registry` (crates/plugin-runtime/src/registry.rs), `ac14_server_and_runtime_default_to_the_same_registry` (crates/server/tests/api_tests.rs) | built |  |
| AC-15 | `config::tests::ac15_legacy_registry_url_is_replaced_on_load_and_written_on_save`, `ac15_custom_registry_url_is_kept` (crates/core/src/config.rs) | built |  |
| AC-16 | `.github/workflows/release-build.yml` (`KRAKE_REGISTRY_PUBKEY_HEX` from `vars.AMIGO_UPDATE_PUBKEY_HEX`, step "Require pinned signing keys") | built |  |
| AC-17 | `ac17_suggest_uses_the_configured_registry`, `ac17_unreachable_registry_means_no_suggestion`, `ac17_suggest_verifies_the_registry_signature` (crates/server/tests/api_tests.rs) | built |  |
| AC-18 | `plugins_remote::tests::ac18_search_matches_id_name_description_and_tags` (crates/cli/src/plugins_remote.rs) | built |  |
| AC-19 | `ac19_update_all_reports_each_plugin_and_skips_new_ones`, `ac19_failed_update_is_reported_and_fails_the_command` (crates/cli/src/plugins_remote.rs) | built |  |
| AC-20 | `ac20_widened_access_needs_approval`, `ac20_without_yes_and_without_a_terminal_the_update_is_skipped`, `ac20_yes_flag_approves_without_asking` (crates/cli/src/plugins_remote.rs) | built |  |
| AC-21 | `ac21_unavailable_registry_and_unknown_plugins_are_errors`, `ac21_no_paired_server_is_an_error`, `ac21_unreachable_server_is_named` (crates/cli/src/plugins_remote.rs) | built |  |

## Decisions

- 2026-11-17: Hosting on Cloudflare Pages from branch `registry`, built and
  signed in GitHub Actions — the signing key stays in GitHub; the branch keeps
  every published version, so cached indexes never point at a missing file.
- 2026-11-17: Signed with the core update key (`AMIGO_UPDATE_SIGNING_KEY`) —
  user decision, one key to manage.
- 2026-11-17: Metadata comes from loading each plugin with the real runtime —
  the index shows what clients will load, and a broken plugin is never
  published.
- 2026-11-17 (build): Blocks AC-01 to AC-11 were drafted before their tests,
  so the first test run was green. The red run was produced by mutation
  instead: disabling the "already published" check, the urlPattern check and
  the id ordering made `ac01`, `ac04_invalid_url_pattern…` and both `ac05`
  source tests fail; forcing the push, skipping signature verification and
  committing unchanged trees made all three `registry_cmd` tests fail.
- 2026-11-17 (build): The maintainer commands are hidden subcommands
  `krake plugins registry build|sign|check|publish` of the CLI, which already
  depends on the plugin runtime; `check` is the pull-request check (AC-13) and
  shares the branch checkout with `publish`.
- 2026-11-17 (build): The CLI now depends on `tempfile`, an existing workspace
  dependency already in `Cargo.lock` — no new crate enters the build.
- 2026-11-17 (build): Publishing works in a temporary `git worktree` of the
  checkout, so it reuses the checkout's credentials, and pushes without
  force; a run that lost a race fails (AC-11).
- 2026-11-17 (build): `ac17_unreachable_registry_means_no_suggestion` was
  green before the fix (the old code also failed to reach the dead default);
  the red run of AC-17 is `ac17_suggest_uses_the_configured_registry`.
- 2026-11-17 (build): The default URL is a constant in both `krake-core`
  (config) and `krake-plugin-runtime` (registry client), which do not depend
  on each other; a server test pins them equal.
- 2026-11-17 (build): `build_test_state` now uses the configured registry URL
  without an on-disk index cache, so server tests cannot share a cached index.
- 2026-11-17 (build): The CLI search/update logic was written with its tests;
  the red run came from mutation (dropping the description match, the approval
  check and the failure exit made the `ac18`, `ac19` failure and `ac20`
  approval tests fail). The empty-search message ("No plugins match.") is
  printed by `print_search`; AC-18's exit 0 follows from `search` returning an
  empty list without error.
- 2026-11-17 (build): `update` reads `/updates/plugins/available` first so an
  unavailable registry is reported as such — `/updates/check` swallows
  registry errors into an empty list.
- 2026-11-17 (build): AC-12, AC-13 and AC-16 are CI-only; they cannot go red
  locally. Manual run against a local bare origin: `publish` created the
  branch, and after editing `alldebrid` without a version bump `check` exited
  1 naming the plugin. Their first CI runs are the proof.
- 2026-11-17 (build): The publish workflow runs on pushes to `main` that touch
  `plugins/`, the plugin runtime, the CLI or `Cargo.lock` (and on demand);
  other pushes cannot change what is published.
- 2026-11-17 (build): `docs/release-todo.md` now asks for one key pair; the
  variable `AMIGO_REGISTRY_PUBKEY_HEX` is no longer read.
- 2026-11-17 (build, verification round 1): AC-17's verification clause got
  `ac17_suggest_verifies_the_registry_signature` (pinned key; a signature from
  another key or none means no suggestion); red run by mutation — skipping
  verification in `PluginUpdater::load_index` made it fail. The server test
  state takes an explicit registry config via `build_test_state_with_registry`.
- 2026-11-17 (build, verification round 1): `docs/plugin-api.md` gained
  "Publishing to the registry"; the publish workflow checks out full history.
- 2026-11-17 (follow-up): The Cloudflare project turned out to be a Workers
  project (Workers Builds) with the custom domain attached, not Pages. It
  needs a Worker configuration in the deployed branch, so the publish step
  writes `wrangler.jsonc` + `.assetsignore` (`hosting_config_serves_the_tree_as_static_assets`
  in crates/cli/src/registry_cmd.rs). Project settings: production branch
  `registry`, builds of other branches off.
