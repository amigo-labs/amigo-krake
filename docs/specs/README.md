# Specs

Feature specs (front matter `status`, `## Acceptance criteria`) and design
notes for Krake. The workflow is in `AGENTS.md`, "Spec-driven workflow".

## Order

| # | Spec | Status | Depends on |
| - | ---- | ------ | ---------- |
| 1 | [plugin-wasm-sandbox](plugin-wasm-sandbox.md) | todo | plugin-wasm-guest (done) |
| 2 | [desktop-app](desktop-app.md) | todo | — (three PRs: P1 embedded server, P2 tray, P3 updater, release and update choice) |
| 3 | [plugin-registry](plugin-registry.md) | todo | — (org setup: Cloudflare Pages project on branch `registry`, DNS) |
| 4 | [local-auth-bypass](local-auth-bypass.md) | todo | — |

## Done

Specs from before this workflow carry a bold `**Status**` header instead of
front matter; they are converted when next revised.

- [amigo-theme](amigo-theme.md)
- [docker-security](docker-security.md)
- [integration-tests](integration-tests.md)
- [link-list-import](link-list-import.md)
- [plugin-wasm-guest](plugin-wasm-guest.md)
- [rust-1-99-toolchain](rust-1-99-toolchain.md)
- [self-update](self-update.md)
- [tauri-workspace-integration](tauri-workspace-integration.md)

## Design notes and reviews

- [plugin-sandbox-hardening](plugin-sandbox-hardening.md) — SH-1 to SH-7a
  implemented; SH-7b replaced by plugin-wasm-sandbox
- [plugin-runtime-wasm-portability](plugin-runtime-wasm-portability.md)
- [architecture-review](architecture-review.md)
- [audit-2026-04-25](audit-2026-04-25.md)
- [ui-review-2026-07](ui-review-2026-07.md)

## Backlog

Gaps not yet written as specs.

| Gap | Note |
| --- | ---- |
| Primitives of `packages/ui` on the shared component classes (`am-*`) | After amigo-theme: buttons, fields, toggles, chips, dialogs take the markup and classes of the shared theme's `components.css`. |
| YouTube extractor runs player JavaScript in a native QuickJS inside the daemon | `crates/extractors/src/youtube/n_challenge.rs`. The script comes from YouTube, not from a reviewed plugin. Candidate: run it through the Wasm executor once plugin-wasm-sandbox is done. |
