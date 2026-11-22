/// <reference path="../../types/krake.d.ts" />

// bunkr — albums (`/a/<id>`) and single files (`/f/`, `/v/`, `/i/`, `/d/`).
// An album lists its files' pages; Krake resolves each page through this
// plugin again (one call per file, so large albums stay within the HTTP
// budget). A file page names a file id, which bunkr's API turns into the
// direct URL, XOR-encrypted with a key that changes every hour.

const BUNKR_HOSTS = [
    "bunkr.ac", "bunkr.ci", "bunkr.cr", "bunkr.fi", "bunkr.ph", "bunkr.pk",
    "bunkr.ps", "bunkr.si", "bunkr.sk", "bunkr.ws", "bunkr.black", "bunkr.red",
    "bunkr.media", "bunkr.site", "bunkr.su", "bunkr.ru", "bunkr.la", "bunkr.is",
    "bunkr.to", "bunkr.ax", "bunkr.cat", "bunkrr.su", "bunkrr.ru",
];
const API_HOST = "apidl.bunkr.ru";
const API_URL = "https://" + API_HOST + "/api/_001_v2";
const DL_ROOT = "https://get.bunkrr.su";

function hostAlternation(): string {
    return BUNKR_HOSTS.map(h => h.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|");
}

function rootOf(url: string): string {
    const host = krake.regexMatch("^https?://(?:app\\.)?([^/:?#]+)", url);
    if (!host) throw new Error("bunkr: not a bunkr link: " + url);
    return "https://" + host;
}

function unescapeHtml(s: string): string {
    return s
        .replace(/&#(\d+);/g, (_m, n) => String.fromCharCode(parseInt(n, 10)))
        .replace(/&#x([0-9a-fA-F]+);/g, (_m, n) => String.fromCharCode(parseInt(n, 16)))
        .replace(/&quot;/g, "\"")
        .replace(/&#39;|&apos;/g, "'")
        .replace(/&lt;/g, "<")
        .replace(/&gt;/g, ">")
        .replace(/&amp;/g, "&");
}

function between(text: string, start: string, end: string): string | null {
    const i = text.indexOf(start);
    if (i < 0) return null;
    const j = text.indexOf(end, i + start.length);
    if (j < 0) return null;
    return text.substring(i + start.length, j);
}

const B64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

function base64Bytes(input: string): number[] {
    const clean = input.replace(/[^A-Za-z0-9+/]/g, "");
    const out: number[] = [];
    let buffer = 0;
    let bits = 0;
    for (let i = 0; i < clean.length; i++) {
        buffer = (buffer << 6) | B64.indexOf(clean.charAt(i));
        bits += 6;
        if (bits >= 8) {
            bits -= 8;
            out.push((buffer >> bits) & 0xff);
        }
    }
    return out;
}

function utf8Decode(bytes: number[]): string {
    let s = "";
    for (const b of bytes) s += "%" + (b < 16 ? "0" : "") + b.toString(16);
    return decodeURIComponent(s);
}

/** bunkr's URL cipher: base64, then XOR with `SECRET_KEY_<hour>`. */
function decryptUrl(encrypted: string, timestamp: number): string {
    const key = "SECRET_KEY_" + Math.floor(timestamp / 3600);
    const bytes = base64Bytes(encrypted);
    const plain = bytes.map((b, i) => b ^ key.charCodeAt(i % key.length));
    return utf8Decode(plain);
}

function resolveAlbum(url: string): DownloadPackage {
    const root = rootOf(url);
    const id = krake.regexMatch("/a/([^/?#]+)", url)!;
    const resp = krake.httpGet(root + "/a/" + id + "?advanced=1");
    if (resp.status === 404) throw new Error("bunkr: album " + id + " not found");
    if (resp.status >= 400) throw new Error("bunkr: album " + id + " failed (HTTP " + resp.status + ")");
    const page = resp.body;

    const title = unescapeHtml(unescapeHtml(between(page, 'property="og:title" content="', '"') || id));
    const list = between(page, "window.albumFiles = [", "</script>") || "";
    const downloads: DownloadInfo[] = [];
    for (const item of list.split("\n},")) {
        const slug = krake.regexMatch("slug:\\s*\"([^\"]+)\"", item);
        if (!slug) continue;
        const original = krake.regexMatch("original:\\s*\"((?:[^\"\\\\]|\\\\.)*)\"", item);
        const size = krake.regexMatch("size:\\s*(\\d+)", item);
        downloads.push({
            url: root + "/f/" + slug,
            filename: original ? unescapeHtml(original.replace(/\\(.)/g, "$1")) : null,
            filesize: size ? parseInt(size, 10) : null,
            chunks_supported: true,
            max_chunks: null,
            headers: null,
            cookies: null,
            wait_seconds: null,
            mirrors: [],
        });
    }
    return { name: title, downloads };
}

function resolveFile(url: string): DownloadPackage {
    const resp = krake.httpGet(url);
    if (resp.status === 404) throw new Error("bunkr: file not found: " + url);
    if (resp.status >= 400) throw new Error("bunkr: file page failed (HTTP " + resp.status + ")");
    const page = resp.body;

    const fileId = krake.regexMatch("data-file-id=\"([^\"]+)\"", page);
    if (!fileId) throw new Error("bunkr: no file id on " + url);
    const heading = krake.regexMatch("<h1[^>]*>([^<]+)<", page)
        || between(page, 'property="og:title" content="', '"');
    const name = heading ? unescapeHtml(heading.trim()) : null;

    const referer = DL_ROOT + "/file/" + fileId;
    const api = krake.httpPost(API_URL, JSON.stringify({ id: fileId }), "application/json", {
        headers: { "Referer": referer, "Origin": DL_ROOT },
    });
    if (api.status >= 400) throw new Error("bunkr: file " + fileId + " is gone (HTTP " + api.status + ")");
    let data: any;
    try {
        data = JSON.parse(api.body);
    } catch (e) {
        throw new Error("bunkr: file API returned invalid JSON");
    }
    if (!data || !data.url) throw new Error("bunkr: file API returned no URL");
    const direct = data.encrypted ? decryptUrl(data.url, data.timestamp) : data.url;

    return {
        name: name || fileId,
        downloads: [{
            url: direct,
            filename: name,
            filesize: null,
            chunks_supported: true,
            max_chunks: null,
            headers: { "Referer": referer },
            cookies: null,
            wait_seconds: null,
            mirrors: [],
        }],
    };
}

module.exports = {
    id: "bunkr",
    name: "Bunkr",
    version: "1.0.0",
    apiVersion: 1,
    permissions: { domains: BUNKR_HOSTS.concat(BUNKR_HOSTS.map(h => "app." + h), [API_HOST]) },
    description: "Downloads bunkr albums and files",
    author: "amigo-labs",
    urlPattern: "https?://(?:app\\.)?(?:" + hostAlternation() + ")/(?:a|f|v|i|d)/[^/?#]+",
    pluginType: "hoster" as PluginTypeHint,

    resolve(url: string): DownloadPackage {
        return krake.regexTest("/a/[^/?#]+", url) ? resolveAlbum(url) : resolveFile(url);
    },
} satisfies KrakePlugin;
