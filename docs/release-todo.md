# Release-TODO — manuelle Schritte zu v1.0

Diese Datei sammelt ausschließlich die **manuell durchzuführenden** Schritte
auf dem Weg zu einem v1.0-Release — Dinge, die ein Maintainer mit Zugang zu
Secrets, bezahlten Accounts oder lokalen Signing-Keys erledigen muss. Reine
Code-Aufgaben (Marketplace-UI, Hoster-Plugins, Rate-Limit, OpenAPI-Codegen,
Web-UI-Tests, CHANGELOG-Backfill, Rustdoc) gehören **nicht** hierher und
laufen über eigene PRs.

Stand: v0.1.0 ist veröffentlicht (Tag `krake-v0.1.0`). Seitdem taggt
release-please wie Pincel und Blinzel ohne Komponente (`v0.1.1`, Titel
„Krake 0.1.1“); `bootstrap-sha` in `release-please-config.json` zeigt auf den
0.1.0-Commit, bis das erste `v…`-Release existiert. Auto-Release-on-merge ist
aktiv, und 4/4 CRITICAL + 11/17 HIGH aus `docs/specs/audit-2026-04-25.md`
sind gemergt.

## Signing & Secrets

- [ ] Tauri-Signing-Key erzeugen (`tauri signer generate`, lokal/offline).
- [ ] `TAURI_SIGNING_PRIVATE_KEY` und `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
      als GitHub-Repo-Secrets hinterlegen (werden in
      `.github/workflows/release-build.yml` bereits gelesen).
- [ ] Pubkey in `tauri/tauri.conf.json` gegen den neuen Key abgleichen.
      Danach kann der Code-Schritt `"active": true` (Updater) gesetzt werden.
- [ ] Registry-Ed25519-**Private**-Key erzeugen und offline sichern;
      sicherstellen, dass `KRAKE_REGISTRY_PUBKEY_HEX` (Build-Arg in
      `.github/workflows/release-build.yml` bzw. Docker-Build) zum
      Public-Key passt.
- [ ] Signier-Prozess für den Plugin-Registry-Index etablieren und
      dokumentieren (wer signiert beim Veröffentlichen neuer Plugins?).
- [ ] Self-Update-Ed25519-**Private**-Key erzeugen und offline sichern;
      sicherstellen, dass `KRAKE_UPDATE_PUBKEY_HEX` (Build-Arg in
      `.github/workflows/release-build.yml` bzw. Docker-Build) zum
      Public-Key passt. `crates/core/build.rs` verweigert Release-Builds
      ohne diesen Key.
- [ ] Release-Pipeline: jedes `krake-server`-Binary mit dem Private-Key
      signieren und die Detached-Signatur als `<asset>.sig`-Release-Asset
      neben dem Binary und `<asset>.sha256` hochladen (der Self-Updater
      erwartet sie und lehnt sonst das Update ab).

### Schlüssel für den Release-Build setzen (Schritt für Schritt)

Stand 2026-11-15: der Release-Build für 0.1.1 (Lauf 43749588193) scheitert in
allen CLI-Jobs an „Require pinned signing keys“, weil die Repository-Variable
`AMIGO_REGISTRY_PUBKEY_HEX` leer ist. Der Workflow liest die Werte unter den
`AMIGO_*`-Namen und reicht sie als `KRAKE_*` an den Build weiter.

Gibt es aus dem 0.1.0-Release schon Schlüsselpaare, dieselben wieder
verwenden: jede ausgelieferte Version vertraut nur dem Schlüssel, der in sie
eingebaut wurde.

1. Ein Ed25519-Schlüsselpaar lokal und offline erzeugen. Es signiert die
   Update-Archive und den Plugin-Registry-Index (Entscheidung 17,
   `docs/specs/plugin-registry.md`):

   ```sh
   openssl genpkey -algorithm ed25519 -out update-key.pem
   # privater Seed (64 Hex-Zeichen):
   openssl pkey -in update-key.pem -outform DER | tail -c 32 | od -An -tx1 -v | tr -d ' \n'
   # öffentlicher Schlüssel (64 Hex-Zeichen):
   openssl pkey -in update-key.pem -pubout -outform DER | tail -c 32 | od -An -tx1 -v | tr -d ' \n'
   ```

2. In GitHub unter Settings → Secrets and variables → Actions eintragen:

   | Art | Name | Wert |
   | --- | ---- | ---- |
   | Variable | `AMIGO_UPDATE_PUBKEY_HEX` | öffentlicher Schlüssel aus `update-key.pem` |
   | Secret | `AMIGO_UPDATE_SIGNING_KEY` | privater Seed aus `update-key.pem` |

   Der Release-Build pinnt `AMIGO_UPDATE_PUBKEY_HEX` auch als
   Registry-Schlüssel; `AMIGO_REGISTRY_PUBKEY_HEX` wird nicht mehr gelesen.
   Der Workflow `registry-publish.yml` signiert den Registry-Index mit
   `AMIGO_UPDATE_SIGNING_KEY`.

3. Die `.pem`-Datei offline sichern und nie committen. Geht ein privater
   Schlüssel verloren, nehmen bestehende Installationen keine Updates mehr an.

4. Den Release-Build neu starten: den fehlgeschlagenen Lauf wiederholen oder
   `release-build.yml` von Hand auslösen (`tag_name`, `version`).

Die Tauri-Installer brauchen zusätzlich `TAURI_SIGNING_PRIVATE_KEY` und
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (oben).

## Accounts & Test-Fixtures (für Verifikation nötig)

- [ ] Bezahlte Test-Accounts für Premiumize und AllDebrid besorgen, um die
      Plugins unter `plugins/multi-hosters/*` end-to-end zu verifizieren.
- [ ] Live-YouTube-Fixtures aufnehmen (Player-JS + `signatureCipher`-Sample)
      für den Fallback-Test zu Audit-Finding #15.

## Veröffentlichung & Distribution

- [ ] Entscheiden, ob `@amigo-labs/krake-plugin-sdk` öffentlich auf npm publiziert wird.
      Falls ja: npm-Org/Account + `NPM_TOKEN`-Secret anlegen und
      `"private": true` in `plugin-sdk/package.json` entfernen.
- [x] Direkt nach dem v0.1.0-Tag `release-as: 0.1.0` aus
      `release-please-config.json` entfernen — sonst versucht jeder weitere
      Lauf erneut 0.1.0 zu releasen.
- [ ] release-please-Release-PR für v1.0 reviewen und mergen (Tag-Cut).
- [ ] Erstes signiertes Desktop-Bundle manuell auf je einer Plattform
      (Linux / macOS / Windows) installieren und das Auto-Update von einer
      heruntergestuften Version aus verifizieren.

## Produkt-Entscheidungen (menschliches Urteil)

- [ ] `docs/specs/docker-security.md`: umsetzen oder begründet für v1.0
      zurückstellen.
- [ ] `docs/specs/tauri-workspace-integration.md`: Restscope festlegen,
      nachdem der Tauri-Updater scharfgeschaltet ist.
