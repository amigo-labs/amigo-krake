/// <reference path="../../types/krake.d.ts" />

// Offline tests: krake.http* and storage are replaced with fakes that serve
// recorded gofile.io responses. Nothing here reaches the real host.

const plugin = module.exports;

interface FakeRequest { method: string; url: string; headers: Record<string, string>; body?: string }

function install(routes: Record<string, (req: FakeRequest) => { status: number; body: string }>) {
    const seen: FakeRequest[] = [];
    const store: Record<string, string> = {};
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
    krake.storageGet = (k: string) => (k in store ? store[k] : null);
    krake.storageSet = (k: string, v: string) => { store[k] = v; };
    return { seen, store };
}

function hostOf(url: string): string {
    return krake.regexMatch("^https?://([^/:?#]+)", url) || "";
}

const ACCOUNT = { status: "ok", data: { token: "guest-token-1" } };
const FOLDER = {
    status: "ok",
    data: {
        type: "folder",
        name: "Holiday",
        children: {
            "b": { type: "file", name: "b.jpg", size: 2048, link: "https://store7.gofile.io/download/web/b/b.jpg" },
            "a": { type: "file", name: "a.mp4", size: 1048576, link: "https://store7.gofile.io/download/web/a/a.mp4" },
            "s": { type: "folder", name: "Sub", code: "SubCode" },
        },
    },
};

test("has required metadata", () => {
    assertEqual(plugin.id, "gofile");
    assertNotNull(plugin.version);
    assertEqual(typeof plugin.resolve, "function");
});

test("AC-16: urlPattern matches folder links only", () => {
    const re = new RegExp(plugin.urlPattern);
    assert(re.test("https://gofile.io/d/AbC123"), "folder link");
    assert(re.test("https://www.gofile.io/d/AbC123"), "www folder link");
    assert(!re.test("https://store7.gofile.io/download/web/a/a.mp4"), "file server link");
    assert(!re.test("https://gofile.io/"), "home page");
    assert(!re.test("https://example.com/d/AbC123"), "other host");
});

test("AC-10: a folder resolves to its files and subfolder links", () => {
    const fake = install({
        "POST https://api.gofile.io/accounts": () => ({ status: 200, body: JSON.stringify(ACCOUNT) }),
        "GET https://api.gofile.io/contents/AbC123": (req) => {
            assertEqual(req.headers["Authorization"], "Bearer guest-token-1");
            assertEqual((req.headers["X-Website-Token"] || "").length, 64, "website token is a SHA-256");
            return { status: 200, body: JSON.stringify(FOLDER) };
        },
    });
    const pkg = plugin.resolve("https://gofile.io/d/AbC123");
    assertEqual(pkg.name, "Holiday");
    assertEqual(pkg.downloads.length, 3);
    assertEqual(pkg.downloads[0].filename, "a.mp4");
    assertEqual(pkg.downloads[0].filesize, 1048576);
    assertEqual(pkg.downloads[0].url, "https://store7.gofile.io/download/web/a/a.mp4");
    assertEqual(pkg.downloads[0].cookies!["accountToken"], "guest-token-1");
    assertEqual(pkg.downloads[1].filename, "b.jpg");
    assertEqual(pkg.downloads[2].url, "https://gofile.io/d/SubCode");
    // The subfolder link is one the plugin handles itself (AC-06 resolves it).
    assert(new RegExp(plugin.urlPattern).test(pkg.downloads[2].url), "subfolder is a folder link");
    assertEqual(fake.store["account_token"], "guest-token-1");
    for (const r of fake.seen) {
        assert(plugin.permissions.domains.indexOf(hostOf(r.url)) >= 0, "AC-16: undeclared host " + r.url);
    }
});

test("AC-10: the stored guest account is reused", () => {
    const fake = install({
        "GET https://api.gofile.io/contents/AbC123": () => ({ status: 200, body: JSON.stringify(FOLDER) }),
    });
    krake.storageSet("account_token", "stored-token");
    const pkg = plugin.resolve("https://gofile.io/d/AbC123");
    assertEqual(pkg.downloads[0].cookies!["accountToken"], "stored-token");
    assertEqual(fake.seen.length, 1);
});

test("AC-10: an expired guest account is replaced once", () => {
    let calls = 0;
    install({
        "POST https://api.gofile.io/accounts": () => ({ status: 200, body: JSON.stringify(ACCOUNT) }),
        "GET https://api.gofile.io/contents/AbC123": () => {
            calls++;
            return calls === 1
                ? { status: 401, body: JSON.stringify({ status: "error-token" }) }
                : { status: 200, body: JSON.stringify(FOLDER) };
        },
    });
    krake.storageSet("account_token", "expired");
    const pkg = plugin.resolve("https://gofile.io/d/AbC123");
    assertEqual(pkg.downloads[0].cookies!["accountToken"], "guest-token-1");
});

function resolveError(body: object): string {
    install({
        "POST https://api.gofile.io/accounts": () => ({ status: 200, body: JSON.stringify(ACCOUNT) }),
        "GET https://api.gofile.io/contents/AbC123": () => ({ status: 200, body: JSON.stringify(body) }),
    });
    try {
        plugin.resolve("https://gofile.io/d/AbC123");
    } catch (e) {
        return String(e);
    }
    throw new Error("resolve did not fail");
}

test("AC-11: a missing folder says not found", () => {
    assert(resolveError({ status: "error-notFound" }).indexOf("not found") >= 0, "not found");
});

test("AC-11: a protected folder says password protected", () => {
    assert(resolveError({ status: "error-passwordRequired" }).indexOf("password protected") >= 0, "password");
});

test("an empty folder returns no downloads", () => {
    install({
        "POST https://api.gofile.io/accounts": () => ({ status: 200, body: JSON.stringify(ACCOUNT) }),
        "GET https://api.gofile.io/contents/Empty1": () => ({
            status: 200,
            body: JSON.stringify({ status: "ok", data: { type: "folder", name: "E", children: {} } }),
        }),
    });
    assertEqual(plugin.resolve("https://gofile.io/d/Empty1").downloads.length, 0);
});
