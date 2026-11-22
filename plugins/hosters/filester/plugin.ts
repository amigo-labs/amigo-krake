/// <reference path="../../types/krake.d.ts" />

// filester.me — files (`/d/<slug>`) and folders (`/f/<slug>`). A file page
// names the file; the public download API returns its path on the cache
// servers. A folder lists its files over numbered pages; Krake resolves
// each listed file through this plugin again (one call per file).

const ROOT = "https://filester.me";
const CACHE_ROOT = "https://cache1.filester.me";
const MIRROR_ROOT = "https://cache6.filester.me";
// One request is the first page; the rest of the budget walks further pages.
const MAX_FOLDER_PAGES = 18;

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

function decodeName(s: string): string {
    const text = unescapeHtml(s);
    try {
        return decodeURIComponent(text);
    } catch (e) {
        return text;
    }
}

function slugOf(url: string, kind: string): string {
    const slug = krake.regexMatch("filester\\.me/" + kind + "/([A-Za-z0-9_-]+)", url);
    if (!slug) throw new Error("filester: not a filester link: " + url);
    return slug;
}

function ogTitle(page: string): string | null {
    return krake.regexMatch("property=\"og:title\" content=\"([^\"]*)\"", page);
}

function resolveFile(url: string): DownloadPackage {
    const slug = slugOf(url, "d");
    const page = krake.httpGet(ROOT + "/d/" + slug);
    if (page.status === 404) throw new Error("filester: file " + slug + " not found");
    if (page.status >= 400) throw new Error("filester: file page failed (HTTP " + page.status + ")");
    const title = ogTitle(page.body);
    const name = title ? decodeName(title) : null;

    const api = krake.httpPost(ROOT + "/api/public/download", JSON.stringify({ file_slug: slug }), "application/json", {
        headers: { "Referer": ROOT + "/d/" + slug, "Origin": ROOT },
    });
    let data: any = null;
    try {
        data = JSON.parse(api.body);
    } catch (e) {
        throw new Error("filester: download API returned invalid JSON (HTTP " + api.status + ")");
    }
    if (api.status >= 400 || !data || !data.download_url) {
        const reason = data && (data.error || data.message);
        throw new Error("filester: no download for " + slug + (reason ? ": " + reason : ""));
    }
    const path = String(data.download_url);
    return {
        name: name || slug,
        downloads: [{
            url: CACHE_ROOT + path + "?download=true",
            filename: name,
            filesize: null,
            chunks_supported: true,
            max_chunks: null,
            headers: { "Referer": ROOT + "/" },
            cookies: null,
            wait_seconds: null,
            mirrors: [MIRROR_ROOT + path + "?download=true"],
        }],
    };
}

function resolveFolder(url: string): DownloadPackage {
    const slug = slugOf(url, "f");
    const downloads: DownloadInfo[] = [];
    let name: string | null = null;
    for (let pageNo = 1; pageNo <= MAX_FOLDER_PAGES; pageNo++) {
        const resp = krake.httpGet(ROOT + "/f/" + slug + "?page=" + pageNo);
        if (resp.status === 404) throw new Error("filester: folder " + slug + " not found");
        if (resp.status >= 400) throw new Error("filester: folder page failed (HTTP " + resp.status + ")");
        const page = resp.body;
        if (name === null) {
            const title = ogTitle(page);
            name = title ? unescapeHtml(title) : slug;
        }
        const items = page.split('class="file-item"').slice(1);
        for (const item of items) {
            const fileSlug = krake.regexMatch("href=['\"]/d/([A-Za-z0-9_-]+)['\"]", item);
            if (!fileSlug) continue;
            const itemName = krake.regexMatch("data-name=\"([^\"]*)\"", item);
            const size = krake.regexMatch("data-size=\"(\\d+)\"", item);
            downloads.push({
                url: ROOT + "/d/" + fileSlug,
                filename: itemName ? unescapeHtml(itemName) : null,
                filesize: size ? parseInt(size, 10) : null,
                chunks_supported: true,
                max_chunks: null,
                headers: null,
                cookies: null,
                wait_seconds: null,
                mirrors: [],
            });
        }
        if (page.indexOf(">→</a>") < 0 && page.indexOf(">&rarr;</a>") < 0) break;
        if (pageNo === MAX_FOLDER_PAGES) {
            krake.logWarn("filester: folder " + slug + " has more than " + MAX_FOLDER_PAGES + " pages; the rest is skipped");
        }
    }
    return { name: name || slug, downloads };
}

module.exports = {
    id: "filester",
    name: "Filester",
    version: "1.0.0",
    apiVersion: 1,
    permissions: { domains: ["filester.me"] },
    description: "Downloads filester.me files and folders",
    author: "amigo-labs",
    urlPattern: "https?://(?:www\\.)?filester\\.me/(?:d|f)/[A-Za-z0-9_-]+",
    pluginType: "hoster" as PluginTypeHint,

    resolve(url: string): DownloadPackage {
        return krake.regexTest("filester\\.me/f/", url) ? resolveFolder(url) : resolveFile(url);
    },
} satisfies KrakePlugin;
