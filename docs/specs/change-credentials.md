---
status: todo
created: 2026-11-26
size: M
design: [AGENTS.md "REST API", AGENTS.md Entscheidung 18 (login optional for private networks), docs/specs/local-auth-bypass.md]
surfaces: [rest, web-ui]
---

# Change the admin login

## Context

Krake has exactly one admin account. Its username and password are chosen
once, in the setup wizard or through `KRAKE_SETUP_USER` /
`KRAKE_SETUP_PASSWORD`, and stored in `config.toml` (`server.admin_username`,
`server.admin_password_hash`, Argon2id). After setup there is no way to change
them in the web UI: a user who picked a weak password at setup, or shares the
password with someone who should lose access, has to edit `config.toml` by
hand and compute a password hash.

`docs/specs/local-auth-bypass.md` listed a password change dialog as out of
scope; this spec adds it. The generic `PUT /api/v1/config` currently passes
`admin_username` and `admin_password_hash` through as part of the config
document, so any client with config access could overwrite them without the
rules below. This spec moves both fields to the account change alone, because a config
document read before a login change would otherwise undo it when saved (AC-11).

## Scope

### In scope

- Changing the admin username, the admin password, or both, under Settings ▸
  Security in the web UI (also in the desktop app, which shows the same UI).
- A REST endpoint for the change.
- Signing out the other browser sessions after a change.
- Locking the credential fields out of the generic config update.

### Out of scope

- Asking for the current password before a change — because the user decided
  that being signed in is enough; whoever has a session (or is let in without
  login by `server.auth_mode`) already has full control over Krake.
- Resetting a forgotten password without the UI — because the user did not
  pick it for this spec; editing `config.toml` stays the way out.
- More than one account or roles — because Krake has one admin by design.
- Changing the NZBGet-compatible API credentials (`nzbget_api`) — because they
  are a separate setting with their own field in Settings.
- `config.toml` edited by hand while the server runs — because every config write already overwrites the file with the server's in-memory state; this feature changes nothing about it.
- A rate limit on the change — because the endpoint checks no secret, so there is nothing to guess.
- Revoking API tokens or paired devices on a change — because the user chose to
  keep them; they are revoked one by one where they are managed.

## Acceptance criteria

- [ ] AC-01 (event): When a signed-in user submits a new password of at least 8 characters, then the server stores it, and the next login succeeds with the new password and fails with the old one.
- [ ] AC-02 (event): When a signed-in user submits a new username, then the next login succeeds with the new username and fails with the old one, and the password stays the same.
- [ ] AC-03 (event): When the username and the password are changed in one submission, then both take effect together, and a login with the new username and the new password succeeds.
- [ ] AC-04 (always): Krake trims leading and trailing whitespace from a submitted username; a username field that is missing, or equal to the current username after trimming, keeps the username, and a password field that is missing or empty keeps the password.
- [ ] AC-05 (error): If a submitted username is empty after trimming, longer than 256 bytes in UTF-8 or contains a control character, or is the reserved name `desktop`, or a submitted non-empty password is shorter than 8 or longer than 1 024 bytes in UTF-8 (the unit setup already counts in), or the body is not valid JSON of the expected shape, then the server refuses the change with a `4xx` status and a JSON body `{ "error": "<code>" }` — `invalid_username`, `invalid_password` or `invalid_body` — and the stored login is unchanged.
- [ ] AC-06 (event): When the login is changed (a new username, or any non-empty password, even one equal to the old), then every browser session of the admin username except the one that made the change is signed out (its next API request is treated as having no login: `401`, unless `server.auth_mode` lets it in without one); the session that made the change stays signed in, and if it is an admin session, `GET /api/v1/me` reports the new username for it.
- [ ] AC-07 (event): When the login is changed, then API tokens, paired devices and desktop app sessions keep working.
- [ ] AC-08 (error): If the change is requested with an API token or the pre-shared token, then the server refuses it with `403` and `{ "error": "forbidden_principal" }`, and the stored login is unchanged.
- [ ] AC-09 (state): While `server.auth_mode` lets a request in without login, that request may change the login like a signed-in session; as no session made it, every browser session of the admin username is signed out.
- [ ] AC-10 (always): `PUT /api/v1/config` never changes the admin username or the admin password hash: values for them in the submitted config are ignored, the request otherwise succeeds as before, and `GET /api/v1/config` keeps returning the username and the redacted hash.
- [ ] AC-11 (event): When a `PUT /api/v1/config` built from a config read before a login change is saved after that change, then the new login stays; and a login change does not undo a settings change saved before it.
- [ ] AC-12 (state): While the user is on Settings ▸ Security, the web UI shows the current username and shows the current username in an editable field, a new password field and the new password again; saving is impossible while the two password entries differ, and a refused change shows a message for the server's error code without clearing the form.
- [ ] AC-13 (event): When a change is saved in the web UI, then a success message appears, both password fields are emptied and the new username is shown, without the user having to sign in again.
- [ ] AC-14 (always): Every text of the account form under Settings ▸ Security, including its success and error messages, exists in English and German.
- [ ] AC-15 (event): When the username is changed, then downloads added under the old username belong to the new one, so the admin's sessions keep receiving their live updates.

## Technical notes

Proposed API shape (the build may adjust names, not behaviour):
`PUT /api/v1/account` with body `{ "username"?: string, "password"?: string }`,
answering `200 { "username": "<new>" }`. A body that changes nothing answers
`200` and signs out nobody.

Like every config write, the change must not be lost to, or undo, a concurrent
`PUT /config` (spec `local-auth-bypass`, AC-11).

Downloads record the username of the session that added them, and live
WebSocket events reach a login session only for its own downloads; hence AC-15.
The name `desktop` is the desktop app's session principal, so an admin may not
take it (AC-05).

The desktop app's session belongs to the principal `desktop`, not to the admin
username. A desktop session may change the login (it is the owner at the
machine); `/me` keeps reporting `desktop` for it. "Browser sessions of the
admin username" are the sessions created by `/login` and the setup wizard.

No rate limit beyond the existing ones: the endpoint needs a login (or the
`auth_mode` exception) and checks no secret, so there is nothing to guess.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Submission repeats the current username and leaves the password empty | `200`, nothing stored, no session signed out |
| New username differs only by surrounding spaces | Trimmed; counts as unchanged if equal to the current one |
| Password of exactly 8 characters, or with leading/trailing spaces | Accepted as typed; spaces are part of the password (setup does the same) |
| Very long username or password (e.g. 10 000 characters) | Refused (AC-05), so a request cannot make the server hash megabytes |
| Username with a newline, tab or other control character | Refused (AC-05): it could not be typed back at the login |
| Username with non-ASCII letters (`Jürgen`) | Accepted and compared exactly as stored, like at login today |
| Two browser tabs of the same session change the login one after the other | Both succeed; the later change wins; the session stays signed in |
| Another browser sends its next request after the change | `401`; the UI shows the login page |
| `KRAKE_SETUP_USER` / `KRAKE_SETUP_PASSWORD` still set at the next start | Ignored as today, because setup is complete; the changed login stays |
| Saving the config file fails (disk full, read-only) | `500`, the running server and `config.toml` keep the old login, no session is signed out |
| Two different admin sessions A and B change the login one after the other (`auth_mode` `always`) | A's change signs out B; B's change, arriving after that, gets `401` and changes nothing |
| Change requested while setup is not complete | Answered like every other API route before setup (`503 setup_required`); the setup wizard creates the login |
| Username field cleared in the form | Saving is refused in the form and by the server (`invalid_username`) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| API tokens and the pre-shared token may not change the login (AC-08) | A leaked script token should not be able to take over the account; the user chose not to require the current password, so the principal is the remaining guard | A user who manages Krake only through scripts cannot change the login without the UI |
| Desktop app sessions may change the login | The desktop app runs on the owner's machine and opens with full rights | None beyond the desktop sign-in |
| Requests let in without login may change the login (AC-09) | They already have full control, including the config (Entscheidung 18) | None beyond what `auth_mode` already allows |
| Desktop app sessions survive a change (AC-07) | They are not bound to the admin credentials; signing them out would break the desktop window until the next launch | None |
| Length limits 256 (username) / 1 024 (password) | Bounds the work of hashing; far above any real login | A password manager producing > 1 024 characters is refused |
| Same minimum password length as setup (8 bytes) | One rule for the same secret | A short non-ASCII password counts its bytes, not its letters |

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

## Decisions

- 2026-11-26: No current password required — the user's choice; the guard is
  the principal (AC-08) instead.
- 2026-11-26: Other browser sessions are signed out, API tokens, paired
  devices and desktop sessions stay — the user's choice.
- 2026-11-26: Config writes are atomic now (separate fix), so a failed save
  leaves the old login on disk; the out-of-scope item about it is gone.
