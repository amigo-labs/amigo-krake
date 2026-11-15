---
status: todo
created: 2026-11-15
size: L
surfaces: [web-ui]
---

# Feature: the shared amigo theme in the web UI

## Context

Every amigo app gets the same look: one theme, kept in amigo-labs/website (`theme/`), and one accent per app, the `accent` of its `docs/site/project.yml`. Krake's web UI so far has its own design system, "Corporate Neon": the tokens of `packages/ui/src/tokens.css`, a glow whose strength the user sets in five steps, CRT scanlines and letter flicker at the highest step, and a dark and a light palette.

The owner decided (2026-11-15, design decision 16 in `AGENTS.md`, replacing the neon parts of the design system): Krake takes the shared theme and its neon effects go entirely. Surfaces, text, status colours, type, radii and shadows come from the shared tokens; the accent stays the brand lime `#9be22d` (decision 13), split into the shared theme's roles. Dark stays the default; Light and System can be chosen.

This spec changes values and removes the neon effects. Krake's primitives (`packages/ui/src/components/`) keep their own markup and shape here; moving them onto the shared component classes is a later spec (backlog).

Terms. **Theme files**: `tokens.css`, `components.css` and `theme.json`, written by amigo-labs/website `scripts/theme.ts` for Krake's `docs/site/project.yml`. **Token**: a named value of the theme files (`bg`, `surface`, `text`, `accent-ink`, …; their meaning is in the website's `theme/README.md`). **Theme choice**: the user's setting Dark, Light or System. **Active palette**: dark or light, what the theme choice resolves to. **Glow shadow**: a `box-shadow`, `text-shadow` or `drop-shadow` whose colour is an accent or status colour, drawn to make an element shine; the neutral elevation shadows `elev-1` to `elev-3` are not glow. **Neon effects**: glow shadows on cards, text, status dots and progress bars, the glowing top line of the sidebar, CRT scanlines, the flickering page title, and the "Neon intensity" setting that scales them. **Theme button**: the sun / moon button in the header that switches between dark and light. **Body-safe**: at least 4.5:1 on `bg`, `surface`, `surface-raised` and `hover`, which the website's theme test asserts for all three text tokens.

## Scope

### In scope

- The theme files in the repository, checked in CI against the generator.
- Every colour, radius, shadow and the font stack of the web UI and of the primitives from the tokens.
- Removal of the neon effects and of the Neon intensity setting.
- The theme choice Dark, Light, System in Settings ▸ Appearance; the theme button sets the choice (AC-09).
- The contrast check and the UI lint on the new values.
- The design documentation: `.impeccable.md`, the "Design System" section and a new decision in `AGENTS.md`.

### Out of scope

- Rebuilding the primitives on the shared component classes (`am-*`) — because that changes every primitive's markup and is a spec of its own; here they only change values.
- Renaming Krake's custom properties (`--bg-surface`, `--text-primary`, …) at their call sites — because it changes nothing a user sees; they stay as aliases of the tokens.
- The Tauri desktop window's title bar following the theme choice — because the desktop app has not shipped (`desktop-app`) and its window follows the operating system.
- Motion that is not neon: page and card enter animations, the progress shimmer, the pulse of an active status dot (its size and opacity; the glow shadow around the dot goes) — because they carry state, not decoration, and already stop under `prefers-reduced-motion`.

## Acceptance criteria

- [ ] AC-01 (always, CI: site-data): The theme files in the repository are exactly what the website's theme command writes for Krake's `docs/site/project.yml`; a pull request in which they differ fails the Site data check.
- [ ] AC-02 (always): In both palettes every colour, radius, shadow and font-stack property the web UI and the primitives use has the value of its token in the theme files: `--bg-deep` = `bg`, `--bg-surface` = `surface`, `--bg-surface-2` = `surface-raised`, `--hover-bg` = `hover`, `--text-primary` / `--text-secondary` / `--text-muted` = `text` / `text-secondary` / `text-muted`, `--border-color` = `border`, `--accent` / `--accent-ink` / `--accent-solid` / `--accent-subtle` / `--on-accent` and `--success` / `--warning` / `--danger` with their `-ink` forms = the token of the same name, `--status-online` / `--status-degraded` / `--status-error` = `success` / `warning` / `danger`, `--radius-xs` to `--radius-lg` and `--radius-full`, `--elev-1` to `--elev-3`, `--font-sans` and `--font-mono` = the token of the same name; Krake defines no colour of its own beside the theme files.
- [ ] AC-03 (always): No neon effect remains: the stylesheets and components of `web-ui/src` and `packages/ui/src` define and use no glow shadow, no scanline overlay, no flicker animation and no neon property or class, and the UI lint fails a change that adds one of the former neon names or a shadow in an accent or status colour.
- [ ] AC-04 (always): Settings ▸ Appearance has no Neon intensity setting, and a Neon intensity value an earlier version stored has no effect and is deleted at the next start where storage can be written.
- [ ] AC-05 (state): While no theme choice is stored, the active palette is dark, whatever the system prefers.
- [ ] AC-06 (state): While a theme choice an earlier version stored (`dark` or `light`) exists, the active palette is that one.
- [ ] AC-07 (event): When the user picks Dark, Light or System in Settings ▸ Appearance, then the active palette is the one the choice resolves to as soon as the pick is handled (no reload), the choice is stored, and after a reload the UI's first render is in that palette with the choice selected.
- [ ] AC-08 (state): While the theme choice is System, the active palette is light where the system prefers light and dark otherwise, and it follows a change of the system preference while the page is open.
- [ ] AC-09 (event): When the user presses the palette button in the header, then the theme choice becomes the palette opposite to the active one (Light while dark is shown, Dark while light is shown), also coming from System, and an open Settings page shows the new choice selected. The button is a two-state switch; System is chosen in Settings.
- [ ] AC-10 (error): If the stored theme choice is not one of `dark`, `light`, `system`, or storage cannot be read or written, then the active palette is dark at start, a pick still applies for the open page, and nothing throws.
- [ ] AC-11 (always): The contrast check of `packages/ui` passes for every pair of `tokens.spec.json` in both palettes, computed from the theme files' values, with text pairs at 4.5:1 or more (including `text-muted`, which is body-safe in the shared theme) and control outlines and fills that carry meaning at 3:1 or more.
- [ ] AC-12 (always): The page's `theme-color` and the PWA manifest's colours are the dark `bg` of the theme files, and while the active palette is light the page's `theme-color` is the light `bg`, also after the system preference changes under System; the manifest has one static colour and keeps the default palette's.
- [ ] AC-13 (option): Where the UI language is German, the choices read "Dunkel", "Hell" and "System"; the English UI reads "Dark", "Light" and "System"; the strings of the removed setting are gone from both languages.
- [ ] AC-14 (always, manual: documentation review): `.impeccable.md` and the "Design System" section of `AGENTS.md` describe the shared theme (no neon, one accent, Dark / Light / System), and `AGENTS.md` carries the decision.
- [ ] AC-15 (state, manual: visual design): While either palette is active, the Downloads, History, Plugins and Settings pages, the Add panel, the detail panel and the command palette look complete at 1280 px and 390 px width: no missing border where a glow used to separate two surfaces, no unreadable text, no leftover neon effect.

## Technical notes

- The theme files live in `packages/ui/src/theme/`; regenerate with `pnpm theme amigo-labs/amigo-krake --out ../amigo-krake/packages/ui/src/theme` in amigo-labs/website. `.github/workflows/site-data.yml` passes `theme-dir: packages/ui/src/theme` to the reusable check and triggers on that folder as well as on `docs/site/**`. A change of the generator in the website shows at the next such pull request; then the files are regenerated.
- Size L in one spec: the neon tokens and the colour tokens are the same lines of `tokens.css` and the same call sites, so splitting them would touch everything twice.
- `theme/tokens.css` is dark on `:root`, light under `data-theme="light"`, and follows the system when the root has no `data-theme`. The root carries `data-theme="dark"` until the choice says otherwise; System removes the attribute. The class `light` on the root, which today selects the light palette, goes.
- `packages/ui/src/tokens.css` keeps Krake's property names and the scales the shared theme does not have (z-index, the font-size steps the call sites use) and defines the rest as `var(--am-…)`. Cards that drew their edge with `--neon-border` take `border`; hover lifts take `elev-1`.
- The theme choice stays in `localStorage` under `theme` (values `dark`, `light`, new: `system`); `neon-intensity` is deleted there.
- Tests: Vitest unit tests in `web-ui/test/` for the theme choice (parse, resolve, toggle, storage failures); `packages/ui/scripts/check-contrast.mjs` for AC-11; `scripts/ui-lint.mjs` (a new hard ban) and a test that scans the sources for AC-03; for AC-02 a test that resolves each listed Krake property through `tokens.css` and the theme files for both palettes (the contrast script's resolver) and compares it with the token, plus a scan that Krake's stylesheets hold no colour literal. The system preference is a fake `matchMedia` in the unit tests. There is no browser suite.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A user who had Neon intensity at Off | Sees the same calm UI as everyone; nothing to migrate |
| A user on a light system with no stored choice, who so far got the light palette | Gets dark (the default); Light or System is one click in Settings or the header |
| The system preference changes while the choice is Dark or Light | Nothing changes |
| Two tabs open, the choice changes in one | The other tab keeps its palette until it is reloaded, because no other setting syncs between tabs either |
| The page before the app's script has run | The empty page shows the dark ground; the UI itself never renders in another palette than the stored one |
| An older build reads a stored `system` | It treats the value as unknown and uses its own default; there is no downgrade path to support |
| `prefers-reduced-motion` is set | As today: enter animations, shimmer and pulse are off |
| The server's CSP (`font-src 'self' data:`) | Inter stays self-hosted from `packages/ui`; no font or stylesheet from another host |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Dark is the default also for users on a light system | The owner chose "dark is the default, light can be chosen" for every app | A light-system user without a stored choice sees a changed palette once |
| The order of the choices is Dark, Light, System | Dark is the default and comes first; same as Pincel | Low: wording only |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 |      | open   |        |
| AC-02 |      | open   |        |
| AC-03 |      | open   |        |
| AC-04 |      | open   |        |
| AC-05 |      | open   |        |
| AC-06 |      | open   |        |
| AC-07 |      | open   |        |
| AC-08 |      | open   |        |
| AC-09 |      | open   |        |
| AC-10 |      | open   |        |
| AC-11 |      | open   |        |
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |
| AC-14 |      | open   |        |
| AC-15 |      | open   |        |

## Decisions
