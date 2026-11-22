#!/usr/bin/env node
/**
 * Regenerates the updater fixtures in this folder: two throwaway minisign
 * keys (never used for a release) and real Tauri-format signatures made with
 * them, for the smoke test of `scripts/updater-key-id-guard.mjs` and
 * `scripts/updater-latest-json.mjs` in ci.yml.
 *
 *   good/        the five desktop updater artifacts (stub contents), each
 *                with a `.sig` from the test key
 *   other-key/   krake-linux-x64.AppImage.sig made with a second key
 *   tauri.conf.json  minimal config whose updater pubkey is the test key
 *
 * Format (minisign, as tauri-cli writes it): a `.sig` file is the base64 of
 *   untrusted comment: ...\n<b64("ED" | key id | ed25519(blake2b512(file)))>\n
 *   trusted comment: ...\n<b64(ed25519(signature | trusted comment))>\n
 * and the pubkey is the base64 of
 *   untrusted comment: minisign public key: <ID>\n<b64("Ed" | key id | pk)>\n
 * The key ID is shown as the little-endian hex of its 8 bytes.
 *
 * Usage: node scripts/fixtures/updater/generate.mjs
 */

import crypto from "node:crypto";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));

export const ARTIFACTS = [
  "krake-windows-x64-setup.exe",
  "krake-windows-x64.msi",
  "krake-macos-x64.app.tar.gz",
  "krake-macos-arm64.app.tar.gz",
  "krake-linux-x64.AppImage",
];

const b64 = (buf) => Buffer.from(buf).toString("base64");
const showId = (id) => Buffer.from(id).reverse().toString("hex").toUpperCase();

function newKey() {
  const { publicKey, privateKey } = crypto.generateKeyPairSync("ed25519");
  const pk = publicKey.export({ type: "spki", format: "der" }).subarray(-32);
  const keyId = crypto.randomBytes(8);
  const pubText =
    `untrusted comment: minisign public key: ${showId(keyId)}\n` +
    `${b64(Buffer.concat([Buffer.from("Ed"), keyId, pk]))}\n`;
  return { privateKey, publicKey, keyId, id: showId(keyId), pubkey: b64(pubText) };
}

function sign(key, name, data) {
  const hash = crypto.createHash("blake2b512").update(data).digest();
  const sig = crypto.sign(null, hash, key.privateKey);
  const trusted = `timestamp:1767225600\tfile:${name}`;
  const global = crypto.sign(null, Buffer.concat([sig, Buffer.from(trusted)]), key.privateKey);
  // Self-check: the fixtures must be signatures minisign would accept.
  if (!crypto.verify(null, hash, key.publicKey, sig)) throw new Error("bad signature");
  const text =
    "untrusted comment: signature from tauri secret key\n" +
    `${b64(Buffer.concat([Buffer.from("ED"), key.keyId, sig]))}\n` +
    `trusted comment: ${trusted}\n${b64(global)}\n`;
  return b64(text);
}

const test = newKey();
const other = newKey();

for (const sub of ["good", "other-key"]) {
  rmSync(join(HERE, sub), { recursive: true, force: true });
  mkdirSync(join(HERE, sub));
}

for (const name of ARTIFACTS) {
  const data = Buffer.from(`krake updater fixture: ${name}\n`);
  writeFileSync(join(HERE, "good", name), data);
  writeFileSync(join(HERE, "good", `${name}.sig`), sign(test, name, data));
}
const appImage = "krake-linux-x64.AppImage";
writeFileSync(
  join(HERE, "other-key", `${appImage}.sig`),
  sign(other, appImage, Buffer.from(`krake updater fixture: ${appImage}\n`)),
);

writeFileSync(
  join(HERE, "tauri.conf.json"),
  `${JSON.stringify({ plugins: { updater: { pubkey: test.pubkey } } }, null, 2)}\n`,
);
writeFileSync(join(HERE, "KEY_IDS"), `test ${test.id}\nother ${other.id}\n`);
console.log(`test key ${test.id}, other key ${other.id}`);
