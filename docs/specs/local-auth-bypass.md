---
status: todo
created: 2026-11-18
size: M
design: [AGENTS.md "REST API", AGENTS.md Entscheidung 18, docs/hosting.md, docs/architecture.md]
surfaces: [rest, web-ui, docker]
---

# Optional login for private networks

## Context

Every `/api/v1/*` request and the WebSocket need a login today: a session
cookie, a paired API token or the pre-shared `api_token`. On a home server
reached only from the LAN that is friction — the user asked for what Sonarr
and Radarr offer as "Authentication Required: Disabled for Local Addresses",
plus a mode without any login for a server that only listens on loopback.

Facts this spec builds on:

- The server already derives a client IP (peer address, or the first
  `X-Forwarded-For` / `Forwarded` entry when `trust_proxy` is on) and uses it
  only for rate limits.
- The server checks no `Origin` or `Host` header. Today the `SameSite=Strict`
  session cookie is what keeps other websites out; without a login that
  protection is gone, so this spec adds its own.
- An authenticated principal has full rights, including `PUT /config` on the
  `server` section. A request let in by this feature has the same rights.
- The desktop app spec (`desktop-app.md`, AC-5) adds no loopback trust of its
  own; this feature is an explicit, opt-in setting and does not change that.

## Scope

### In scope

- A login mode with three values: **always** (default, today's behavior),
  **private** (no login for requests from private networks), **never** (no
  login for direct requests on this machine, only allowed while the server
  listens on loopback only; forwarded requests are checked as in
  **private**).
- The mode in the config file, an environment variable that overrides it,
  and a Security section in the web UI settings.
- Protection against other websites and DNS rebinding while a request is let
  in without login.
- The web UI skips its login page when the server lets it in without login.

### Out of scope

- A user-defined list of trusted networks — because the user chose the fixed
  set of private ranges (as Sonarr does).
- A list of extra host names (for example a public domain that resolves to
  a LAN address) for requests without login — because the local host name
  rule covers IP addresses and local names; such a request still works with
  a login.
- Click'n'Load (port 9666) and the NZBGet JSON-RPC API — because they have
  their own access rules (loopback only, HTTP Basic auth) that this mode does
  not touch.
- A logout button, a password change dialog or session management in the
  web UI — because the server offers no dedicated endpoints for them yet
  (the password hash can only be replaced through `PUT /config`); that is
  its own spec.
- Per-user rights for requests without login — because Krake has one admin
  account and every authenticated request already has full rights.
- A trusted list of proxy addresses — because `trust_proxy` is all or
  nothing today, and AC-05 accepts a forwarded address only from a peer that
  is itself private.

## Acceptance criteria

Terms used below:

- **Private address** — an IPv4 address in 127.0.0.0/8, 10.0.0.0/8,
  172.16.0.0/12, 192.168.0.0/16 or 169.254.0.0/16, or an IPv6 address `::1`,
  in fc00::/7 or fe80::/10, or an IPv4-mapped IPv6 address of a private IPv4
  address.
- **Local host name** — the request has exactly one `Host` (or HTTP/2
  `:authority`), and its host part (without port) is an IP literal,
  `localhost`, a name without a dot (for example `casa`), or a name ending in
  `.local`, `.lan`, `.home.arpa` or `.internal` (case-insensitive). The rule
  is against DNS rebinding; the address rule (AC-02, AC-05) decides who is
  local.
- **Same origin** — the request carries no `Origin` header or exactly one
  whose host equals the host of `Host` (case-insensitive) and whose port
  equals the port of `Host`, where a missing port on either side means the
  default port of the `Origin`'s scheme (80 for `http`, 443 for `https`);
  `Origin: null` is not same origin. In addition, a `Sec-Fetch-Site` header, when present, is
  `same-origin` or `none`.
- **Let in without login** — the request is served with full rights, as
  the pre-shared `api_token` is today, although it carries no valid
  session, token or `api_token`.
- **Loopback bind** — the address the server listens on (the config's
  `server.bind`, or `KRAKE_BIND` when set) has the host `localhost` or a
  loopback IP literal.

### Modes

- [ ] AC-01 (always): Krake's login mode is **always** unless the config or
  the environment sets another mode. In mode **always** no request is let in
  without login, and every existing authentication test passes unchanged.
- [ ] AC-02 (option): Where the login mode is **private**, a request to the
  REST API or the WebSocket is let in without login if its client address
  (AC-05) is a private address, its `Host` is a local host name and it is
  same origin; any other request without a valid login gets `401` as
  before.
- [ ] AC-03 (option): Where the login mode is **never**, a request to the
  REST API or the WebSocket is let in without login if its `Host` is a local
  host name and it is same origin; a request that carries `X-Forwarded-For`
  or `Forwarded` is checked as in mode **private**.
- [ ] AC-04 (error): If the login mode is **never** without a loopback bind,
  then the server refuses to start and names the setting and the bind
  address. A config change through the API that would result in mode
  **never** without a loopback bind (a change of the mode, of the bind, or
  both) is refused with `400` and the config stays unchanged.

### Address checks

- [ ] AC-05 (always): The client address for AC-02 is the peer address of
  the connection. Only while `trust_proxy` is on and the peer address itself
  is a private address (the proxy), and the request carries
  `X-Forwarded-For` (all header lines read as one list; else the `for=`
  entries of `Forwarded`), the client address is found by reading that list
  from the end, ports removed, skipping private addresses: the first entry
  that is not a private address is the client address; if every entry is a
  private address, the client address is the first entry. While
  `trust_proxy` is off, or the peer address is not private, a request that
  carries `X-Forwarded-For` or `Forwarded` is never let in without login. A
  list entry read before the client address is found that is not an IP
  address (`unknown`, an obfuscated name, garbage) means the request is
  never let in without login.
- [ ] AC-06 (always): A request that carries a valid login is served as that
  principal in every mode. A request with an invalid, expired or revoked
  login that meets AC-02 or AC-03 is let in without login.

### What a request without login sees

- [ ] AC-07 (state): While a request is let in without login, it sees every
  download and every WebSocket event (as the admin does), downloads it adds
  belong to no user, and `GET /api/v1/me` reports that it was let in without
  login and under which mode.
- [ ] AC-08 (event): When the config changes (login mode, `trust_proxy` or
  anything else that AC-02 to AC-05 depend on), then every open WebSocket
  that was let in without login is checked again, and the server closes each
  one that would no longer be let in.

### Configuration

- [ ] AC-09 (option): Where the environment variable `KRAKE_AUTH_MODE` is
  set (non-empty after trimming, case-insensitive), it overrides the config
  file. The config read through the API then reports the mode in effect and
  that the environment sets it; a config change through the API that sends a
  different mode is refused with `400`, one that sends the mode in effect is
  accepted and leaves the file's value as it was. The web UI shows the mode
  as set by the environment and does not offer to change it.
- [ ] AC-10 (error): If the config file or `KRAKE_AUTH_MODE` holds a value
  other than `always`, `private` or `never`, then the server refuses to start
  and names the value and the allowed values; through the API such a value
  is refused with `400`.
- [ ] AC-11 (event): When the login mode is changed through the API, then
  every request that starts after the change is checked under the new mode,
  without a restart. Config changes through the API are validated and stored
  one after another, each against the config it replaces, so two concurrent
  changes cannot together reach a state that AC-04 refuses.

### Web UI

- [ ] AC-12 (state): While the server lets the browser in without login, the
  web UI opens directly without its login page. When a later request gets
  `401` (the mode was switched back), the web UI shows its login page.
- [ ] AC-13 (event): When the user opens Settings ▸ Security, then the UI
  shows the three modes, each with an explanation of who gets in without
  login, the current mode, and a warning that in **private** and **never**
  every device that gets in has full control, including the config and the
  admin password, and that behind Docker Desktop, rootless Docker or Podman,
  or a tunnel every request may look local, so such setups keep **always**.
- [ ] AC-14 (state): While the server has no loopback bind, the mode
  **never** is shown disabled with the reason.
- [ ] AC-15 (event): When the user switches to **private** or **never**, then
  the UI asks for confirmation first; nothing changes if the user cancels.
- [ ] AC-16 (always): Every new user-visible string of this spec exists in
  English and German.

### Docs

- [ ] AC-17 (always): The hosting documentation describes the three modes
  and `KRAKE_AUTH_MODE`; that requests through a reverse proxy (or a chain
  of proxies) count as private only with `trust_proxy` on and only when the
  proxy forwards a local host name (a public domain keeps the login); that IPv6 clients with
  global addresses need a login; and that whatever forwards connections
  without `X-Forwarded-For` (Docker Desktop, rootless Docker or Podman,
  Docker's userland proxy, SSH or socat tunnels, tunnel daemons) makes
  internet requests look private, so such setups must
  stay on **always**.

## Technical notes

- Config: `server.auth_mode = "always" | "private" | "never"` in the TOML
  (missing means `always`, so existing config files keep working), read live
  per request like `trust_proxy`. `GET /config` returns it; `PUT /config`
  validates the resulting config (AC-04, AC-09, AC-10).
- Auth: a new principal (for example `Principal::Local { mode }`) set by the
  auth middleware when no credential matches and the conditions hold; it is
  unowned like `Preshared`. `/me` could return
  `{"kind": "local", "mode": "private" | "never"}`.
- `client_ip` today returns the *first* forwarded hop, which a client can
  forge; AC-05 reads the list from the end instead (the usual
  "rightmost non-trusted address" rule with private addresses as trusted
  proxies). The rate limiters may keep
  their behavior.
- `setup_guard` and the setup wizard are unchanged: a server that has not
  finished setup behaves as today in every mode.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Mode private, phone at `192.168.1.20` opens `http://casa:1517` | Let in without login |
| Mode private, same phone opens `http://krake.example.com` (resolves to LAN IP) | Login needed (not a local host name) |
| Mode private, a public website in the LAN browser sends `fetch("http://192.168.1.10:1516/api/v1/downloads", {method:"POST"})` | `Origin` / `Sec-Fetch-Site: cross-site` → `401` |
| Mode private, a public website embeds `<img src="http://192.168.1.10:1516/api/v1/updates/check">` | `Sec-Fetch-Site: cross-site` → `401` |
| Mode private, DNS rebinding: `evil.example` resolves to `192.168.1.10` | Not a local host name → `401` |
| Mode private, nginx on the same host, `trust_proxy` off, request from the internet | Carries `X-Forwarded-For` → login needed |
| Mode private, `trust_proxy` on, client sends `X-Forwarded-For: 192.168.1.5`, proxy appends `203.0.113.7` | Last entry public → login needed |
| Mode private, `trust_proxy` on, internet client connects directly (port forward) with forged `X-Forwarded-For: 192.168.1.5` | Peer address public → login needed |
| Mode private, `trust_proxy` on, LAN client connects directly without forwarded headers | Peer address private → let in without login |
| Mode private, cloudflared container → nginx container, internet client `198.51.100.4` | List `198.51.100.4, 172.18.0.3` → first non-private from the end is public → login needed |
| Mode private, `GET /config` while `KRAKE_AUTH_MODE=private`, then the UI saves an unrelated setting | PUT carries the mode in effect → accepted, file value kept |
| Mode private, `trust_proxy` switched off while a phone behind the proxy has the UI open | Its WebSocket is closed (AC-08); next request needs login |
| Mode private, request from another Docker container (`172.18.0.5`) | Private → let in without login (documented) |
| Mode never, `KRAKE_BIND=0.0.0.0:1516` | Server refuses to start (AC-04) |
| Mode never, `PUT /config` sets `server.bind = "0.0.0.0:1516"` | `400`, config unchanged (AC-04) |
| Mode never, nginx on the same host forwards an internet request | Carries `X-Forwarded-For` → checked as private → login needed (AC-03) |
| `curl` from the LAN without `Origin`, `Host: 192.168.1.10:1516`, mode private | Let in without login |
| Expired session cookie, mode private, request from the LAN | Let in without login (AC-06) |
| `KRAKE_AUTH_MODE=Private ` and `auth_mode = "always"` in the file | Mode private; UI shows it as set by the environment |
| `KRAKE_AUTH_MODE=""` | Treated as unset; the file decides |
| Mode switched from private to always with the web UI open on a phone | Its WebSocket is closed, the next request gets `401`, the login page shows |
| IPv6 phone with a global address (`2001:db8::5`) | Not private → login needed (documented) |
| Server still in setup mode (wizard not finished) | Setup guard answers as today in every mode |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Requests let in without login get full admin rights | Krake has one admin account and every authenticated request already has full rights; Sonarr behaves the same | Anyone who gets in can change the config or the admin password |
| Docker bridge and VPN ranges count as private | They are in the private ranges; the user chose the fixed set | A container or VPN peer gets in without login |
| IPv6 global addresses are not private | A fixed list cannot know the LAN's prefix; the user chose the fixed set | IPv6 clients on the LAN must log in |
| The peer address is the real client unless a proxy says otherwise | Krake cannot see through forwarders that drop the client address | Setups with such forwarders let the internet in; AC-17 tells them to stay on **always** |
| An invalid or revoked login falls back to the network rule | The network rule alone decides who gets in without login; a stale cookie should not lock a LAN user out | A revoked token keeps working from the LAN while the mode allows it |
| `Sec-Fetch-Site` and `Origin` keep browsers out of cross-site requests | Every current browser sends `Sec-Fetch-Site`; old browsers still send `Origin` on non-GET requests | An old browser allows cross-site GET without either header; acceptable because no Krake GET endpoint changes state |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `crates/server/tests/local_auth_tests.rs`::always_is_the_default_and_needs_a_login + the unchanged `auth_flow_tests.rs`, `api_tests.rs`, `ws_tests.rs` | built |        |
| AC-02 | `crates/server/src/local_auth.rs` tests (private_ranges, local_host_names, same_origin_rules, modes) + `crates/server/tests/local_auth_tests.rs`::private_lets_local_requests_in, ::private_refuses_other_hosts_and_origins | built |        |
| AC-03 | `crates/server/src/local_auth.rs` tests::modes + `crates/server/tests/local_auth_tests.rs`::never_lets_direct_requests_in_on_a_loopback_bind | built |        |
| AC-04 | `crates/server/tests/local_auth_tests.rs`::never_needs_a_loopback_bind_through_the_api, ::startup_refuses_unknown_modes_and_never_without_loopback | built |        |
| AC-05 | `crates/server/src/local_auth.rs` tests (client_address_*, forwarded_*, unreadable_forwarded_entries_are_never_let_in) + `crates/server/tests/local_auth_tests.rs`::forwarded_requests_need_trust_proxy | built |        |
| AC-06 | `crates/server/tests/local_auth_tests.rs`::a_valid_login_wins_and_an_invalid_one_falls_back | built |        |
| AC-07 | `crates/server/tests/local_auth_tests.rs`::me_reports_the_mode_and_added_downloads_belong_to_no_one, ::websocket_without_login_sees_events_and_is_closed_when_the_mode_changes | built |        |
| AC-08 | `crates/server/tests/local_auth_tests.rs`::websocket_without_login_sees_events_and_is_closed_when_the_mode_changes | built |        |
| AC-09 | `crates/server/tests/local_auth_tests.rs`::environment_mode_overrides_the_file_and_is_fixed, ::startup_refuses_unknown_modes_and_never_without_loopback | built |        |
| AC-10 | `crates/server/tests/local_auth_tests.rs`::unknown_modes_are_refused_through_the_api, ::startup_refuses_unknown_modes_and_never_without_loopback | built |        |
| AC-11 | `crates/server/tests/local_auth_tests.rs`::a_mode_change_applies_to_the_next_request, ::concurrent_changes_cannot_combine_into_a_refused_state | built |        |
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |
| AC-14 |      | open   |        |
| AC-15 |      | open   |        |
| AC-16 |      | open   |        |
| AC-17 |      | open   |        |

## Decisions

- 2026-11-18: Three modes always / private / never — chosen by the user;
  `never` only on loopback binds, because a LAN-reachable server without any
  check is what `private` covers more safely.
- 2026-11-18: Fixed private ranges instead of a list — chosen by the user,
  like Sonarr.
- 2026-11-18: Config file, environment variable and a Settings section —
  chosen by the user.
- 2026-11-18: Forwarded requests count as private only with `trust_proxy`,
  only from a private peer and only by the last hop (AC-05), because a same-host reverse proxy would
  otherwise make every internet request look like loopback, and the first
  `X-Forwarded-For` entry is set by the client.
- 2026-11-18: Host, Origin and `Sec-Fetch-Site` checks are part of the feature, because
  without a login cookie nothing else stops a website in the user's browser
  (CSRF) or a rebinding domain from driving the API.
- 2026-11-18 (spec-build): `never` through the API also needs the listener
  bound to loopback at startup, not only the new `server.bind`, because a
  bind change takes effect only after a restart; until then the server keeps
  listening where it started.
- 2026-11-18 (spec-build): The config keeps `server.auth_mode` as a string
  (validated at startup and on `PUT /config`) so an unknown value in the file
  is reported by name instead of silently falling back to the defaults, which
  is what a failed TOML parse does today. `server.auth_mode_from_env` exists
  only in API responses.
- 2026-11-18 (spec-build): An open WebSocket let in without a login is
  checked again after every `PUT /config` (a change counter in the app
  state), not only on mode changes, so switching off `trust_proxy` closes it
  too.
