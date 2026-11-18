# Hosting Krake

Krake ships three first-class deployment shapes. Pick the one that matches
where the server actually runs.

## Which variant do I need?

| Variant | Runs standalone? | Needs a server too? | Typical use |
|---|---|---|---|
| **Tauri Desktop** (`Krake`, crate `krake-desktop`) | yes — spawns an internal loopback server | no | Personal laptop/desktop, single user |
| **`krake-server` binary** (Linux/Mac/Windows) | yes — ships the Web UI embedded | no | Self-hosted on a NAS / homelab without containers |
| **Docker image** (`krake:latest`) | yes (as a container) | no | Always-on homeserver, multiple clients |
| **`krake <URL>`** (direct download) | yes — one shot, no daemon | no | Quick single-file download, `yt-dlp`-style |
| **Web UI in browser** | no — it's only a frontend | yes — served by `krake-server` or Docker | Regular GUI use |
| **`krake` queue commands** (`add`, `list`, `pause`, …) | no | yes — a running `krake-server`. Use locally by default, or pair once with a remote via `krake login <url>` and then address it with `--remote <alias>`. | Scripted queue management, talking to a Docker host from a laptop |

In short:

- **Standalone** (no extra service required): Tauri, the `krake-server` binary, the Docker container, `krake <URL>`.
- **Needs a running server** (Docker or local, doesn't matter which): browser UI, `krake` queue commands. The CLI is one binary — `login` / `remote` / `--remote` are just one-time pairing and later addressing on the same tool.

## Local (default)

Running `krake-server` (or the Tauri desktop app) with no extra config
binds to `127.0.0.1:1516`. Nothing on the network can reach the process,
so the setup wizard is skipped. The API still wants a login unless you set
`auth_mode = "never"` (see [Login for your own network](#login-for-your-own-network)),
which a loopback-only server may use.

## Docker / LAN (first-run wizard)

```sh
cd docker && docker compose up -d
```

`docker-compose.yml` sets `KRAKE_BIND=0.0.0.0:1516`, so the container
listens on every interface. On first start the server enters **setup
mode**: any HTTP request to `/api/v1/*` returns 503 `{"error":
"setup_required"}`, and the Web UI automatically shows the setup wizard.

1. Open `http://<docker-host>:1516` in a browser.
2. Create the admin account (username + password). That's it — a session
   cookie is issued, the setup flag is persisted to
   `/config/config.toml`, and the app loads.

First-visitor-wins (trust-on-first-use) is the default, matching Home
Assistant / Jellyfin / Sonarr. For public-internet deployments where that
isn't acceptable, see below.

### Provisioning options (env vars)

| Variable | Effect |
| --- | --- |
| `KRAKE_BIND` | Listen address. Default `127.0.0.1:1516` locally, `0.0.0.0:1516` in Docker. |
| `KRAKE_SETUP_PIN` | When set, the wizard requires `X-Setup-Pin: <value>` on `/api/v1/setup/*`. Recommended for public VPS / Tailscale Funnel. |
| `KRAKE_SETUP_USER` + `KRAKE_SETUP_PASSWORD` | Headless provisioning — the admin account is created at startup and the wizard never appears. Good for IaC / docker-compose. |
| `KRAKE_TRUST_PROXY` | `true` = honour `X-Forwarded-For` / `X-Forwarded-Proto`. Enable only behind a reverse proxy you control. |
| `KRAKE_AUTH_MODE` | `always` (default), `private` or `never` — who needs to log in; overrides `server.auth_mode` and locks the choice in the web UI. See [Login for your own network](#login-for-your-own-network). |
| `KRAKE_API_TOKEN` | Legacy pre-shared bearer token; accepted alongside session cookies and pairing-issued API tokens. |
| `KRAKE_AUTO_UPDATE_PLUGINS` | `true` = periodically install plugin updates from the signed registry. Defaults to off; relies on the Ed25519-signed `index.json.sig` for tamper evidence. |
| `KRAKE_PLUGIN_REGISTRY_DEV_UNSIGNED` | `1` = accept an unsigned plugin registry (disables signature verification). Local development only — never set in production. |

The variables were called `AMIGO_*` before the rename to Krake. The old
names still work when the `KRAKE_*` name is unset; the server logs a
deprecation warning for each one it picks up.

### Plugin auto-updates

The plugin registry is published at
`https://krake-registry.amigo-labs.dev/index.json` (the default of
`update.plugin_registry_url`; configs that still name the old
`amigo-downloader-plugins` URL are moved to it on load). The index
(`index.json`) is served alongside a detached Ed25519 signature
(`index.json.sig`), made with the same key as the self-update archives. The krake-server binary pins the
signer's public key, verifies the signature on every registry fetch, and
rejects any tampered or unsigned index. Per-plugin source files are
additionally SHA-256 pinned in the index.

With that tamper evidence in place, periodic auto-installs are safe to
enable. Set `auto_update_plugins = true` in `[update]` (or export
`KRAKE_AUTO_UPDATE_PLUGINS=true`). The background task then runs every
`update.check_interval_hours` hours and calls `update_all_plugins()` on
each tick; failures are logged, never fatal. New (not-yet-installed)
plugins are skipped — only plugins the admin has already installed ever
update automatically.

To rotate the registry signing key, ship a new krake-server release with
the new pinned public key. Clients that haven't upgraded will reject the
new signatures — this is the intended behaviour, it fails safe.

## Login for your own network

By default every browser and script needs a login. Like Sonarr's
"Authentication Required: Disabled for Local Addresses", Krake can skip
it for your own network. Set it under **Settings ▸ Security**, in
`config.toml`, or with `KRAKE_AUTH_MODE` (which wins and locks the
setting in the web UI):

```toml
[server]
auth_mode = "private"   # always | private | never
```

| Mode | Who gets in without a login |
| --- | --- |
| `always` | Nobody (default). |
| `private` | Requests from private addresses: `127.0.0.0/8`, `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`, `169.254.0.0/16`, `::1`, `fc00::/7`, `fe80::/10`. Everyone else logs in. |
| `never` | Everybody who reaches the server directly. Only allowed while the server listens on loopback (`127.0.0.1`, `::1`, `localhost`); otherwise it refuses to start. |

Whoever gets in without a login has full control, including the settings
and the admin password. A request is only let in without login when, in
addition:

- the browser calls Krake by an IP address, `localhost`, a name without a
  dot (`http://casa:1516`) or a name ending in `.local`, `.lan`,
  `.home.arpa` or `.internal`. A public domain name such as
  `krake.example.com` keeps the login, even if it points to a LAN
  address (protection against DNS rebinding);
- the request comes from Krake's own page, not from another website open
  in the same browser (`Origin` and `Sec-Fetch-Site` are checked).

Things to know before you choose `private`:

- **Reverse proxies.** A request that went through a proxy counts as
  private only with `KRAKE_TRUST_PROXY=true`, only when the proxy itself
  connects from a private address, and only when the first public address
  in `X-Forwarded-For` (read from the end, past every private proxy in a
  chain) is private. Without `KRAKE_TRUST_PROXY`, every proxied request
  needs a login. A proxy that forwards a public domain as `Host` keeps the
  login as well.
- **IPv6.** Phones and laptops on a home network often use global IPv6
  addresses (`2001:…`); those are not private and still need a login.
- **Forwarders that hide the client.** Docker Desktop (macOS, Windows),
  rootless Docker or Podman, Docker's userland proxy, SSH or `socat`
  tunnels and tunnel daemons hand Krake connections from a private address
  without saying where they came from. Behind any of them an internet
  request looks local: keep `always` there.
- **Docker networks and VPNs** are private ranges: other containers and
  VPN peers get in without a login.

## Reverse proxy (nginx, Caddy, Traefik)

Serve Krake behind TLS termination on a subdomain (recommended over a
sub-path). Set `KRAKE_TRUST_PROXY=true` so the pairing flow and rate
limiter see the real client IP.

### nginx

```nginx
server {
    listen 443 ssl http2;
    server_name krake.example.com;

    location / {
        proxy_pass http://127.0.0.1:1516;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $remote_addr;
        proxy_set_header X-Forwarded-Proto $scheme;

        # WebSocket upgrade for /api/v1/ws
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_read_timeout 3600s;
    }
}
```

### Caddy

```caddy
krake.example.com {
    reverse_proxy 127.0.0.1:1516 {
        header_up X-Forwarded-For {remote_host}
        header_up X-Forwarded-Proto {scheme}
    }
}
```

Caddy handles WebSocket upgrades automatically.

### Sub-path deployment

Not recommended — the Web UI bundle assumes it owns `/`. If you must,
strip the prefix in the proxy and set the Vite `base` at build time.
Using a subdomain is always easier.

## Tailscale

Bind Krake to `0.0.0.0:1516` (or to the Tailscale interface IP for
belt-and-braces) and reach it at `http://<name>.<tailnet>.ts.net:1516`.
Nothing special to configure on the Krake side — TOFU is safe because
your tailnet isn't the open internet.

Two caveats:

- **Tailscale Serve** (tls on a tailnet-only hostname) works out of the
  box. Set `KRAKE_TRUST_PROXY=true` if you want the real client IP
  instead of `127.0.0.1` in the pairing UI.
- **Tailscale Funnel** exposes your instance to the public internet. Treat
  it exactly like a VPS — set `KRAKE_SETUP_PIN` so random visitors can't
  claim the admin account before you do.

## Pairing the CLI against a remote server

From any machine that can reach the server:

```sh
krake login http://krake.example.com
# Prints a fingerprint like "472-189" and polls for approval.

# In the admin browser: click "Approve" on the pop-up device-approval
# card. The fingerprint displayed in the modal should match the one
# printed by the CLI.

krake remote list       # confirm the remote was saved
krake remote use <name> # set as default
```

`remotes.toml` is saved under `~/.config/krake/` with `0600` permissions.
