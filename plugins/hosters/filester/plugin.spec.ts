/// <reference path="../../types/krake.d.ts" />

// Offline tests: krake.http* is replaced with fakes that serve recorded
// filester.me pages and API answers. Nothing here reaches the real host.

const plugin = module.exports;

interface FakeRequest { method: string; url: string; headers: Record<string, string>; body?: string }

function install(routes: Record<string, (req: FakeRequest) => { status: number; body: string }>) {
    const seen: FakeRequest[] = [];
    const answer = (req: FakeRequest) => {
        seen.push(req);
        const route = routes[req.method + " " + req.url] || routes[req.method + " " + req.url.split("?")[0]];
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

const FILE_PAGE =
    '<html><head><meta property="og:title" content="Report%202026 &amp; notes.pdf"></head>' +
    '<body><span>UUID</span><span>abc</span></body></html>';

function folderPage(items: [string, string, number][], more: boolean): string {
    let html = '<html><head><meta property="og:title" content="Docs &amp; Stuff"></head><body>';
    for (const [slug, name, size] of items) {
        html += '<button class="file-item" data-name="' + name + '" data-size="' + size +
            '" data-date="2026-11-01T10:00:00Z"><a href=\'/d/' + slug + '\'>open</a></button>';
    }
    if (more) html += '<a href="?page=2">→</a>';
    return html + "</body></html>";
}

test("has required metadata", () => {
    assertEqual(plugin.id, "filester");
    assertNotNull(plugin.version);
    assertEqual(typeof plugin.resolve, "function");
});

test("AC-16: urlPattern matches file and folder links only", () => {
    const re = new RegExp(plugin.urlPattern);
    assert(re.test("https://filester.me/d/aB3-x_9"), "file link");
    assert(re.test("https://filester.me/f/Folder1"), "folder link");
    assert(re.test("https://www.filester.me/d/aB3"), "www file link");
    assert(!re.test("https://cache1.filester.me/files/aB3/report.pdf?download=true"), "cache link");
    assert(!re.test("https://filester.me/"), "home page");
    assert(!re.test("https://example.com/d/aB3"), "other host");
});

test("AC-14: a file link resolves to its download URL and name", () => {
    const seen = install({
        "GET https://filester.me/d/aB3": () => ({ status: 200, body: FILE_PAGE }),
        "POST https://filester.me/api/public/download": (req) => {
            assertEqual(JSON.parse(req.body!).file_slug, "aB3");
            return { status: 200, body: JSON.stringify({ download_url: "/files/aB3/report.pdf" }) };
        },
    });
    const pkg = plugin.resolve("https://filester.me/d/aB3");
    assertEqual(pkg.downloads.length, 1);
    const d = pkg.downloads[0];
    assertEqual(d.url, "https://cache1.filester.me/files/aB3/report.pdf?download=true");
    assertEqual(d.filename, "Report 2026 & notes.pdf");
    assert(!new RegExp(plugin.urlPattern).test(d.url), "download URL is not resolved again");
    assertDeclared(seen);
});

test("filester.si links are the same site and resolve through filester.me", () => {
    const re = new RegExp(plugin.urlPattern);
    assert(re.test("https://filester.si/d/7ZmUjr1"), "filester.si file link");
    assert(re.test("https://filester.si/f/Folder1"), "filester.si folder link");
    assert(!re.test("https://cache1.filester.si/files/aB3/a.png?download=true"), "cache link");
    const seen = install({
        "GET https://filester.me/d/7ZmUjr1": () => ({ status: 200, body: FILE_PAGE }),
        "POST https://filester.me/api/public/download": (req) => {
            assertEqual(JSON.parse(req.body!).file_slug, "7ZmUjr1");
            return { status: 200, body: JSON.stringify({ download_url: "/files/7ZmUjr1/a.png" }) };
        },
        "GET https://filester.me/f/Folder1": () => ({ status: 200, body: folderPage([["s1", "one.png", 1]], false) }),
    });
    const pkg = plugin.resolve("https://filester.si/d/7ZmUjr1");
    assertEqual(pkg.downloads[0].url, "https://cache1.filester.me/files/7ZmUjr1/a.png?download=true");
    const folder = plugin.resolve("https://filester.si/f/Folder1");
    assertEqual(folder.downloads[0].url, "https://filester.me/d/s1");
    assertDeclared(seen);
});

test("AC-14: an API refusal fails with its reason", () => {
    install({
        "GET https://filester.me/d/priv": () => ({ status: 200, body: FILE_PAGE }),
        "POST https://filester.me/api/public/download": () => ({
            status: 403,
            body: JSON.stringify({ error: "File is private" }),
        }),
    });
    let message = "";
    try {
        plugin.resolve("https://filester.me/d/priv");
    } catch (e) {
        message = String(e);
    }
    assert(message.indexOf("File is private") >= 0, "got: " + message);
});

test("AC-15: a folder resolves to one file link per file across its pages", () => {
    const seen = install({
        "GET https://filester.me/f/Folder1?page=1": () => ({
            status: 200,
            body: folderPage([["s1", "one.zip", 100], ["s2", "two &amp; more.zip", 200]], true),
        }),
        "GET https://filester.me/f/Folder1?page=2": () => ({
            status: 200,
            body: folderPage([["s3", "three.zip", 300]], false),
        }),
    });
    const pkg = plugin.resolve("https://filester.me/f/Folder1");
    assertEqual(pkg.name, "Docs & Stuff");
    assertEqual(pkg.downloads.length, 3);
    assertEqual(pkg.downloads[0].url, "https://filester.me/d/s1");
    assertEqual(pkg.downloads[1].filename, "two & more.zip");
    assertEqual(pkg.downloads[2].filesize, 300);
    const re = new RegExp(plugin.urlPattern);
    assert(pkg.downloads.every((d: DownloadInfo) => re.test(d.url)), "file links are handled by this plugin (AC-06)");
    assertDeclared(seen);
});

test("an empty folder returns no downloads", () => {
    install({
        "GET https://filester.me/f/Empty?page=1": () => ({ status: 200, body: folderPage([], false) }),
    });
    assertEqual(plugin.resolve("https://filester.me/f/Empty").downloads.length, 0);
});
