---
status: todo
created: 2026-11-22
size: M
design: [AGENTS.md "Plugin-System" (Registry & Updates, Sandboxing), AGENTS.md "REST API" (Plugins), docs/plugin-api.md]
surfaces: [rest, web-ui]
---

# Plugin suggestion when adding links

## Context

When links are pasted into the add panel, Krake queues them with whatever
plugins are installed. A link to a host that a registry plugin knows (bunkr,
gofile, filester, …) silently falls back to `generic-http`, which downloads
the host's HTML page instead of the file for album and landing-page links,
and the user only learns about the better plugin by browsing the Plugins
page.

The building blocks exist: `POST /api/v1/plugins/suggest` matches one URL
against the configured, verified registry (plugin-registry AC-17), and
`POST /api/v1/updates/plugins/{id}/install` installs a registry plugin, with
`approve_permissions` confirming its `permissions.domains`. But the suggest
endpoint takes one URL, returns the first registry entry whose pattern
matches — for an unknown host that is `generic-http`, for 1fichier a premium
multi-hoster — and ignores what is already installed. The add panel calls
neither endpoint.

## Scope

### In scope

- `POST /api/v1/plugins/suggest` additionally accepts a list of links and
  answers with the registry plugins that would handle them better than the
  installed ones.
- The web UI's add panel asks for suggestions while links are in its URL
  field and offers to install (or enable) the suggested plugins in place.

### Out of scope

- Suggesting multi-hosters (AllDebrid, Premiumize, Real-Debrid) — because
  they only work with a paid account; suggesting them for every link of a
  supported host would be noise (the user's choice).
- Suggesting a plugin for a link an enabled installed multi-hoster already
  matches — because a user who installed and enabled a multi-hoster chose it
  for those hosts.
- Suggesting `generic-http` or other generic plugins — because they match
  every link and are the fallback the suggestion is meant to improve on.
- Suggestions for links that arrive without the add panel (Click'n'Load,
  deep links, CLI, RSS, `.txt` dropped on the Downloads page) — because there
  is no place to show a choice there; they can follow in a later spec.
- Installing automatically without a click — because installing a plugin
  grants it network access to its domains, which the user confirms.
- Re-resolving downloads that were already queued before the plugin was
  installed — because the user decides whether to add the links before or
  after installing; queued downloads keep their resolver.

## Acceptance criteria

- [ ] AC-01 (event): When `POST /api/v1/plugins/suggest` is called with
  `{"urls": [...]}`, then the server answers `200` with
  `{"suggestions": [...]}`: one entry per suggested plugin with its `id`,
  `name`, `version`, `domains` (the plugin's `permissions.domains`: `null`
  when it is unscoped and may reach any public host, `[]` when it may reach
  none), `urls` (the given links it covers, a link given several times as the
  same string listed once, in input order) and `action` (`"install"` or
  `"enable"`). Entries are ordered by the position of their first covered
  link in the input.
- [ ] AC-02 (always): The server suggests only plugins whose plugin type is
  `hoster` (in the registry: tagged `hoster`); a `multi-hoster` or `generic`
  plugin is never suggested, even when it is the only one whose pattern
  matches.
- [ ] AC-03 (always): The server suggests nothing for a link that an
  installed, enabled `hoster` or `multi-hoster` plugin already matches; a
  link that only an installed generic plugin (or no plugin) matches can get a
  suggestion.
- [ ] AC-04 (event): When a link matches an installed but disabled `hoster`
  plugin and no enabled `hoster` or `multi-hoster` plugin, then the server
  suggests that installed plugin with `action: "enable"` (and its installed
  `version` and `domains`), and no registry plugin for that link.
- [ ] AC-05 (always): The server only suggests a registry plugin for
  `action: "install"` that the server's install endpoint would accept for its
  host API and app version; an incompatible plugin is left out.
- [ ] AC-06 (always): A link appears in the `urls` of at most one
  suggestion. Among several candidates (after AC-02 to AC-05) the installed
  disabled plugin wins (among several, the one first in the server's plugin
  list); among registry plugins, the one listed first in the registry index.
- [ ] AC-07 (error): If the registry is unreachable or fails signature
  verification, then the answer is `200` and holds no `action: "install"`
  suggestion; `action: "enable"` suggestions, which need no registry, are
  still given.
- [ ] AC-08 (error): If `urls` holds more than 1 000 entries (counted as
  sent, duplicates and non-links included), is not a list of strings
  (`null` included), or the body has both `url` and `urls` or neither, then
  the server answers `400` or `422` and no suggestions. Entries that are not `http`/`https` URLs are ignored,
  because plugin URL patterns only match web links; an empty list answers
  `200` with no suggestions.
- [ ] AC-09 (always): `POST /api/v1/plugins/suggest` with `{"url": "..."}`
  answers exactly as before this spec (plugin-registry AC-17).
- [ ] AC-10 (event): When the add panel's URL field holds at least one link
  (links as the panel already counts them for its "n links" note) and the
  field has not changed for 500 ms, then within one second the panel shows
  one card per suggestion for those links with the plugin name, the number of
  links it covers, all its domains ("any public host" when unscoped, "no
  hosts" for an empty list; more than three are shown behind a "show all"
  toggle) and an Install or Enable button.
- [ ] AC-11 (event): When the user clicks Install on a card, then the plugin
  is installed with its domains approved, a success toast names the plugin
  and the panel asks again for the current links, so the card disappears and
  any other card whose links the new plugin now covers goes too. When the
  user clicks Enable, the installed plugin is enabled (no domain approval:
  its domains were approved at its install), with the same toast and refresh.
- [ ] AC-12 (error): If installing or enabling fails, then an error toast
  with a translated title shows the server's (English) message, the card stays with its button usable again,
  and the URL field keeps its links.
- [ ] AC-13 (state): While a lookup runs, and after it failed, the Add
  button queues the links as without this feature, and a failed lookup shows
  no error.
- [ ] AC-14 (state): While the URL field changes, the panel only shows the
  answer to its latest lookup: an answer to an earlier lookup that arrives
  later is dropped, and a field without links shows no cards.
- [ ] AC-15 (always): Every new text of this feature — the card line with
  the link count, the domain line with "any public host" and "no hosts",
  "show all", Install, Enable, the success toasts and the error toast titles — exists in English and
  German.

## Technical notes

Request and answer of the list form:

```json
POST /api/v1/plugins/suggest
{"urls": ["https://bunkr.cr/a/x", "https://gofile.io/d/y", "https://example.com/f"]}

200
{"suggestions": [
  {"id": "bunkr", "name": "Bunkr", "version": "1.0.0",
   "domains": ["bunkr.cr", "..."], "urls": ["https://bunkr.cr/a/x"], "action": "install"},
  {"id": "gofile", "name": "Gofile", "version": "1.0.0",
   "domains": ["api.gofile.io"], "urls": ["https://gofile.io/d/y"], "action": "install"}
]}
```

Card sketch (above the Add button, inside the URL tab):

```
┌──────────────────────────────────────────────┐
│ Bunkr handles 3 of these links                │
│ May connect to: bunkr.ac, bunkr.ci, bunkr.cr  │
│ [show all 47]                                 │
│                                   [Install]   │
└──────────────────────────────────────────────┘
```

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Links for two hosts with registry plugins | Two cards, ordered by the first link of each |
| The same link pasted twice | Counted once on its card |
| Plugin already installed and enabled | No card for its links |
| Only `generic-http` matches (unknown host) | No card |
| 1fichier link, only a multi-hoster matches in the registry | No card (AC-02) |
| Registry offline | No card, no error; Add works (AC-07, AC-13) |
| User edits the field while a lookup runs | The older answer is dropped (AC-14) |
| User installs, then the field is cleared | Nothing more to show; the plugin stays installed |
| Plugin is unscoped (`domains: null`) | Card says "any public host" |
| 5 000 links pasted | Server answers 400; panel shows no card and no error (AC-08, AC-13) |
| The panel is closed while an install runs | The install completes on the server; its toast still appears |
| Another client installs the plugin before the click | The install succeeds again (a reinstall of the same id) and the refresh drops the card (AC-11) |
| The refresh after an install fails | The installed plugin's card is removed; other cards stay until the next lookup |
| A link matches a disabled installed hoster and another registry hoster | One Enable card for the installed plugin (AC-06) |
| User clicks Add before installing | Links are queued with the installed plugins (AC-13) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| 500 ms quiet time before the lookup | Avoids a request per keystroke while typing a link by hand; pasting is a single change, so the card appears half a second later | Felt as slow; the number is in one place |
| Clicking Install on a card counts as approving the shown domains | The card shows the same domain list the Plugins page asks to confirm; a second confirmation step would repeat it | A user may not read the list; the same holds for the Plugins page today |
| Limit 1 000 links per request | Covers a large pasted list while bounding regex work per request | A bigger paste gets no suggestion |
| The registry entry does not change between the card and the click | The registry is signed and published from `main` a few times a week; the window is seconds | A new version with wider domains is installed with them approved, as on the Plugins page today |
| "Installed" means a plugin in the server's loaded plugin list | That is the list the resolver matches links against | A plugin file that failed to load counts as not installed and can be suggested again |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ac01_suggestions_group_links_per_plugin_in_input_order`, `ac01_domains_keep_unscoped_and_empty_apart` (crates/plugin-runtime/src/registry.rs); `suggest_ac01_ac02_a_list_of_links_gets_hoster_suggestions` (crates/server/tests/api_tests.rs) | built |  |
| AC-02 | `ac02_only_hoster_plugins_are_suggested` (crates/plugin-runtime/src/registry.rs); `suggest_ac01_ac02_a_list_of_links_gets_hoster_suggestions` (crates/server/tests/api_tests.rs) | built |  |
| AC-03 | `ac03_links_an_enabled_hoster_or_multi_hoster_handles_get_nothing` (crates/plugin-runtime/src/registry.rs) | built |  |
| AC-04 | `ac04_ac06_a_disabled_installed_hoster_is_offered_for_enabling` (crates/plugin-runtime/src/registry.rs) | built |  |
| AC-05 | `ac05_incompatible_registry_plugins_are_left_out` (crates/plugin-runtime/src/registry.rs) | built |  |
| AC-06 | `ac06_the_first_registry_plugin_wins_a_link`, `ac04_ac06_a_disabled_installed_hoster_is_offered_for_enabling` (crates/plugin-runtime/src/registry.rs) | built |  |
| AC-07 | `ac07_without_a_registry_only_enable_suggestions_remain` (crates/plugin-runtime/src/registry.rs); `suggest_ac07_an_unreachable_registry_means_no_install_suggestion` (crates/server/tests/api_tests.rs) | built |  |
| AC-08 | `ac08_non_web_links_are_ignored` (crates/plugin-runtime/src/registry.rs); `suggest_ac08_bad_lists_are_refused` (crates/server/tests/api_tests.rs) | built |  |
| AC-09 | `suggest_ac09_the_single_url_form_is_unchanged`, `ac17_*` (crates/server/tests/api_tests.rs) | built |  |
| AC-10 | "AC-10 asks once for all links after 500 ms without change", "domain line (AC-10)" (web-ui/test/pluginSuggest.test.ts); "AC-10 shows name, link count, domains behind show all, and Install", "AC-10 names unscoped and empty domain lists, and Enable for an installed plugin" (web-ui/test/pluginSuggestions.dom.test.ts) | built |  |
| AC-11 | "AC-11 refresh asks again for the current links at once" (web-ui/test/pluginSuggest.test.ts); "AC-11 Install approves the domains, toasts and asks again", "AC-11 Enable enables the installed plugin", "AC-11 a failed refresh after an install still removes the installed card" (web-ui/test/pluginSuggestions.dom.test.ts) | built |  |
| AC-12 | "AC-12 a failed install keeps the card and its button, with an error toast", "AC-12 a failed enable keeps the card, its button and the field's links" (web-ui/test/pluginSuggestions.dom.test.ts) | built |  |
| AC-13 | "AC-13 a failed lookup reports nothing and throws nothing" (web-ui/test/pluginSuggest.test.ts); "add panel (AC-13) queues the links while a lookup is still running" (web-ui/test/pluginSuggestions.dom.test.ts) | built |  |
| AC-14 | "AC-14 drops an answer to an earlier lookup that arrives later", "AC-14 a field without links shows no cards at once and drops the pending answer", "AC-14 a failed lookup for changed links clears the cards of the old links" (web-ui/test/pluginSuggest.test.ts) | built |  |
| AC-15 | "AC-15 cards, buttons and toasts read German" (web-ui/test/pluginSuggestions.dom.test.ts); "every literal tr() key exists in the en and de tables" (web-ui/test/i18nKeys.test.ts), which sees all 11 keys of `PluginSuggestions.svelte` | built |  |

## Decisions

- 2026-11-22: Only `hoster` plugins are suggested, because multi-hosters
  need a paid account and generic plugins match every link (user's choice).
- 2026-11-22: A disabled installed plugin is offered for enabling ahead of a
  registry plugin, because the user installed it before and enabling needs
  no new domain approval (user's choice).
- 2026-11-22: `/plugins/suggest` gets a list form instead of a new endpoint,
  because it already answers "which plugin for this link" from the verified
  registry; the single-URL form stays for the CLI and older clients (user's
  choice).
- 2026-11-22: The add panel never waits for a suggestion, because adding
  links must keep working when the registry is slow or offline (user's
  choice).
- 2026-11-22 (build): The selection rules live in one pure function in the
  plugin runtime, `registry::suggest_for_links`, because they need only the
  registry index and the loaded plugin list; the server handler only loads
  both. Its unit tests cover AC-02 to AC-06 without a plugin host.
- 2026-11-22 (build): A registry plugin whose `id` is already installed is
  never an install suggestion, because installing it again is an update,
  which the Plugins page offers with its own domain comparison.
- 2026-11-22 (build): `suggest_ac09_the_single_url_form_is_unchanged` was
  green at once: it guards existing behaviour (plugin-registry AC-17) against
  the new list form, so there is no red run for AC-09.
- 2026-11-22 (build): A string or numbers in `urls` are rejected by the
  JSON extractor (`422`). A field given as `null` counts as given, so `null`
  in either field, both or neither field and more than 1 000 links answer
  `400` with an `error` message (spec-check round 1: `{"url": .., "urls":
  null}` was read as the single form).
- 2026-11-22 (build): A failed lookup after the field changed clears the
  cards (they belong to links that are gone); a failed refresh after an
  install keeps them (spec-check round 1, AC-14).
- 2026-11-22 (build): The lookup (quiet time, latest answer only, refresh)
  is a plain module, `web-ui/src/lib/pluginSuggest.ts`, tested with fake
  timers; the cards are `PluginSuggestions.svelte` inside the URL tab above
  the Add button, fed with the links the panel already counts.
- 2026-11-22 (build): After a successful install or enable the card is
  removed at once and the lookup runs again, so a failed refresh still drops
  the installed plugin's card (edge case "The refresh after an install
  fails").
- 2026-11-22 (build): spec-check round 2 found test gaps, not code gaps: the
  failed enable (AC-12) and the German texts picked by a ternary (AC-15) had
  no test. Both new tests were green at once; the German one was shown to
  fail by removing `add.suggest_enabled` from the `de` table. Every `tr()`
  call now names its key literally, so the i18n key test sees all of them.
