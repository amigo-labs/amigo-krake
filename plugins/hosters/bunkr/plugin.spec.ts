/// <reference path="../../types/krake.d.ts" />

// Offline tests: krake.http* is replaced with fakes that serve recorded
// bunkr pages and API answers. Nothing here reaches the real hosts.

const plugin = module.exports;

interface FakeRequest { method: string; url: string; headers: Record<string, string>; body?: string }

function install(routes: Record<string, (req: FakeRequest) => { status: number; body: string }>) {
    const seen: FakeRequest[] = [];
    const answer = (req: FakeRequest) => {
        seen.push(req);
        const path = req.url.split("?")[0];
        const route = routes[req.method + " " + path];
        if (!route) throw new Error("unexpected request " + req.method + " " + req.url);
        const r = route(req);
        return { status: r.status, body: r.body, headers: {} };
    };
    krake.httpGet = (url: string, opts?: HttpRequestOptions) =>
        answer({ method: "GET", url, headers: (opts && opts.headers) || {} });
    krake.httpPost = (url: string, body: string, _ct: string, opts?: HttpRequestOptions) =>
        answer({ method: "POST", url, headers: (opts && opts.headers) || {}, body });
    return seen;
}

function hostOf(url: string): string {
    return krake.regexMatch("^https?://([^/:?#]+)", url) || "";
}

function assertDeclared(seen: FakeRequest[]) {
    for (const r of seen) {
        assert(plugin.permissions.domains.indexOf(hostOf(r.url)) >= 0, "AC-16: undeclared host " + r.url);
    }
}

const ALBUM_PAGE = [
    "<html><head>",
    '<meta property="og:title" content="Summer &amp;amp; Sun">',
    "</head><body>",
    "<script>",
    "window.albumFiles = [",
    "{",
    " id: 101,",
    ' name: "u-1.mp4",',
    ' original: "Beach \\"day\\".mp4",',
    ' slug: "AbcDef12",',
    " size:  5242880 ,",
    ' timestamp: "10:00:00 01/11/2026",',
    "},",
    "{",
    " id: 102,",
    ' name: "u-2.jpg",',
    ' original: "sun.jpg",',
    ' slug: "GhiJkl34",',
    " size:  2048 ,",
    ' timestamp: "10:00:01 01/11/2026",',
    "}",
    "];",
    "</script></body></html>",
].join("\n");

const FILE_PAGE = [
    "<html><head>",
    '<meta property="og:title" content="My Video.mp4">',
    "</head><body>",
    '<h1 class="truncate">My Video.mp4</h1>',
    '<a class="btn" data-file-id="777001" href="https://get.bunkrr.su/file/777001">Download</a>',
    "</body></html>",
].join("\n");

// "https://c.bunkr-cache.se/abc/My Video.mp4" XOR "SECRET_KEY_489944", base64.
const ENCRYPTED = { encrypted: true, timestamp: 1763800000, url: "OzE3IjZucGQmdz1BVlJLGVcyJis3ayc6ZCQ7PBt1QBliXTcgLHwoJGs=" };

test("has required metadata", () => {
    assertEqual(plugin.id, "bunkr");
    assertNotNull(plugin.version);
    assertEqual(typeof plugin.resolve, "function");
});

test("AC-16: urlPattern matches bunkr album and file links only", () => {
    const re = new RegExp(plugin.urlPattern);
    for (const u of [
        "https://bunkr.cr/a/XyZ12345", "https://bunkr.si/f/AbcDef12", "https://bunkr.cr/v/AbcDef12",
        "https://bunkr.ac/i/AbcDef12", "https://bunkr.ws/d/AbcDef12", "https://app.bunkr.cr/a/XyZ",
        "https://bunkrr.su/f/AbcDef12",
    ]) assert(re.test(u), "should match " + u);
    for (const u of [
        "https://c.bunkr-cache.se/abc/My Video.mp4", "https://bunkr.cr/", "https://example.com/a/XyZ",
        "https://get.bunkrr.su/file/777001", "https://apidl.bunkr.ru/api/_001_v2",
    ]) assert(!re.test(u), "should not match " + u);
});

test("AC-12: an album resolves to one file link per file", () => {
    const seen = install({
        "GET https://bunkr.cr/a/XyZ12345": () => ({ status: 200, body: ALBUM_PAGE }),
    });
    const pkg = plugin.resolve("https://bunkr.cr/a/XyZ12345");
    assertEqual(pkg.name, "Summer & Sun");
    assertEqual(pkg.downloads.length, 2);
    assertEqual(pkg.downloads[0].url, "https://bunkr.cr/f/AbcDef12");
    assertEqual(pkg.downloads[0].filename, "Beach \"day\".mp4");
    assertEqual(pkg.downloads[0].filesize, 5242880);
    assertEqual(pkg.downloads[1].url, "https://bunkr.cr/f/GhiJkl34");
    const re = new RegExp(plugin.urlPattern);
    assert(pkg.downloads.every((d: DownloadInfo) => re.test(d.url)), "file links are handled by this plugin (AC-06)");
    assertDeclared(seen);
});

test("AC-13: a file link resolves to the decrypted direct URL with a Referer", () => {
    const seen = install({
        "GET https://bunkr.cr/f/AbcDef12": () => ({ status: 200, body: FILE_PAGE }),
        "POST https://apidl.bunkr.ru/api/_001_v2": (req) => {
            assertEqual(JSON.parse(req.body!).id, "777001");
            return { status: 200, body: JSON.stringify(ENCRYPTED) };
        },
    });
    const pkg = plugin.resolve("https://bunkr.cr/f/AbcDef12");
    assertEqual(pkg.downloads.length, 1);
    const d = pkg.downloads[0];
    assertEqual(d.url, "https://c.bunkr-cache.se/abc/My Video.mp4");
    assertEqual(d.filename, "My Video.mp4");
    assertEqual(d.headers!["Referer"], "https://get.bunkrr.su/file/777001");
    assert(!new RegExp(plugin.urlPattern).test(d.url), "direct URL is not resolved again");
    assertDeclared(seen);
});

test("AC-13: an unencrypted answer is used as is", () => {
    install({
        "GET https://bunkr.si/v/AbcDef12": () => ({ status: 200, body: FILE_PAGE }),
        "POST https://apidl.bunkr.ru/api/_001_v2": () => ({
            status: 200,
            body: JSON.stringify({ encrypted: false, url: "https://c.bunkr-cache.se/plain.mp4" }),
        }),
    });
    assertEqual(plugin.resolve("https://bunkr.si/v/AbcDef12").downloads[0].url, "https://c.bunkr-cache.se/plain.mp4");
});

test("a deleted file fails with a clear error", () => {
    install({
        "GET https://bunkr.cr/f/Gone0000": () => ({ status: 404, body: "" }),
    });
    let message = "";
    try {
        plugin.resolve("https://bunkr.cr/f/Gone0000");
    } catch (e) {
        message = String(e);
    }
    assert(message.indexOf("not found") >= 0, "got: " + message);
});

test("an empty album returns no downloads", () => {
    install({
        "GET https://bunkr.cr/a/Empty": () => ({ status: 200, body: "<html>window.albumFiles = [\n];</script></html>" }),
    });
    assertEqual(plugin.resolve("https://bunkr.cr/a/Empty").downloads.length, 0);
});
