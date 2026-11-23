/// <reference path="../../types/krake.d.ts" />

// gofile.io — folders (`/d/<id>`) through the site's own API with a guest
// account. The file servers want the account token as `accountToken` cookie.

const API = "https://api.gofile.io";
const SITE = "https://gofile.io";
// The website token is derived from the user agent the request sends, so
// both come from here.
const USER_AGENT =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36";
const LANGUAGE = "en-US";
const WT_SALT = "5d4f7g8sd45fsd";
const WT_SLOT_SECONDS = 14400;

interface GofileReply {
    status: string;
    data?: any;
}

function folderId(url: string): string {
    const id = krake.regexMatch("gofile\\.io/d/([A-Za-z0-9-]+)", url);
    if (!id) throw new Error("gofile: not a folder link: " + url);
    return id;
}

function parseReply(body: string, what: string): GofileReply {
    try {
        return JSON.parse(body);
    } catch (e) {
        throw new Error("gofile: " + what + " returned invalid JSON");
    }
}

function baseHeaders(): Record<string, string> {
    return {
        "User-Agent": USER_AGENT,
        "Referer": SITE + "/",
        "Origin": SITE,
    };
}

/** A guest account token, created once and kept in plugin storage. */
function accountToken(fresh: boolean): string {
    if (!fresh) {
        const stored = krake.storageGet("account_token");
        if (stored) return stored;
    }
    const resp = krake.httpPost(API + "/accounts", "{}", "application/json", { headers: baseHeaders() });
    const reply = parseReply(resp.body, "account creation");
    if (reply.status !== "ok" || !reply.data || !reply.data.token) {
        throw new Error("gofile: could not create a guest account (" + reply.status + ")");
    }
    krake.storageSet("account_token", reply.data.token);
    return reply.data.token;
}

function websiteToken(token: string): string {
    const slot = Math.floor(Date.now() / 1000 / WT_SLOT_SECONDS);
    return krake.sha256(USER_AGENT + "::" + LANGUAGE + "::" + token + "::" + slot + "::" + WT_SALT);
}

function fetchContent(id: string, token: string): GofileReply {
    const query = "?contentFilter=&page=1&pageSize=1000&sortField=name&sortDirection=1";
    const headers = baseHeaders();
    headers["Authorization"] = "Bearer " + token;
    headers["X-Website-Token"] = websiteToken(token);
    headers["X-BL"] = LANGUAGE;
    const resp = krake.httpGet(API + "/contents/" + id + query, { headers });
    return parseReply(resp.body, "folder listing");
}

/** The download URL of a file entry: `link`, or `directLink` when gofile
 *  sends only that one (as JDownloader's plugin reads it). */
function fileLink(file: any): string | null {
    return file.link || file.directLink || null;
}

function fileEntry(file: any, token: string): DownloadInfo {
    return {
        url: fileLink(file)!,
        filename: file.name || null,
        filesize: typeof file.size === "number" ? file.size : null,
        chunks_supported: true,
        max_chunks: null,
        headers: { "Referer": SITE + "/" },
        cookies: { "accountToken": token },
        wait_seconds: null,
        mirrors: [],
    };
}

function folderEntry(folder: any): DownloadInfo {
    return {
        url: SITE + "/d/" + (folder.code || folder.id),
        filename: null,
        filesize: null,
        chunks_supported: false,
        max_chunks: null,
        headers: null,
        cookies: null,
        wait_seconds: null,
        mirrors: [],
    };
}

module.exports = {
    id: "gofile",
    name: "Gofile",
    version: "1.0.1",
    apiVersion: 1,
    permissions: { domains: ["api.gofile.io"] },
    description: "Downloads gofile.io folders (every file, subfolders one level deep)",
    author: "amigo-labs",
    urlPattern: "https?://(?:www\\.)?gofile\\.io/d/[A-Za-z0-9-]+",
    pluginType: "hoster" as PluginTypeHint,

    resolve(url: string): DownloadPackage {
        const id = folderId(url);
        let token = accountToken(false);
        let reply = fetchContent(id, token);
        // A stored guest account can expire: start over with a new one once.
        if (reply.status === "error-token" || reply.status === "error-notAuthenticated") {
            token = accountToken(true);
            reply = fetchContent(id, token);
        }
        if (reply.status === "error-notFound") {
            throw new Error("gofile: folder " + id + " not found");
        }
        if (reply.status === "error-passwordRequired" || reply.status === "error-passwordWrong") {
            throw new Error("gofile: folder " + id + " is password protected");
        }
        if (reply.status !== "ok" || !reply.data) {
            throw new Error("gofile: folder " + id + " failed (" + reply.status + ")");
        }

        const data = reply.data;
        if (data.type === "file") {
            return { name: data.name || id, downloads: [fileEntry(data, token)] };
        }
        if (data.password && !data.children) {
            throw new Error("gofile: folder " + id + " is password protected");
        }
        const children: any[] = Object.keys(data.children || {}).map(k => data.children[k]);
        // Files first, then subfolders, each by name ignoring case.
        const key = (c: any) => (c.type === "folder" ? "1" : "0") + String(c.name).toLowerCase();
        children.sort((a, b) => (key(a) < key(b) ? -1 : key(a) > key(b) ? 1 : 0));

        const downloads: DownloadInfo[] = [];
        for (const child of children) {
            if (child.type === "file" && fileLink(child)) {
                downloads.push(fileEntry(child, token));
            } else if (child.type === "folder") {
                downloads.push(folderEntry(child));
            }
        }
        return { name: data.name || id, downloads };
    },
} satisfies KrakePlugin;
