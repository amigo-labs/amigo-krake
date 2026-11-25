#!/usr/bin/env node
// Local live test: resolves real hoster links through the plugins with
// `krake plugins test <plugin.ts> <url>` and prints one line per link.
// Never part of CI — it reaches real hosters (see plugins/README.md).
//
// Usage:
//   node scripts/plugin-live-test.mjs [links-file] [--head] [--only <plugin>]
//
//   links-file  default plugins/live-links.txt (git-ignored); lines are
//               "<plugin> <url>", "#" starts a comment, e.g.
//               gofile https://gofile.io/d/AbC123
//   --head      also send a HEAD request to every resolved download URL
//   --only      test only the links of one plugin
//
// The krake binary is KRAKE_BIN or target/debug/krake; krake-plugin-host must
// sit next to it (or KRAKE_PLUGIN_HOST_BIN). Build both with
//   cargo build -p krake-cli -p krake-plugin-runtime --bin krake --bin krake-plugin-host

import { spawn } from "node:child_process";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const args = process.argv.slice(2);
const head = args.includes("--head");
const onlyAt = args.indexOf("--only");
const only = onlyAt >= 0 ? args[onlyAt + 1] : null;
const positional = args.filter((a, i) => !a.startsWith("--") && (onlyAt < 0 || i !== onlyAt + 1));
const linksFile = resolve(positional[0] ?? join(root, "plugins/live-links.txt"));
const bin = process.env.KRAKE_BIN ?? join(root, "target/debug/krake");
const TIMEOUT_MS = 90_000;

const BUILD_HINT = "cargo build -p krake-cli -p krake-plugin-runtime --bin krake --bin krake-plugin-host";
if (!existsSync(bin)) {
    fail(`krake binary not found at ${bin}: run \`${BUILD_HINT}\` or set KRAKE_BIN`);
}
if (!process.env.KRAKE_PLUGIN_HOST_BIN && !existsSync(join(bin, "..", "krake-plugin-host"))) {
    fail(`krake-plugin-host not found next to ${bin}: run \`${BUILD_HINT}\` or set KRAKE_PLUGIN_HOST_BIN`);
}
if (!existsSync(linksFile)) {
    fail(`links file not found: ${linksFile}\ncopy plugins/live-links.example.txt to plugins/live-links.txt and add links`);
}

// plugin id (folder name) -> path of its plugin.ts
const plugins = new Map();
for (const category of readdirSync(join(root, "plugins"), { withFileTypes: true })) {
    if (!category.isDirectory()) continue;
    for (const dir of readdirSync(join(root, "plugins", category.name), { withFileTypes: true })) {
        const file = join(root, "plugins", category.name, dir.name, "plugin.ts");
        if (dir.isDirectory() && existsSync(file)) plugins.set(dir.name, file);
    }
}

const links = readFileSync(linksFile, "utf8")
    .split("\n")
    .map((l) => l.replace(/#.*/, "").trim())
    .filter(Boolean)
    .map((l) => l.split(/\s+/))
    .filter(([name]) => !only || name === only);

if (links.length === 0) fail("no links to test");

let failed = 0;
let skipped = 0;
for (const [name, url] of links) {
    const file = plugins.get(name);
    if (!file || !url) {
        skipped++;
        report("SKIP", name, url ?? "", file ? "missing URL" : `unknown plugin (known: ${[...plugins.keys()].join(", ")})`);
        continue;
    }
    const { out, timedOut } = await run(bin, ["plugins", "test", file, url]);
    const result = classify(out, timedOut);
    if (result.status !== "OK") failed++;
    report(result.status, name, url, result.detail);
    if (result.status === "OK" && head) {
        for (const dl of result.pkg.downloads) {
            const status = await probe(dl);
            if (!/^2\d\d/.test(status)) failed++;
            console.log(`        HEAD ${status}  ${dl.url}`);
        }
    }
}

console.log(`\n${links.length - failed - skipped} ok, ${failed} failed, ${skipped} skipped`);
process.exit(failed || skipped ? 1 : 0);

function classify(out, timedOut) {
    if (timedOut) return { status: "FAIL", detail: `timeout after ${TIMEOUT_MS / 1000}s` };
    if (out.includes("URL does not match")) return { status: "FAIL", detail: "URL does not match urlPattern" };
    const error = out.match(/\nError: (.*)/s);
    if (error) return { status: "FAIL", detail: error[1].trim().split("\n")[0] };
    const json = out.slice(out.indexOf("{", out.indexOf("Resolving:")));
    let pkg;
    try {
        pkg = JSON.parse(json);
    } catch {
        return { status: "FAIL", detail: `unexpected output: ${out.trim().split("\n").pop()}` };
    }
    const downloads = pkg.downloads ?? [];
    if (downloads.length === 0) return { status: "FAIL", detail: `package "${pkg.name}" has no downloads` };
    const first = downloads[0];
    const size = first.filesize == null ? "size ?" : formatBytes(first.filesize);
    return {
        status: "OK",
        pkg,
        detail: `"${pkg.name}": ${downloads.length} download(s), first ${first.filename ?? "(no name)"} (${size})`,
    };
}

async function probe(dl) {
    const headers = { ...(dl.headers ?? {}) };
    if (dl.cookies) {
        headers.Cookie = Object.entries(dl.cookies).map(([k, v]) => `${k}=${v}`).join("; ");
    }
    try {
        const res = await fetch(dl.url, { method: "HEAD", headers, redirect: "follow", signal: AbortSignal.timeout(30_000) });
        const len = res.headers.get("content-length");
        return `${res.status}${len ? ` ${formatBytes(Number(len))}` : ""}`;
    } catch (e) {
        return `error ${e.cause?.code ?? e.message}`;
    }
}

function run(cmd, argv) {
    return new Promise((done) => {
        const child = spawn(cmd, argv, { stdio: ["ignore", "pipe", "pipe"] });
        let out = "";
        let timedOut = false;
        child.stdout.on("data", (d) => (out += d));
        child.stderr.on("data", (d) => (out += d));
        const timer = setTimeout(() => {
            timedOut = true;
            child.kill("SIGKILL");
        }, TIMEOUT_MS);
        child.on("close", () => {
            clearTimeout(timer);
            done({ out, timedOut });
        });
    });
}

function report(status, name, url, detail) {
    console.log(`${status.padEnd(4)}  ${name.padEnd(14)} ${url}\n        ${detail}`);
}

function formatBytes(n) {
    const units = ["B", "KB", "MB", "GB", "TB"];
    let i = 0;
    while (n >= 1024 && i < units.length - 1) {
        n /= 1024;
        i++;
    }
    return `${n.toFixed(i ? 1 : 0)} ${units[i]}`;
}

function fail(msg) {
    console.error(msg);
    process.exit(2);
}
