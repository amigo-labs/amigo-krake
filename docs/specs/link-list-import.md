---
status: todo
created: 2026-11-17
size: M
design: [AGENTS.md "Core-Features" (Queue & Scheduler), AGENTS.md "CLI", AGENTS.md "REST API" (POST /downloads/batch), docs/architecture.md]
surfaces: [web-ui, cli]
---

# Link list import

## Context

Users collect download links in plain text files: one link per line, often
with a note after the link, comment lines or blank lines in between. Krake
should take such a file in one step and queue every link in it.

Some of this exists today, but it is incomplete:

- Dropping a non-DLC, non-NZB file on the web UI queues every line that
  starts with `http`. A note after the link becomes part of the URL,
  duplicates are queued twice, and failures show only a generic toast.
- The file picker of the add panel accepts `.txt` but copies the raw file
  text into the URL field. Any line with a note after the link then counts as
  invalid.
- The PWA share target on Android (`manifest.json`, GET) queues only the
  first link of a shared text, and it cannot receive files.
- The CLI has `krake add --dlc` and `--nzb`, but nothing for text files.

This spec gives all of these paths one line rule (the **link list rule**,
below), so the same file yields the same links everywhere.

## Scope

### In scope

- The link list rule, implemented for the web UI and for the CLI and pinned
  by one shared set of test cases.
- Web UI: dropping a text file, choosing a text file in the add panel, and
  pasting or dropping plain text all use the rule.
- Android: the installed PWA accepts shared text and shared text files, and
  queues every link from them.
- CLI: `krake add --list <file>` reads a link list (`-` reads stdin) and
  queues it on the local coordinator, like `--dlc`.
- A result report on every path: how many links were queued, already queued,
  rejected and how many lines were not recognized.

### Out of scope

- Finding URLs in the middle of prose ("see https://… for part 2") in files —
  because the user chose one link per line: free text extraction picks up
  image, forum and tracking links that nobody wants to download.
- Grouping the links of a file into a package named after the file — because
  packages are not wired into the data model (the DLC import queues flat too);
  that is its own spec.
- A REST endpoint that parses text files on the server — because the web UI
  already sends the extracted links through `POST /downloads/batch`, which
  validates every URL.
- Queueing a link list on a paired remote server from the CLI — because
  `krake add` runs on the local coordinator for every input type today;
  remote `add` is its own spec.
- Picking up `.txt` files from a watch folder — because the user did not ask
  for it; the NZB watch folder stays as it is.
- A native Android app (Tauri mobile) — because `desktop-app.md` excludes
  Tauri mobile; the share target works through the installed PWA.
- Magnet, FTP and other schemes — because Krake downloads only http and https
  (no torrent backend, no FTP backend).
- Encrypted containers or other container formats inside the text file —
  because those have their own import path (`--dlc`, drop of `.dlc`).

## Acceptance criteria

Terms used below:

- **Whitespace** — characters with the Unicode `White_Space` property (space,
  tab, U+00A0, U+2003, …). U+FEFF is not whitespace.
- **Active download** — a download in the list whose state is queued,
  downloading or paused (not completed, failed or cancelled).
- **Result counts** — *queued* (a new download was created), *already queued*
  (an active download with the same URL existed, nothing was created),
  *rejected* (Krake refused the link, with a reason) and *not recognized*
  (a line that is not a link by AC-01).

### Link list rule

- [ ] AC-01 (always): Krake reads a link list line by line (LF, CRLF and CR
  line endings, a leading UTF-8 byte order mark ignored, bytes that are not
  valid UTF-8 replaced by U+FFFD). For each line with leading and trailing
  whitespace removed: if it starts with `http://` or `https://`
  (case-insensitive), its link is everything up to the first whitespace
  character; if it is empty or starts with `#`, it is skipped and not
  counted; any other line counts as *not recognized*.
- [ ] AC-02 (always): The links of one list are queued in the order of their
  first appearance, and a link that appears more than once in the list is
  queued once. Two links count as the same when they are equal character by
  character after the rule of AC-01.
- [ ] AC-03 (always): The web UI and the CLI produce the same links and the
  same *not recognized* count for every case in one shared set of test cases,
  which covers at least: notes after a link, CRLF, CR, a byte order mark,
  invalid UTF-8, comment lines, blank lines, duplicates, uppercase
  `HTTPS://`, U+00A0 and U+FEFF inside a line, a link without host, and a
  line that is not a link.
- [ ] AC-04 (error): If the part of a link between `://` and the first `/`,
  `?` or `#` is empty (for example `https://` or `https:///path`), then the
  line counts as *not recognized*. Any other malformed link is left to Krake's
  URL validation and counts as *rejected* (AC-08).

### Web UI

- [ ] AC-05 (event): When the user drops a file on the web UI that is neither
  `.dlc` nor `.nzb`, then Krake queues the links of that file by the link
  list rule and shows one result toast with the result counts (counts of zero
  are left out). Links that equal the URL of an active download are not sent
  and count as *already queued*.
- [ ] AC-06 (event): When the user chooses a text file in the add panel, then
  the URL field is replaced by the extracted links, one per line, and the
  panel states how many lines were not recognized; nothing is queued before
  the user submits. If the file yields no link or is larger than 1 MiB, the
  URL field keeps its content and the panel shows the message of AC-09 or
  AC-10 instead.
- [ ] AC-07 (event): When the user drops plain text (not a file) on the web
  UI outside the add panel, or pastes it while no text field has focus, then
  Krake applies the link list rule and result toast of AC-05. Text pasted
  into the add panel's URL field stays there for review and is queued only on
  submit, by the same link list rule.
- [ ] AC-08 (error): If Krake rejects some links of a list (for example a
  private address or an invalid URL), then the other links are still queued
  and the result toast names each rejected link with its reason (the first
  five, then "and N more").
- [ ] AC-09 (error): If a list yields no link, then nothing is sent to the
  server and the message says that the file (or text) contained no links.
- [ ] AC-10 (error): If a dropped or chosen file, or dropped or pasted text,
  is larger than 1 MiB (its size in bytes as UTF-8, for a file its size on
  disk), then nothing is queued and the message says it is too large and
  names the limit.
- [ ] AC-11 (event): When the web UI queues a list of any length within the
  size limit, then every link reaches the server, however many requests that
  takes. If one of those requests fails as a whole (network error, session
  expired, server error), then every link sent before the failure stays
  queued, no further request is sent, and the result toast states the counts
  so far plus how many links were not sent. For a share, AC-16 applies
  instead when the session expired.
- [ ] AC-12 (always): Every new user-visible string of this spec exists in
  English and German.

### Android share target

- [ ] AC-13 (always): The installed Krake web app offers itself as a share
  target for shared text, shared links and shared files that are text
  (MIME type `text/*`, or extension `.txt` when the type is missing).
- [ ] AC-14 (event): When the user shares text to the installed Krake app,
  then Krake joins the shared title, text and URL as lines in that order and
  queues the links by the link list rule, with the result toast of AC-05. If
  the rule finds no link in the joined title, text and URL, Krake queues the
  first `http://` or `https://` URL found anywhere in them (case-insensitive, ending at whitespace,
  trailing `.,;:!?)]}>"'` removed), so a share like "Look at this:
  https://…" still works as it does today.
- [ ] AC-15 (event): When the user shares one or more text files (with or
  without text) to the installed Krake app, then Krake queues the links of
  the shared text and all files as one list (AC-02 applies across them), with
  the result toast of AC-05.
- [ ] AC-16 (state): While nobody is logged in, or the session expires before
  a share is queued, Krake keeps the shared content in the browser, shows the
  login, and queues it after the next successful login in that browser;
  links of a share already sent before the session expired are not sent
  twice. A second share that arrives meanwhile is queued too. Each share is
  queued exactly once, also with several Krake tabs open. Shared content older
  than 30 minutes is discarded without queueing, and the user sees a toast
  that a share expired.
- [ ] AC-17 (error): If a shared file is not text or is larger than 1 MiB,
  then that file is skipped, the toast names it with the reason, and the
  links of the other shared files and text are still queued.

### CLI

- [ ] AC-18 (event): When the user runs `krake add --list <file>`, then the
  CLI queues the links of the file by the link list rule on the local
  coordinator and prints one line per queued link and a summary with the
  result counts. Links that equal the URL of an active download count as
  *already queued* and create no download.
- [ ] AC-19 (option): Where the file argument of `--list` is `-`, the CLI
  reads the list from standard input.
- [ ] AC-20 (error): If the file of `--list` does not exist, cannot be read
  or is larger than 1 MiB (for `-`: more than 1 MiB on standard input), then the CLI prints the path and the reason,
  queues nothing and exits with a non-zero status.
- [ ] AC-21 (error): The CLI exits with a non-zero status if the list yields
  no link or every link was rejected; otherwise (at least one link queued or
  already queued) it exits with status zero.
- [ ] AC-22 (always): `--list` cannot be combined with a URL, `--nzb` or
  `--dlc` in one call; the CLI refuses that with a usage error.

## Technical notes

- The rule exists twice: TypeScript in `web-ui/src/lib/`, Rust in the core
  crate (the CLI already depends on core, and Click'n'Load's plain list
  fallback can use it later). AC-03 pins both through one fixture directory
  both test suites read, for example `crates/core/tests/fixtures/link-lists/`
  with the expected links and count next to each case. Do not use JS `\s` /
  `trim()` or Rust `trim()` blindly: AC-01 defines whitespace, and the
  fixtures with U+FEFF and U+00A0 catch the difference.
- Web UI queueing goes through `POST /api/v1/downloads/batch` (SSRF guard
  per URL, `{ids, errors}` with status 207 on partial failure), in chunks
  whose JSON body stays at or below 1 MiB, well under the 2 MiB body default
  (a 1 MiB list can grow past 2 MiB as JSON once invalid bytes become U+FFFD
  and quotes are escaped). An error
  string has the form `"<url>: <reason>"`; the client matches it by the URL
  it sent. *Already queued* is decided before sending from the download list
  the UI already holds; a link the server answers with an id that was in
  that list counts as *already queued* too.
- Android: Web Share Target with files needs `method: "POST"` and
  `enctype: "multipart/form-data"` in the manifest; the service worker
  answers that POST, stores the shared parts with a timestamp (IndexedDB) and
  redirects to the app, which picks them up after login and deletes them.
  This replaces the current GET share target.
- Android offers the share target only for an installed PWA, and Chrome
  installs a PWA only from a secure origin (HTTPS, or `localhost`). Behind
  plain `http://` on a LAN address the share target does not appear; the
  hosting docs should say so.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Line `https://host/a.rar  Part 1` | Link `https://host/a.rar` |
| Line `   https://host/a.rar` (indented) | Link `https://host/a.rar` |
| Line `HTTPS://HOST/a.rar` | Link `HTTPS://HOST/a.rar` |
| Line `# https://host/old.rar` | Skipped, not counted |
| Line `Part 1: https://host/a.rar` | Not recognized (no extraction mid-line) |
| Line `magnet:?xt=…` or `ftp://…` | Not recognized |
| Same link twice in one file | Queued once |
| Link equals an active download | Already queued, no new download |
| Link equals a completed or failed download | Queued again (a new download) |
| File with Windows line endings and a BOM | Same links as the LF version |
| File that is not valid UTF-8 | Invalid bytes replaced; lines that still match are queued |
| Empty file / only comments | "Contained no links", nothing queued |
| 0-byte shared file next to shared text | File yields no links; links of the text are queued |
| Shared text "Look at this: https://host/x." | Fallback of AC-14 queues `https://host/x` |
| Share arrives while logged out, login 40 minutes later | Share discarded, "share expired" toast |
| Two shares while logged out | Both queued after login |
| Line with `http://127.0.0.1/…` | Rejected by the private address guard; reported (AC-08) |
| 5 000 links in a 1 MiB file | Sent in several requests; all queued |
| Network drops after the 3rd of 10 requests | Links of 3 requests queued, toast says how many were not sent (AC-11) |
| Session expires in the middle of a share | Sent links stay queued; the rest waits for the next login (AC-16) |
| Two Krake tabs open when the user logs in after a share | The share is queued once |
| `krake add --list` while `krake-server` holds the same database | Same behavior as `krake add --dlc` today (not changed by this spec) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Size limit of 1 MiB per file or text | Same as the DLC upload limit; a link list of 1 MiB holds thousands of links | Someone with a huge list must split it |
| No limit on the number of links beyond the size limit | The batch endpoint has none, and chunking keeps each request small | A very long list takes a while to queue |
| Bytes that are not UTF-8 are replaced, not decoded as Windows-1252 | URLs are ASCII in practice, and guessing an encoding needs a detector both implementations must agree on | A non-ASCII path in a Windows-1252 file is mangled |
| The CLI does not apply the server's private address guard | `krake add --dlc` and direct `krake get` do not either: the CLI is the user's own downloader | None beyond today's |
| A pending share expires after 30 minutes | Long enough to log in, short enough that an old share does not surprise the user days later | User must share again after a long pause |
| Another client queues the same URL between the UI's last list update and the request | The race is a few hundred milliseconds wide and the WebSocket keeps the list current | That link counts as *queued* although no new download was created |
| A completed or failed download with the same URL does not block queueing | That is how the coordinator dedups today (active downloads only); re-downloading is a valid wish | A user may download a file twice |

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
| AC-17 |      | open   |        |
| AC-18 |      | open   |        |
| AC-19 |      | open   |        |
| AC-20 |      | open   |        |
| AC-21 |      | open   |        |
| AC-22 |      | open   |        |

## Decisions

- 2026-11-17: One link per line, the first whitespace-separated word of the
  line — chosen by the user over extraction from free text, because notes
  after a link are common and stray links in prose are not.
- 2026-11-17: Links of a file are queued flat, without a package — chosen by
  the user; packages are not wired into the data model yet.
- 2026-11-17: Shared text keeps a fallback to the first URL anywhere in the
  text (AC-14), because share sheets usually send "title + link" in one line
  and the current share target already handles that.
- 2026-11-17: Web UI and CLI only (plus the Android share target); no server
  side text parsing and no watch folder — chosen by the user.
