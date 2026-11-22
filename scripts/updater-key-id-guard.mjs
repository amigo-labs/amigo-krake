#!/usr/bin/env node
/**
 * Key-ID guard for the desktop updater signatures (docs/specs/desktop-app.md,
 * AC-21): every Tauri `.sig` in <dir> must carry the key ID of
 * `plugins.updater.pubkey` in tauri.conf.json. A release signed with another
 * key (say, a repository secret that shadows the organization key) would be
 * rejected by every installed copy, so the upload stops instead.
 *
 * Usage: node scripts/updater-key-id-guard.mjs <dir> [--conf <tauri.conf.json>]
 *
 * Environment:
 *   UPDATER_SIGNING_KEY_ID  overrides the expected key ID (16 hex chars), for
 *                           the bridge release of a key rotation. Empty or
 *                           unset: the ID of the configured pubkey.
 *
 * A Tauri `.sig` is any `.sig` next to an `.exe`, `.msi`, `.app.tar.gz` or
 * `.AppImage`, and any other `.sig` that is not the 128-hex-char Ed25519
 * signature of a CLI / server archive (scripts/sign-update-archives.sh).
 * Both the pubkey and a signature are base64 of a minisign text whose second
 * line is base64 of: 2 algorithm bytes, the 8-byte key ID, then the key or
 * signature. The ID is shown as the little-endian hex of those 8 bytes.
 */

import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const TAURI_SIG = /\.(exe|msi|app\.tar\.gz|AppImage)\.sig$/;
const ARCHIVE_SIG = /^[0-9a-f]{128}\s*$/;

/** Key ID of a Tauri pubkey or `.sig` (base64 of a minisign text). */
function minisignKeyId(tauriBase64, { signature }) {
  const text = Buffer.from(tauriBase64.trim(), "base64").toString("utf8");
  const lines = text.split(/\r?\n/);
  if (!lines[0]?.startsWith("untrusted comment:") || !lines[1]) {
    throw new Error("not a minisign text (no untrusted comment line)");
  }
  const bin = Buffer.from(lines[1].trim(), "base64");
  const [len, algs] = signature ? [74, ["ED", "Ed"]] : [42, ["Ed"]];
  const alg = bin.subarray(0, 2).toString("latin1");
  if (bin.length !== len || !algs.includes(alg)) {
    throw new Error(`not a minisign ${signature ? "signature" : "public key"} (${bin.length} bytes, algorithm ${JSON.stringify(alg)})`);
  }
  return Buffer.from(bin.subarray(2, 10)).reverse().toString("hex").toUpperCase();
}

function main(argv) {
  const args = [...argv];
  let conf = "tauri/tauri.conf.json";
  const i = args.indexOf("--conf");
  if (i !== -1) {
    conf = args[i + 1];
    args.splice(i, 2);
  }
  const dir = args[0];
  if (!dir || !conf) {
    console.error("usage: updater-key-id-guard.mjs <dir> [--conf <tauri.conf.json>]");
    return 2;
  }

  const pubkey = JSON.parse(readFileSync(conf, "utf8"))?.plugins?.updater?.pubkey;
  if (typeof pubkey !== "string" || !pubkey) {
    console.error(`${conf}: no plugins.updater.pubkey`);
    return 1;
  }
  let configured;
  try {
    configured = minisignKeyId(pubkey, { signature: false });
  } catch (e) {
    console.error(`${conf}: plugins.updater.pubkey: ${e.message}`);
    return 1;
  }

  const override = (process.env.UPDATER_SIGNING_KEY_ID ?? "").trim();
  if (override && !/^[0-9a-fA-F]{16}$/.test(override)) {
    console.error(`UPDATER_SIGNING_KEY_ID must be 16 hex chars, got ${JSON.stringify(override)}`);
    return 1;
  }
  const expected = override ? override.toUpperCase() : configured;
  const source = override
    ? `UPDATER_SIGNING_KEY_ID (pubkey in ${conf}: ${configured})`
    : `plugins.updater.pubkey in ${conf}`;

  const sigs = readdirSync(dir)
    .filter((name) => name.endsWith(".sig"))
    .filter((name) => TAURI_SIG.test(name) || !ARCHIVE_SIG.test(readFileSync(join(dir, name), "utf8")))
    .sort();
  if (sigs.length === 0) {
    console.error(`no Tauri .sig files in ${dir}`);
    return 1;
  }

  let failed = 0;
  for (const name of sigs) {
    let found;
    try {
      found = minisignKeyId(readFileSync(join(dir, name), "utf8"), { signature: true });
    } catch (e) {
      console.error(`::error::${name}: ${e.message}`);
      failed++;
      continue;
    }
    if (found !== expected) {
      console.error(`::error::${name} is signed with key ID ${found}, expected ${expected} (${source})`);
      failed++;
    } else {
      console.log(`${name}: key ID ${found}`);
    }
  }
  if (failed) {
    console.error(`${failed} of ${sigs.length} signatures do not carry key ID ${expected}`);
    return 1;
  }
  console.log(`all ${sigs.length} Tauri signatures carry key ID ${expected}`);
  return 0;
}

process.exitCode = main(process.argv.slice(2));
