# Krake Plugins

TypeScript/JavaScript plugin system powered by QuickJS-NG + SWC.

## Structure

```
plugins/
├── types/krake.d.ts          # Type declarations (KrakePlugin, DownloadInfo, krake.*)
├── template/plugin.ts        # Starter template — copy this
├── hosters/
│   ├── bunkr/                # bunkr albums and files
│   ├── filester/             # filester.me files and folders
│   ├── generic-http/         # Smart HTTP link finder (fallback for any URL)
│   ├── gofile/               # gofile.io folders
│   └── xfilesharing/         # 50+ XFileSharingPro sites
├── multi-hosters/            # alldebrid, premiumize, real-debrid
└── extractors/
    └── youtube/              # YouTube video downloads
```

Each plugin folder holds `plugin.ts` and, where it has tests, `plugin.spec.ts`.
Specs that must not reach the network replace `krake.httpGet` / `krake.httpPost`
with fakes serving recorded responses (see `hosters/gofile/plugin.spec.ts`).

## Creating a Plugin

1. Copy `template/` to `your-plugin/`
2. Edit `plugin.ts` — set `id`, `name`, `urlPattern`, implement `resolve()`
3. Done — auto-detected on next start

```ts
/// <reference path="../types/krake.d.ts" />

module.exports = {
    id: "my-hoster",
    name: "My Hoster",
    version: "1.0.0",
    urlPattern: "https?://(www\\.)?my-hoster\\.com/.+",

    resolve(url: string): DownloadPackage {
        const resp = krake.httpGet(url);
        const downloadUrl = krake.regexMatch('href="([^"]+\\.zip)"', resp.body);

        return {
            name: "My Download",
            downloads: [{
                url: downloadUrl || url,
                filename: null,
                filesize: null,
                chunks_supported: true,
                max_chunks: null,
                headers: null,
                cookies: null,
                wait_seconds: null,
                mirrors: [],
            }],
        };
    },
} satisfies KrakePlugin;
```

## Contract

**Required:**

| Field | Type | Description |
|---|---|---|
| `id` | `string` | Unique plugin ID |
| `name` | `string` | Display name |
| `version` | `string` | Semver version |
| `urlPattern` | `string` | Regex matching handled URLs |
| `resolve(url)` | `→ DownloadPackage` | Resolve URL into downloads |

**Optional:**

| Field | Type | Description |
|---|---|---|
| `description` | `string` | Short description |
| `author` | `string` | Author name |
| `checkOnline(url)` | `→ "online" \| "offline" \| "unknown"` | Online check |
| `login(user, pass)` | `→ boolean` | Premium account login |
| `supportsPremium()` | `→ boolean` | Has premium support? |
| `decryptContainer(data)` | `→ string[]` | DLC/CCF decryption |
| `resolveFolder(url)` | `→ string[]` | Folder → file URLs |

## resolve() Return Type

```ts
interface DownloadPackage {
    name: string;              // Package name (shown in UI)
    downloads: DownloadInfo[]; // One or more files
}

interface DownloadInfo {
    url: string;               // Direct download URL
    filename: string | null;   // Filename, or null for auto-detect
    filesize: number | null;   // Size in bytes, or null
    chunks_supported: boolean; // Parallel download?
    max_chunks: number | null; // Max parallel chunks
    headers: Record | null;    // Extra HTTP headers
    cookies: Record | null;    // Cookies to send
    wait_seconds: number | null; // Countdown before download
    mirrors: string[];         // Alternative URLs
}
```

## Testing

```bash
# Run spec file (plugin.spec.ts)
krake plugins test ./plugins/youtube/plugin.ts

# Resolve a URL directly
krake plugins test ./plugins/youtube/plugin.ts https://youtube.com/watch?v=dQw4w9WgXcQ
```

Spec files use `test()`, `assert()`, `assertEqual()`, `assertNotNull()`:

```ts
const plugin = module.exports;

test("metadata", () => {
    assertEqual(plugin.id, "youtube");
    assertNotNull(plugin.version);
});

test("url pattern", () => {
    assert(new RegExp(plugin.urlPattern).test("https://youtube.com/watch?v=abc"));
});
```

## Host API

All access goes through `krake.*` — plugins run sandboxed with no direct network/filesystem access.

| Function | Description |
|---|---|
| `krake.httpGet(url)` | GET → `HttpResponse {status, body, headers}` |
| `krake.httpPost(url, body, contentType)` | POST → `HttpResponse` |
| `krake.httpHead(url)` | HEAD → `HttpResponse {status, headers}` |
| `krake.setCookie(domain, name, value)` | Set cookie |
| `krake.getCookie(domain, name)` | Get cookie value |
| `krake.clearCookies(domain)` | Clear domain cookies |
| `krake.regexMatch(pattern, text)` | First capture group |
| `krake.regexMatchAll(pattern, text)` | All matches |
| `krake.base64Encode(input)` | Base64 encode |
| `krake.base64Decode(input)` | Base64 decode |
| `krake.storageGet(key)` | Read persistent value |
| `krake.storageSet(key, value)` | Write persistent value |
| `krake.storageDelete(key)` | Delete persistent value |
| `krake.logInfo(msg)` | Log info |
| `krake.logWarn(msg)` | Log warning |
| `krake.logError(msg)` | Log error |
| `krake.logDebug(msg)` | Log debug |
| `console.log/warn/error` | Also available |

## Publishing to the registry

Plugins under `hosters/`, `multi-hosters/` and `extractors/` are published to
the plugin registry at `https://krake-registry.amigo-labs.dev` on every push to
`main` (`.github/workflows/registry-publish.yml`). Servers offer them in the
marketplace (`krake plugins search`, `install`, `update`) and update installed
copies from there.

- **Raise `version` whenever you change a plugin.** Every published version is
  kept under `plugins/<id>/<version>/` and never overwritten; the pull-request
  check "Plugin registry check" fails when a plugin's source changed but its
  version is not higher than the published one.
- The index entry comes from loading the plugin with the real runtime: `id`,
  `name`, `version`, `description`, `author`, `urlPattern`, `apiVersion`,
  `permissions.domains`, and `pluginType` as its tag. A plugin that does not
  load, or whose id, SemVer version, host-API version or `urlPattern` is
  invalid, stops the publish.
- Try it locally (unsigned, nothing is pushed):

  ```sh
  cargo build -p krake-cli -p krake-plugin-runtime --bins
  ./target/debug/krake plugins registry build --out /tmp/registry
  ./target/debug/krake plugins registry check   # compares with the published branch
  ```

Details: `docs/specs/plugin-registry.md`.
