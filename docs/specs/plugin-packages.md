---
status: todo
created: 2026-11-22
size: M
design: [AGENTS.md "Plugin-System" (Plugin-Interface, DownloadInfo), AGENTS.md "Core-Features" (Queue & Scheduler), docs/plugin-api.md (resolve, DownloadPackage), docs/architecture.md]
surfaces: [core, plugin-api]
---

# Plugin packages, headers and cookies, and three file-host plugins

## Context

A plugin's `resolve()` returns a `DownloadPackage`: a name and a list of
`DownloadInfo` entries, each with a URL, filename, size, `headers` and
`cookies`. Krake uses only part of that today:

- Only the **first** entry is queued; every other entry of a package is
  dropped. A folder or album link therefore yields one file.
- `headers` and `cookies` are dropped: the transfer always starts with no
  extra headers. Hosts that check the `Referer` or a session cookie on the
  file server refuse the download.

File hosts that users ask for most — gofile.io, bunkr and filester.me —
hand out folders and albums and need a cookie or a `Referer` on the file
request, so they cannot work until both gaps are closed. This spec closes
them and adds the three plugins.

The plugin API (`DownloadPackage`, `DownloadInfo`, `krake.d.ts`,
`HOST_API_VERSION`) does not change: the fields already exist, Krake starts
honouring them.

## Scope

### In scope

- Queue every entry of a resolved package, files of a multi-file package in
  a folder of their own.
- Send an entry's headers and cookies with its transfer, also after retries
  and a restart, without exposing them through the API or the history.
- Resolve an entry whose URL a plugin claims (a folder that lists file pages)
  through that plugin before queueing it, one level deep.
- Plugins for gofile.io, bunkr (its current domains) and filester.me.

### Out of scope

- A package entity in the UI (grouping, package-level pause) — because
  packages are not wired into the data model (see link-list-import) and the
  folder on disk already keeps the files together.
- Re-resolving a link when its transfer starts instead of when it is added —
  because resolution at add time is how Krake works for every plugin today;
  a link that expired in a long queue fails and is retried by the user.
- Password-protected gofile folders and private filester files — because
  the plugin API has no way to ask the user for a per-link password.
- Premium accounts of the three hosts — because their free download paths
  need none, and no one asked for premium.
- Resolving deeper than one level (a folder inside a folder inside a folder) —
  because every plugin's HTTP budget is per call, and one level covers
  albums and folders of the three hosts.
- Changing the plugin API or `HOST_API_VERSION` — because the existing
  `DownloadPackage` fields already express everything needed.

## Acceptance criteria

Terms used below:

- **Entry** — one `DownloadInfo` of the package a plugin returned.
- **Transfer request** — every HTTP request Krake makes to download an
  entry's file (each chunk, each retry, each attempt after a restart).

- [ ] AC-01 (event): When a plugin resolves an added URL to a package with
  several entries, then Krake queues one download per entry, in the order of
  the package, each with the entry's filename and size.
- [ ] AC-02 (always): The downloads of a package with more than one entry are
  saved in a folder named after the package of the added URL, made safe by
  the rule Krake applies to file names (no path separators, no characters
  invalid on Windows, never `..`), inside the folder the download would
  otherwise go to (the download folder, or its category subfolder when the
  add names a category); a package with one entry is saved as before,
  without that folder, because a folder for one file adds nothing.
- [ ] AC-03 (always): Krake sends the headers of an entry with every transfer
  request of its download — the first attempt, an attempt after a failed one,
  and an attempt after Krake was restarted with the download still queued —
  except `Host`, `Content-Length`, `Range`, `Transfer-Encoding` and
  `Connection`, which Krake sets itself.
- [ ] AC-04 (always): Krake sends the cookies of an entry as a `Cookie` header
  with every transfer request of its download; when the entry also sets a
  `Cookie` header, both are sent in that one header.
- [ ] AC-05 (always): The headers and cookies of an entry appear neither in
  the REST responses for downloads or history, nor in WebSocket events, nor
  in the history entry the download leaves.
- [ ] AC-06 (event): When an entry's URL differs from the added URL and the
  plugin Krake would pick for the entry's URL is the plugin that returned the
  entry, then Krake resolves the entry through that plugin and queues the
  entries that resolution returns, instead of the entry itself. Entries of
  that second resolution are queued as they are, except those that the same
  plugin would pick again: they are deeper than one level, not queued, and
  logged. (Another plugin claiming the entry, such as the generic HTTP
  plugin that matches every URL, does not trigger a second resolution.)
- [ ] AC-07 (error): If resolving an entry (AC-06) fails, then that entry is
  not queued, the failure is logged with the entry's URL, and the other
  entries are queued.
- [ ] AC-08 (error): If a plugin returns a package without entries, or none
  of its entries ends up queued or already queued (AC-07, AC-06 depth), then
  the add fails with an error that names the plugin, and nothing is queued.
- [ ] AC-09 (always): `POST /api/v1/downloads` answers with the ids of the
  downloads the add stands for, in package order, in `ids` — newly queued
  ones and, for an entry whose URL was already queued, the existing
  download's id — and the first of them in `id`, because existing clients
  read `id` and the first entry is the one they got before.
- [ ] AC-10 (event): When a gofile.io folder link (`/d/<id>`) is added, then the
  gofile plugin returns the folder's files as entries with their names and
  sizes, and its subfolders as folder links (AC-06); every file entry carries
  the guest account cookie the file server needs.
- [ ] AC-11 (error): If gofile.io reports a folder as not found or as password
  protected, then the gofile plugin fails with an error that says which.
- [ ] AC-12 (event): When a bunkr album link (`/a/<id>`) is added, then the
  bunkr plugin returns one entry per file of the album, each a bunkr file
  link (AC-06).
- [ ] AC-13 (event): When a bunkr file link (`/f/`, `/v/`, `/i/` or `/d/`) is
  added, then the bunkr plugin returns the file's direct URL (decrypted when
  bunkr sends it encrypted) with the file's name and a `Referer` header.
- [ ] AC-14 (event): When a filester.me file link (`/d/<slug>`) is added, then
  the filester plugin returns the file's download URL with the file's name.
- [ ] AC-15 (event): When a filester.me folder link (`/f/<slug>`) is added, then
  the filester plugin returns one file link per file across the folder's
  pages (AC-06).
- [ ] AC-16 (always): Every request the three plugins' `resolve` makes in
  their tests goes to a host in that plugin's `permissions.domains`, and each
  plugin's `urlPattern` matches its own link forms (gofile `/d/`; bunkr
  `/a/`, `/f/`, `/v/`, `/i/`, `/d/` on bunkr domains; filester `/d/`, `/f/`)
  and neither its file servers' direct URLs nor other hosts.

## Technical notes

- Headers and cookies are kept in the download's metadata (key
  `request_headers`, one object with the merged `Cookie`), which the history
  copy strips like the NZB data; no schema change.
- Plugin behaviour (AC-10 to AC-16) is tested offline in each plugin's
  `plugin.spec.ts` by replacing `krake.http*` with fakes that serve recorded
  responses; the tests never reach the real hosts.
- gofile: a guest account token from `POST https://api.gofile.io/accounts`,
  then `GET https://api.gofile.io/contents/<id>` with `Authorization: Bearer`,
  `X-Website-Token` (SHA-256 of user agent, language, token, the 4-hour time
  slot and the site's salt) and the token as `accountToken` cookie on the
  file request.
- bunkr: the album page lists file slugs; a file page carries
  `data-file-id`; `POST https://apidl.bunkr.ru/api/_001_v2` with
  `{"id": …}` returns the URL, XOR-encrypted with
  `SECRET_KEY_<timestamp / 3600>` when `encrypted` is set.
- filester: the file page's `og:title` is the name; `POST
  https://filester.me/api/public/download` with `{"file_slug": …}` returns
  the download path on `cache1.filester.me`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A package entry's URL is already queued | Not queued again; the existing id is in `ids` (AC-09), the others are queued. |
| The same URL twice in one package | Queued once; its id appears twice in `ids`. |
| Every entry already queued | The add succeeds with the existing ids (AC-08 counts them). |
| gofile subfolder that holds further subfolders | The deeper subfolders are skipped and logged (AC-06). |
| Two files with the same name in one package folder | Handled like two downloads with the same name in one folder today. |
| A bunkr album or filester folder with no files | AC-08: the add fails, nothing queued. |
| A network failure in the middle of a folder walk | The plugin call fails; the add falls back as for any failed resolution today (the URL is queued as is). |
| A package of one entry whose URL a plugin claims | Resolved once more (AC-06); the result is queued without a package folder unless it has several entries. |
| An entry resolves back to the URL that was added | Queued as is (no second resolution, AC-06 requires a different URL). |
| Package name with `/`, `..` or characters invalid on Windows | The folder name is made safe; it never leaves the download folder. |
| Package name empty | The package folder is named after the plugin id, because that is the one name always present. |
| Entry headers contain `Cookie` and cookies | One `Cookie` header with both (AC-04). |
| gofile folder with 0 files | AC-08: the add fails, nothing queued. |
| bunkr album larger than one plugin call's HTTP budget | The album page is one request; files are resolved one call each (AC-06). |
| Plugin throws for one bunkr file of an album | That file is skipped and logged (AC-07), the rest is queued. |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The owner wants folders and albums of the three hosts, not only single files | gofile links are always folders, bunkr shares are mostly albums | Low |
| The hosts' current web flows (documented by gallery-dl, checked 2026-11-22) stay as they are | No official free download API exists for gofile or bunkr | Medium: hosts change their flows; plugin updates go through the registry |
| Headers and cookies may be stored in SQLite metadata | They are short-lived guest sessions and referrers; the DB sits in Krake's config folder next to the config and plugin storage, which already hold plugin API keys with the same protection | Low |
| No upper limit on entries per package | Albums of the three hosts have at most a few hundred files; each second resolution is one plugin call, done one after another | Low: a huge album takes long to add |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 |      | open   |        |
| AC-02 |      | open   |        |
| AC-03 |      | open   |        |
| AC-04 |      | open   |        |
| AC-05 |      | open   |        |
| AC-06 |      | open   |        |
| AC-07 |      | open   |        |
| AC-08 |      | open   |        |
| AC-09 |      | open   |        |
| AC-10 |      | open   |        |
| AC-11 |      | open   |        |
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |
| AC-14 |      | open   |        |
| AC-15 |      | open   |        |
| AC-16 |      | open   |        |

## Decisions

- 2026-11-22: Written in goal mode (owner asked for "plugins for gofile.io,
  bunkr.cr and filester.me" and every spec implemented). The core gaps were
  found while researching the plugins; without them none of the three can
  download. No clarification round with the owner, because goal mode asks
  for uninterrupted work; open points are under Assumptions, and the owner
  reviews the spec in the PR.
- 2026-11-22, clarification pass: AC-06 narrowed to the plugin that returned
  the entry (the generic HTTP plugin claims every URL); deeper folders are
  skipped instead of queued as pages; duplicates count as handled entries
  in AC-08 and are reported in AC-09; AC-03 names the attempts and the
  headers Krake owns; AC-05 covers WebSocket events; AC-16 is stated as
  what the tests observe.
