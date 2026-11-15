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
| AC-01 | `.github/workflows/site-data.yml` passes `theme-dir: packages/ui/src/theme` to the website's `validate-project` check (CI: site-data); locally `node scripts/theme.ts <krake>/docs/site/project.yml --out <krake>/packages/ui/src/theme --check` on website `main` | built | |
| AC-02 | `packages/ui/scripts/check-theme.mjs` (`checkAliases`: 32 properties in 2 palettes; `checkColourLiterals`: `tokens.css`, `app.css`), run by `pnpm --filter @amigo-labs/krake-ui check` | built | |
| AC-03 | `packages/ui/scripts/check-theme.mjs` (`checkNeon` with `effects.mjs`, self-test of the glow rule) and `scripts/ui-lint.mjs` (`no-neon-name`, `no-glow-shadow`) | built | |
| AC-04 | `web-ui/test/theme.test.ts`: "deletes the Neon intensity an earlier version stored", "Settings ▸ Appearance offers the three choices and no intensity setting", "has no string of the removed intensity setting" | built | |
| AC-05 | `web-ui/test/theme.test.ts`: "is dark with nothing stored, also on a light system", "falls back to dark for nothing stored and for an unknown value" | built | |
| AC-06 | `web-ui/test/theme.test.ts`: "keeps a choice an earlier version stored" | built | |
| AC-07 | `web-ui/test/theme.test.ts`: "applies and stores a pick at once, and starts in it after a reload" | built | |
| AC-08 | `web-ui/test/theme.test.ts`: "follows the system preference under System while the page is open", "follows the system under System", "ignores the system preference under Dark and Light" | built | |
| AC-09 | `web-ui/test/theme.test.ts`: "toggles to the palette opposite to the active one, also from System", "is the palette opposite to the active one" | built | |
| AC-10 | `web-ui/test/theme.test.ts`: "starts dark on an unknown stored value and keeps working", "starts dark and still applies a pick when storage throws or is missing", "works without a document, a media query or storage" | built | |
| AC-11 | `packages/ui/scripts/check-contrast.mjs` over `tokens.spec.json` (66 assertions, 2 palettes), run by `pnpm --filter @amigo-labs/krake-ui check` | built | |
| AC-12 | `web-ui/test/theme.test.ts`: "keeps theme-color on the active background", "index.html starts in the dark palette with the dark background as theme-color", "the PWA manifest carries the dark background" | built | |
| AC-13 | `web-ui/test/theme.test.ts`: "names the three choices in English and German", "has no string of the removed intensity setting" | built | |
| AC-14 | manual: the owner checked the rewritten `.impeccable.md` and the "Design System" section of `AGENTS.md` ("Krake und Sofarr sind ok"), 2026-11-15 | built |        |
| AC-15 | manual: the owner checked both palettes on the Downloads, History, Plugins and Settings pages, the Add panel, the detail panel and the command palette ("Krake und Sofarr sind ok"), 2026-11-15 | built |        |

## Decisions

Taken while building, without asking (spec-build):

- **Red runs** (2026-11-15, before the implementation):
  - `pnpm --filter @amigo-labs/krake-web-ui test` — `test/theme.test.ts` failed with "Cannot find module '../src/lib/theme'" (1 failed suite, 56 older tests passed).
  - `node packages/ui/scripts/check-theme.mjs` — exit 1, 259 problems: 51 alias mismatches (first: `dark: --bg-deep is "#1e1e1e", the token bg is "#15161a"`), the `:root.light` palette rule, 42 colour literals in `tokens.css`, 131 `no-neon-name` and 34 `no-glow-shadow` findings.
  - `node packages/ui/scripts/check-contrast.mjs` with the raised `tokens.spec.json` — exit 1, 6 of 60 assertions: `--text-muted #8f8f8f on --bg-surface-2 #2d2d2d = 4.26 (need 4.50)`, the same on `--hover-bg` (4.34), `--border-strong` undefined; each in both palettes.
  - `node scripts/ui-lint.mjs` with the new ban — exit 1, 165 problems (first: `packages/ui/src/components/Button.svelte:151 [no-neon-name]`).
- **AC-01 cannot go red locally**: the Site data job is CI-only; its first run on the pull request is the proof. Locally the generator's `--check` on a pristine copy of website `main` reports "up to date".
- **What counts as a glow shadow in the checks**: a `box-shadow`, `text-shadow` or `drop-shadow()` layer that has a blur and an accent or status colour (or `currentcolor`), or a shadow given as one custom property other than `--elev-0..3`. Unblurred shapes stay: the 3px selected-edge bar (`inset 3px 0 0 var(--accent)`), the 2px ring around the sidebar logo, the inset focus ring of `SegmentedControl` — they are edges, not shine, and the shared theme names the selected-edge bar as a role of `accent`.
- **Former neon names the lint bans**: any identifier containing `neon`, `glow`, `scanline` or `flicker`, and `applyIntensity`; `intensity` alone is too common a word. Comments are not scanned. One exception carries `ui-lint-disable-next-line no-neon-name`: the storage key `neon-intensity` in `web-ui/src/lib/theme.ts`, which AC-04 needs to delete.
- **The scan for colour literals covers Krake's stylesheets** (`packages/ui/src/tokens.css`, `web-ui/src/app.css`). Three literals in components stay as they were, because they are not theme colours: the black scrim behind `Dialog` and `DropZone` (`rgb(0 0 0 / 60%)`), the white paper and its dark error text under a captcha image (`CaptchaDialog`, already exempt from `no-raw-hex`), and the red badge drawn into the favicon canvas (`stores.ts`). A shared scrim token would have to be added in amigo-labs/website.
- **`--radius-xl` is `radius-lg`**: the shared scale ends at `lg` (12px). Tailwind's `rounded-xl` and the `Dialog` panel used 16px; they now take 12px rather than keeping a fourth step of Krake's own.
- **`--hover-bg` is the opaque `hover` token** (AC-02), no longer an 8 % accent tint; a hovered row is neutral and the accent marks only what is active or selected.
- **`tokens.spec.json`**: `--text-muted` is a 4.5 text pair on `--bg-deep`, `--bg-surface`, `--bg-surface-2` and `--hover-bg`. New: `--border-strong` (the theme's `border-strong`) at 3.0 on the three surfaces as the outline of a control; `Field` inputs and the `Toggle` rail now use it. The hairline `--border-color` guard went from 1.4 to 1.3: the shared `border` is 1.37:1 on `surface` in both palettes and is a decorative separator by the theme's definition (`theme/README.md`), so the old threshold could only be met by a colour of Krake's own.
- **Spacing, `--ease-out`, `--dur-fast` and `--dur-base` are aliases of the theme's tokens too** (same values as before); z-index, the font-size steps, `--ease-out-expo`, `--ease-spring` and `--dur-slow` stay Krake's.
- **The progress shimmer mixes `--accent-solid` with `--text-primary`** instead of the literal `white`, so the highlight is lighter in dark and darker in light.
- **Replacements for the glow**: `.neon-card` became `.outline-card` (hairline border, `elev-1` on hover); `Card` lost its `neon` prop (one caller, `PairingModal`) and an interactive card lifts to `elev-1` on hover; a grabbed download row takes `elev-2`; the search trigger and the feedback link card take `--border-strong` on hover; active chips that mixed `--neon-primary` into transparent take `--accent-subtle`; the delete button's border in Settings ▸ Webhooks mixes `--danger`. The status pulse animates size and opacity (`status-pulse`); the error dot no longer flickers.
- **`theme-color` is read from the stylesheet** (`getComputedStyle(root).getPropertyValue("--am-bg")`) after each change instead of repeating the two colours in TypeScript; if the stylesheet has no value yet, the meta keeps the dark `bg` of `index.html`. No inline script was added: the server's CSP is `script-src 'self'`, and the edge-case table accepts the dark ground before the app's script runs.
- **Settings ▸ Appearance uses the `SegmentedControl` primitive** (a radiogroup with arrow keys) for Dark / Light / System instead of three hand-styled buttons; inline styles went from 208 to 205 (`scripts/ui-budget.json`).
- **The header button's accessible name is translated** (`theme.switch_to_light` / `theme.switch_to_dark`, en + de); it was hard-coded English. The command palette keeps "Toggle light / dark", which does the same as the button.
- **New public API for review**: `web-ui/src/lib/theme.ts` exports `ThemeChoice`, `Palette`, `THEME_CHOICES`, `parseThemeChoice`, `resolvePalette`, `toggledChoice`, `ThemeEnv`, `ThemeStore`, `createThemeStore`, `browserThemeEnv` and the store `theme` (with `theme.palette`); `stores.ts` no longer exports `theme`, `ThemeMode`, `neonIntensity`, `applyIntensity`, `getNeonLabel`, `NeonLevel`. `packages/ui`: the custom property `--border-strong`, the class `.outline-card`; removed: `Card`'s `neon` prop, `.neon-card`, `.neon-top-line`, `.neon-text-*`, every `--neon-*` property.
