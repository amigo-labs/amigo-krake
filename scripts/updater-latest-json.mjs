#!/usr/bin/env node
/**
 * Writes the Tauri 2 updater manifest `latest.json` for a release
 * (docs/specs/desktop-app.md, AC-20). The desktop app checks
 * `releases/latest/download/latest.json` (plugins.updater.endpoints).
 *
 * Usage: node scripts/updater-latest-json.mjs <dir> --version <x.y.z> --tag <tag>
 *          [--repo <owner/name>] [--out <file>]
 *
 * <dir> holds the desktop assets under their stable names (AC-22) with their
 * `.sig` files next to them. Every platform links the asset of the release
 * tag, and its `signature` is the content of the `.sig`. A missing asset or a
 * missing or empty signature fails the script and writes nothing, so no
 * manifest ever carries an empty `signature`. Writes <dir>/latest.json unless
 * --out says otherwise.
 */

import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

/** Updater platform key → stable asset name. */
const PLATFORMS = {
  // Tauri 2 tries `<os>-<arch>-<installer>` first, then `<os>-<arch>`;
  // a plain Windows entry gets the NSIS setup, the default bundle.
  "windows-x86_64": "krake-windows-x64-setup.exe",
  "windows-x86_64-nsis": "krake-windows-x64-setup.exe",
  "windows-x86_64-msi": "krake-windows-x64.msi",
  "darwin-x86_64": "krake-macos-x64.app.tar.gz",
  "darwin-aarch64": "krake-macos-arm64.app.tar.gz",
  "linux-x86_64": "krake-linux-x64.AppImage",
  "linux-x86_64-appimage": "krake-linux-x64.AppImage",
};

function parseArgs(argv) {
  const opts = { repo: "amigo-labs/amigo-krake" };
  const rest = [];
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg.startsWith("--")) {
      opts[arg.slice(2)] = argv[++i];
    } else {
      rest.push(arg);
    }
  }
  opts.dir = rest[0];
  return opts;
}

function main(argv) {
  const opts = parseArgs(argv);
  if (!opts.dir || !opts.version || !opts.tag) {
    console.error("usage: updater-latest-json.mjs <dir> --version <x.y.z> --tag <tag> [--repo <owner/name>] [--out <file>]");
    return 2;
  }
  const version = opts.version.replace(/^v/, "");
  if (!/^\d+\.\d+\.\d+([-+][0-9A-Za-z.+-]+)?$/.test(version)) {
    console.error(`--version ${JSON.stringify(opts.version)} is not a semver version`);
    return 1;
  }

  const base = `https://github.com/${opts.repo}/releases/download/${opts.tag}`;
  const platforms = {};
  const errors = [];
  for (const [platform, asset] of Object.entries(PLATFORMS)) {
    const path = join(opts.dir, asset);
    if (!existsSync(path)) {
      errors.push(`${platform}: asset ${asset} is missing`);
      continue;
    }
    const sigPath = `${path}.sig`;
    const signature = existsSync(sigPath) ? readFileSync(sigPath, "utf8").trim() : "";
    if (!signature) {
      errors.push(`${platform}: signature ${asset}.sig is ${existsSync(sigPath) ? "empty" : "missing"}`);
      continue;
    }
    platforms[platform] = { signature, url: `${base}/${asset}` };
  }
  if (errors.length) {
    for (const e of errors) console.error(`::error::${e}`);
    console.error("latest.json not written");
    return 1;
  }

  const manifest = {
    version,
    notes: `Krake ${version}: see https://github.com/${opts.repo}/releases/tag/${opts.tag}`,
    pub_date: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
    platforms,
  };
  const out = opts.out ?? join(opts.dir, "latest.json");
  writeFileSync(out, `${JSON.stringify(manifest, null, 2)}\n`);
  console.log(`wrote ${out}`);
  console.log(JSON.stringify(manifest, null, 2));
  return 0;
}

process.exitCode = main(process.argv.slice(2));
