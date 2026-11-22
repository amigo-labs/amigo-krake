# Changelog

## [0.1.13](https://github.com/amigo-labs/amigo-krake/compare/v0.1.12...v0.1.13) (2026-11-22)


### Bug Fixes

* **web-ui:** no Update button where the server cannot update itself ([b1dcece](https://github.com/amigo-labs/amigo-krake/commit/b1dcece091d8c04196fdd1c1ac53c34e62a81d96))
* **web-ui:** no Update button where the server cannot update itself ([2acf1ac](https://github.com/amigo-labs/amigo-krake/commit/2acf1acca7193c0be08cf42110fb4201e4960781))

## [0.1.12](https://github.com/amigo-labs/amigo-krake/compare/v0.1.11...v0.1.12) (2026-11-22)


### Bug Fixes

* **tauri:** turn the bundler back on so releases get installers ([ee14bfa](https://github.com/amigo-labs/amigo-krake/commit/ee14bfa86fbf60599dc256ca790434f22b6951e8))
* **tauri:** turn the bundler back on so releases get installers ([a6030b0](https://github.com/amigo-labs/amigo-krake/commit/a6030b073ab05014c6a17edac611c8912e35b2b0))

## [0.1.11](https://github.com/amigo-labs/amigo-krake/compare/v0.1.10...v0.1.11) (2026-11-22)


### Features

* **core:** queue every entry of a plugin package with its headers and cookies (AC-01 to AC-09) ([d9406ab](https://github.com/amigo-labs/amigo-krake/commit/d9406ab9bb02e4b21389cfcb2030862800f92467))
* gofile, bunkr and filester plugins; plugin packages; desktop app ([3ee7b0c](https://github.com/amigo-labs/amigo-krake/commit/3ee7b0c62867bb9ebd0062f43e33f4bdbfff26c5))
* **plugins:** gofile, bunkr and filester hoster plugins (AC-10 to AC-16) ([b4e0d69](https://github.com/amigo-labs/amigo-krake/commit/b4e0d69ebdec388687d0c3417314408f017da366))
* **server:** desktop update controller and update-choice routes (AC-13 to AC-17, AC-25 to AC-31, AC-34, AC-35) ([b0dfbcc](https://github.com/amigo-labs/amigo-krake/commit/b0dfbcce295b1380964413bd7392a15268469370))
* **tauri:** embedded server, desktop session, tray, single instance and deep links (AC-1 to AC-12) ([fb47764](https://github.com/amigo-labs/amigo-krake/commit/fb477645b6ae97ad6bc82ff35d995198552f3852))
* **tauri:** updater artifacts, passive Windows updates and silent-setup hook (AC-19, AC-32) ([dda7e41](https://github.com/amigo-labs/amigo-krake/commit/dda7e4127efd1e550f0867a3bc1564eaf4a7c595))
* **web-ui:** desktop update question, announcement, banner and About (AC-18, AC-25 to AC-30, AC-33) ([96dcae3](https://github.com/amigo-labs/amigo-krake/commit/96dcae3ba85c0af36542543d9f6c6c51b9816af7))


### Bug Fixes

* **ci:** upload only the release's own artifacts ([5a59599](https://github.com/amigo-labs/amigo-krake/commit/5a5959985f22306d137e99b94721a3b53c4fee0e))
* **server:** keep the registry index cache in the plugins folder (desktop-app AC-6) ([76a8946](https://github.com/amigo-labs/amigo-krake/commit/76a8946e91919e00d81681fbc312ad40785021cc))
* **server:** mark the desktop session cookie Secure the way /login does ([f083a49](https://github.com/amigo-labs/amigo-krake/commit/f083a493e47c577e1c2ccbe10024e68dd620a543))
* **server:** parse desktop update bodies with axum's Json extractor ([8ae7833](https://github.com/amigo-labs/amigo-krake/commit/8ae7833308fc22b9ec968f7cef60f1b6beae77e1))
* **tauri:** guard deep-link URLs like the API and handle a second launch's link once (AC-6, AC-8, AC-11, AC-12) ([e0728a9](https://github.com/amigo-labs/amigo-krake/commit/e0728a93700b44fb8c4f118000237db44ad0f957))
* url 2.5.9 (RUSTSEC-2026-0592), escape bunkr hosts fully, web UI-independent desktop tests ([21a1a84](https://github.com/amigo-labs/amigo-krake/commit/21a1a8477c671ed1803f026ba1be28ce83e5ae55))

## [0.1.10](https://github.com/amigo-labs/amigo-krake/compare/v0.1.9...v0.1.10) (2026-11-19)


### Bug Fixes

* **plugin-runtime:** precompile the plugin guest on a thread with a large stack ([ae79803](https://github.com/amigo-labs/amigo-krake/commit/ae7980335d905ad6f32d23dfd2a191206bf44aff))
* **tauri:** call tauri_build::build, the build script failed to compile ([5671d3b](https://github.com/amigo-labs/amigo-krake/commit/5671d3b909f019eee5e6eb7f91c1cff60043905f))
* unbreak the release build (Windows guest precompile, Tauri build script, wasmtime advisories) ([4379ead](https://github.com/amigo-labs/amigo-krake/commit/4379ead1f08802fa504f9c932511514b0a6e35e2))
* update wasmtime to 50.0.2 (RUSTSEC-2026-0566, -0568, -0573) ([7c25501](https://github.com/amigo-labs/amigo-krake/commit/7c2550114eb671317541ee4fff4770b2b73e251f))

## [0.1.9](https://github.com/amigo-labs/amigo-krake/compare/v0.1.8...v0.1.9) (2026-11-19)


### Features

* **core:** queue NZBs with priority, paused, duplicate and media checks ([e83b168](https://github.com/amigo-labs/amigo-krake/commit/e83b1689659de24ed65d0aa18339831dcec0944f))


### Bug Fixes

* **cli:** add an NZB file as one Usenet download ([069a436](https://github.com/amigo-labs/amigo-krake/commit/069a43693ee01cbfc0779d60e66f9327a95858b4))
* **core:** add an NZB as a usenet download of its own ([17db32a](https://github.com/amigo-labs/amigo-krake/commit/17db32a0ac08504fed4c344c95362947576ee3e5))
* **core:** end a broken Usenet release as failed, with the reason ([99f3142](https://github.com/amigo-labs/amigo-krake/commit/99f314289354eb62f6ff67ef31ba321e1c311346))
* **core:** keep a download's category in its metadata, not as a package ([da78c7b](https://github.com/amigo-labs/amigo-krake/commit/da78c7b858629c7a1bb8eaaf74af4fbd37b4cd93))
* **core:** tell a full or unwritable disk from a broken archive when unpacking ([4e22844](https://github.com/amigo-labs/amigo-krake/commit/4e22844cddc3512e6e8c570ba85c5b6e76c1b696))
* NZBGet API as a working download client (append, category, history name, own folder) ([2813966](https://github.com/amigo-labs/amigo-krake/commit/2813966c3b21a587cad47b6fba5e35e9a14fd941))
* **server:** add watch-folder and RSS NZBs as usenet downloads ([fd7559d](https://github.com/amigo-labs/amigo-krake/commit/fd7559da92b53446b88a03eab7843214069ab5e3))
* **server:** give downloads NZBIDs that never change ([add7a1d](https://github.com/amigo-labs/amigo-krake/commit/add7a1d1d4ed333235173a9c5ede48925ab71b25))
* **server:** honour all of append's parameters in the NZBGet API ([9d3aa6d](https://github.com/amigo-labs/amigo-krake/commit/9d3aa6d7ee9f0f0e2fa5e05e6b68179863f2bb94))
* **server:** make the NZBGet API a working download client for *arr tools ([ad2dde4](https://github.com/amigo-labs/amigo-krake/commit/ad2dde4ceeb6d13e0d45942a70d024640233f9eb))
* **server:** queue uploaded NZBs as usenet downloads ([a559cf4](https://github.com/amigo-labs/amigo-krake/commit/a559cf423d24c264a710a64b82233708b7a653e2))
* **server:** report why a Usenet download failed in the NZBGet history ([cb9551c](https://github.com/amigo-labs/amigo-krake/commit/cb9551cea6cd647e9c90de6b583c0f781cf411c5))

## [0.1.8](https://github.com/amigo-labs/amigo-krake/compare/v0.1.7...v0.1.8) (2026-11-18)


### Bug Fixes

* **tauri:** give the deep-link plugin its mobile config as a list ([0b6d975](https://github.com/amigo-labs/amigo-krake/commit/0b6d97522ef0d05961fa192c213e58544e38ff99))
* **tauri:** unbreak the release build (deep-link config, Windows plugin guest) ([8bae37a](https://github.com/amigo-labs/amigo-krake/commit/8bae37a3610d9eeaecb052a92e143e981bda4e2d))

## [0.1.7](https://github.com/amigo-labs/amigo-krake/compare/v0.1.6...v0.1.7) (2026-11-18)


### Features

* **cli:** queue a link list with krake add --list (AC-18, AC-19, AC-20, AC-21, AC-22) ([553b050](https://github.com/amigo-labs/amigo-krake/commit/553b050b2bd5a5e9e81400afec29e6d932127e01))
* **cli:** translate the link list messages and embed the locales (AC-12) ([0547248](https://github.com/amigo-labs/amigo-krake/commit/05472482969f20ad259b55da9a140ad8f877775f))
* **core:** parse link lists by the shared line rule (AC-01, AC-02, AC-03, AC-04) ([243a24b](https://github.com/amigo-labs/amigo-krake/commit/243a24b13b07d4725b2602ebb22f676118936bbf))
* link list import + optional login for private networks ([92e3235](https://github.com/amigo-labs/amigo-krake/commit/92e3235f2184cce3bd272a3ab75e939d96514c2f))
* **server:** decide which requests a private network lets in without login (AC-02, AC-03, AC-05) ([f26d123](https://github.com/amigo-labs/amigo-krake/commit/f26d123cac7c2030a36f3c76611542cfa44ab746))
* **server:** let private networks in without login by auth_mode (AC-01 to AC-11) ([20660fb](https://github.com/amigo-labs/amigo-krake/commit/20660fbecdc10cf05033f6c547a54e2f492d55ee))
* **web-ui:** choose who needs to log in under Settings &gt; Security (AC-12, AC-13, AC-14, AC-15, AC-16) ([1644c99](https://github.com/amigo-labs/amigo-krake/commit/1644c99442a427121d8b7b1806744192c8060972))
* **web-ui:** import link lists by drop, paste and the add panel (AC-05, AC-06, AC-07, AC-08, AC-09, AC-10, AC-11, AC-12) ([ff72157](https://github.com/amigo-labs/amigo-krake/commit/ff721573223ecae7667d3d0c5c97b295e8f039d8))
* **web-ui:** parse link lists by the shared line rule (AC-01, AC-02, AC-03, AC-04) ([07c6485](https://github.com/amigo-labs/amigo-krake/commit/07c6485b24720ceaab8ba47ec2062fe61636a7a2))


### Bug Fixes

* never read auth_mode_from_env from a file, and match IPv6 loopback in the UI (AC-09, AC-14) ([6c8dd39](https://github.com/amigo-labs/amigo-krake/commit/6c8dd39ddfe587a804c28e9963eb1d7bcfd0ab2a))
* **server:** close the config races of login-free access and accept every loopback bind (AC-08, AC-11, AC-14) ([ef645d5](https://github.com/amigo-labs/amigo-krake/commit/ef645d50b31b3170174a9f0988f04aa2059b3847))
* **server:** refuse login-free access for unreadable forwarding headers (AC-05, AC-04) ([db2d884](https://github.com/amigo-labs/amigo-krake/commit/db2d884e7103dfd198507b548c42c3bbc335bb17))
* **server:** serialize the setup wizard's config write, and wait for the dialog in the click test (AC-08, AC-11, AC-15) ([333a4e0](https://github.com/amigo-labs/amigo-krake/commit/333a4e0d831c1ed82ac44a22ba89bc61fa778431))
* **web-ui:** show every line of a link list result toast (AC-08, AC-11) ([1a2f2c6](https://github.com/amigo-labs/amigo-krake/commit/1a2f2c63e2470b1263f8330acf0d64e28db8dbdd))
* **web-ui:** show only the stored config after a failed settings save (AC-13) ([a4dda0f](https://github.com/amigo-labs/amigo-krake/commit/a4dda0fd5a2ef34f432a92032623cddd282c933a))

## [0.1.6](https://github.com/amigo-labs/amigo-krake/compare/v0.1.5...v0.1.6) (2026-11-18)


### Bug Fixes

* **core:** keep chunks of same-name downloads apart and remove them on delete ([9aa3056](https://github.com/amigo-labs/amigo-krake/commit/9aa3056637b277fd9b859703105099cbcaa5435f))
* **core:** keep chunks of same-name downloads apart and remove them on delete ([fffbf1e](https://github.com/amigo-labs/amigo-krake/commit/fffbf1e2741c70fd4430e9123111e584a8d596ed))

## [0.1.5](https://github.com/amigo-labs/amigo-krake/compare/v0.1.4...v0.1.5) (2026-11-17)


### Bug Fixes

* **web-ui:** compact downloads toolbar on phones ([062236c](https://github.com/amigo-labs/amigo-krake/commit/062236cd79f1edeaeb3973166f26de73066c4208))

## [0.1.4](https://github.com/amigo-labs/amigo-krake/compare/v0.1.3...v0.1.4) (2026-11-17)


### Bug Fixes

* **cli:** publish a Workers static-assets config with the registry branch ([18fc7bd](https://github.com/amigo-labs/amigo-krake/commit/18fc7bd5ea7cb5c2602f7c947d8c9345447abc54))
* **cli:** publish a Workers static-assets config with the registry branch ([7106b3f](https://github.com/amigo-labs/amigo-krake/commit/7106b3fb159633fea5abcd010ff62cb3f6181f47))

## [0.1.3](https://github.com/amigo-labs/amigo-krake/compare/v0.1.2...v0.1.3) (2026-11-17)


### Features

* **cli:** plugins search and update against the paired server (AC-18 to AC-21) ([a246525](https://github.com/amigo-labs/amigo-krake/commit/a246525387e65ebae85e4eaaffa5b0a1fea17de2))
* **cli:** registry build, sign, check and publish commands (AC-10, AC-11) ([b469da1](https://github.com/amigo-labs/amigo-krake/commit/b469da1e2ea748ef3f0761b75ed162181d84c2cb))
* **plugin-runtime:** build and sign the plugin registry index (AC-01 to AC-09) ([40d9938](https://github.com/amigo-labs/amigo-krake/commit/40d99387870457b06028723cb5937f7333860583))


### Bug Fixes

* **core:** default to the published plugin registry and migrate the dead URL (AC-14, AC-15) ([24289a0](https://github.com/amigo-labs/amigo-krake/commit/24289a0b5e814a28335f01668763018abbff72ef))
* **plugin-runtime:** default registry index URL points at the published registry (AC-14) ([848c2c3](https://github.com/amigo-labs/amigo-krake/commit/848c2c3e88547c4a1d02d1ba607d583916aaf895))
* **server:** suggest plugins from the configured registry (AC-14, AC-17) ([9b13c0f](https://github.com/amigo-labs/amigo-krake/commit/9b13c0f4735ffabe2ce4dd9295494c36ac560576))

## [0.1.2](https://github.com/amigo-labs/amigo-krake/compare/v0.1.1...v0.1.2) (2026-11-16)


### Bug Fixes

* **cli:** link OpenSSL in the aarch64 Linux release build ([47b8485](https://github.com/amigo-labs/amigo-krake/commit/47b8485328d5a850b5581cf638e2f16a59ae99ad))
* **release:** installers, aarch64 Linux and Windows release builds ([a60930f](https://github.com/amigo-labs/amigo-krake/commit/a60930f198e07acd14e290fe3f85a06500994d79))

## [0.1.1](https://github.com/amigo-labs/amigo-krake/compare/v0.1.0...v0.1.1) (2026-11-15)


### Features

* **ui:** take colours, radii, shadows and fonts from the shared amigo theme (AC-01, AC-02, AC-03, AC-11) ([aa22c7b](https://github.com/amigo-labs/amigo-krake/commit/aa22c7b3212fe5740f267d3533f290ff26e50e62))
* **web-ui:** drop the neon effects and offer Dark, Light and System (AC-03, AC-04, AC-05, AC-06, AC-07, AC-08, AC-09, AC-10, AC-12, AC-13) ([c36b5f1](https://github.com/amigo-labs/amigo-krake/commit/c36b5f1203dde9adce4e93a6324786d33bc0c457))
* **web-ui:** the shared amigo theme, no neon, Dark / Light / System ([eac1c27](https://github.com/amigo-labs/amigo-krake/commit/eac1c27f1f41fe59ce802c8affefc404019023ab))


### Bug Fixes

* **ui:** take the last colour, shadow and radius of Krake's own from the tokens (AC-02) ([c359e13](https://github.com/amigo-labs/amigo-krake/commit/c359e130049556cee34316270e2f4f260cfe4b51))

## 0.1.0 (2026-11-11)


### ⚠ BREAKING CHANGES

* the workspace package names and the SDK's CLI name changed.
* the database, config, temp and container paths are renamed (migrated automatically on start, except custom bind mounts onto /etc/amigo/plugins, which must now target /etc/krake/plugins).
* the build-time keys are read only as KRAKE_REGISTRY_PUBKEY_HEX and KRAKE_UPDATE_PUBKEY_HEX, and the signing script only takes KRAKE_UPDATE_SIGNING_KEY. The workflows map them from the unchanged AMIGO_* repository variables and secret.
* the crates and their library paths are renamed.
* name the desktop executable Krake
* the desktop app has a new bundle identifier.
* the container image has a new name.
* the binaries and release archives have new names.

### Features

* /spec-verify updates spec status and acceptance criteria ([3e7c924](https://github.com/amigo-labs/amigo-krake/commit/3e7c924a0c58c7895932bcec91de2f5b51520678))
* 10 UX features — tab badge, relative time, sort, delete confirm, system theme, compact view, bandwidth limit, context menu, notifications, progress ring ([a347048](https://github.com/amigo-labs/amigo-krake/commit/a3470481b4abac5477f285cd41d107b74abbd738))
* add /spec-fix skill + update CLAUDE.md with all 5 skills ([97c13c6](https://github.com/amigo-labs/amigo-krake/commit/97c13c64be74f8eb19066ea9179a08dad30b7911))
* add automated release management with release-please ([fcbe87f](https://github.com/amigo-labs/amigo-krake/commit/fcbe87f1ccb140f4a5f6c16f3b7ed4286a72d7dc))
* add automated release management with release-please ([ce29bd6](https://github.com/amigo-labs/amigo-krake/commit/ce29bd68d4bfc2d2e1d4d74a62ececc884ddb010))
* add captcha UI, webhook settings, and notification frontend ([8fa6a38](https://github.com/amigo-labs/amigo-krake/commit/8fa6a3827c3c75cdd41c062890ae7555bcd8a419))
* add chunk persistence and resumable chunked downloads ([8ef2ca5](https://github.com/amigo-labs/amigo-krake/commit/8ef2ca552ab72d851325ed155123eb0617b8e7ba))
* add crypto, captcha manager, notifications, and webhook system ([ee40ee1](https://github.com/amigo-labs/amigo-krake/commit/ee40ee1b8347b62f87bd87350aafbcc99893e36b))
* add Docker support with container management script and update docker-compose configuration ([ecc0a58](https://github.com/amigo-labs/amigo-krake/commit/ecc0a58bf9355ea637b013fa8f9bd0e8d827a954))
* add integration tests and wire Extractor crate as URL resolver ([b92e40b](https://github.com/amigo-labs/amigo-krake/commit/b92e40b88c85b3b8881034caa2888bea69672e0d))
* add ProtocolBackend trait and activate HLS/DASH backends ([4f42a39](https://github.com/amigo-labs/amigo-krake/commit/4f42a39e66c6ce8bad47308373c6bda2e9818678))
* add retry config UI in Settings page ([7567d85](https://github.com/amigo-labs/amigo-krake/commit/7567d85dcb6290f1b6e2176419bd8d2420253928))
* add skip() to test harness for network-dependent tests ([67b3148](https://github.com/amigo-labs/amigo-krake/commit/67b31488aed24298c1266908489f945357ee4b7f))
* add spec-driven development skills (/spec, /verify, /feature) ([78aa88d](https://github.com/amigo-labs/amigo-krake/commit/78aa88dc50f2ebc2616eb3ca2cf29affac751194))
* add spec-driven development skills (/spec, /verify, /feature) ([6c84cc7](https://github.com/amigo-labs/amigo-krake/commit/6c84cc7993032dc402295525057309c6b288e27d))
* add title and duration fields to DownloadInfo ([fb236c1](https://github.com/amigo-labs/amigo-krake/commit/fb236c1a6d629b2ab29094b2e965dcb2925312a1))
* add URL resolver middleware for plugins and extractors ([66b3987](https://github.com/amigo-labs/amigo-krake/commit/66b3987da45c1cff796e5ab0c7e9ab770db20fa6))
* add Usenet UI — server management + NZB downloads pages ([0916355](https://github.com/amigo-labs/amigo-krake/commit/0916355509bafef47d8be1ed187a217bcc3b01f3))
* add YouTube integration tests and fix plugin test runner ([55e7739](https://github.com/amigo-labs/amigo-krake/commit/55e773914e675f89b7911b7dd19c2483c5a6716f))
* add YouTube integration tests and fix plugin test runner ([ea6ea43](https://github.com/amigo-labs/amigo-krake/commit/ea6ea4361c7e58191c51b01372937013a261a591))
* cache bandwidth limit and add config validation ([2732062](https://github.com/amigo-labs/amigo-krake/commit/2732062279e5dacda1be01f6117e33f03ed73298))
* **cli:** amigo-dl login / remote subcommands for device pairing ([82aaa56](https://github.com/amigo-labs/amigo-krake/commit/82aaa564602f95e576dc1d160da2f77dc4288cd8))
* complete visual redesign of amigo-downloader web-ui with Cyberpunk Arcade theme and integrated audit fixes ([63824eb](https://github.com/amigo-labs/amigo-krake/commit/63824eb746d68a7d652ad37c2bf669edbe787454))
* confirm plugin network access on install and update ([2563fad](https://github.com/amigo-labs/amigo-krake/commit/2563fad1237e40a5d814aa29ced1f83046785659))
* convert all plugins to TypeScript, smart generic-http link finder ([80233fc](https://github.com/amigo-labs/amigo-krake/commit/80233fcd7d4f100a87002036884acb7830d08725))
* **core/config:** extend ServerConfig for setup-wizard flow ([037c32c](https://github.com/amigo-labs/amigo-krake/commit/037c32c42bb4a229a198e5b77661a5f6dc391681))
* **core/storage:** sessions, api_tokens, pairing_requests tables ([5bdf810](https://github.com/amigo-labs/amigo-krake/commit/5bdf810d5d71bd2565bafdb1fb2fc2eaa2cedb40))
* cut initial v0.1.0 release ([4ac4bdb](https://github.com/amigo-labs/amigo-krake/commit/4ac4bdb4c6f9d8748ee23936609ab99ae16734bb))
* cut initial v0.1.0 release ([236c2a0](https://github.com/amigo-labs/amigo-krake/commit/236c2a04f12cb47b2e13ad4d9c581e8d4819e7b4))
* cyberpunk arcade redesign with 60:30:10 neon balance ([83dc68c](https://github.com/amigo-labs/amigo-krake/commit/83dc68c50683965ed581ada533f4a809a6aa28a8))
* **docker, docs:** opt-in Docker defaults + reverse-proxy / Tailscale guide ([659f47f](https://github.com/amigo-labs/amigo-krake/commit/659f47f6f1661d3677dc4a7f8a70d3b9929371d3))
* ETA, copy URL, keyboard nav, search, batch ops, speed graph ([8a3a684](https://github.com/amigo-labs/amigo-krake/commit/8a3a6842bfc157306b77826cf276ab16be4dcd69))
* extend plugin Host API + activate post-processing pipeline ([1c2a927](https://github.com/amigo-labs/amigo-krake/commit/1c2a92742a683bc23f41693aeb3e7ac248813882))
* feature toggles for RSS feeds and server stats ([261ccdb](https://github.com/amigo-labs/amigo-krake/commit/261ccdbc722a29310184bdddbe1e765cbf49b9bc))
* file icons, connection status, collapsible sidebar, i18n, DnD, favicon badge, aria-live, retry ([ecd607b](https://github.com/amigo-labs/amigo-krake/commit/ecd607b170045849a5b47a348f6c6e899053084b))
* harden Docker deployment (docker-security spec) ([0e0bfa2](https://github.com/amigo-labs/amigo-krake/commit/0e0bfa24b8eb7e51069222dfa3ac4fc3903ebdad))
* i18n system with JSON locale files and auto language detection ([906bcd8](https://github.com/amigo-labs/amigo-krake/commit/906bcd8dd70a629760465a801d8bc1d9e6b46fbb))
* implement real light mode theme ([e69e7aa](https://github.com/amigo-labs/amigo-krake/commit/e69e7aa46ad56ef8f44c5cb9f694bbc112508009))
* implement Usenet feature with server management and UI updates ([4e160a2](https://github.com/amigo-labs/amigo-krake/commit/4e160a2c9372fee4a880e280e455fb5ed99500ea))
* implement Usenet feature with server management and UI updates ([b7c442e](https://github.com/amigo-labs/amigo-krake/commit/b7c442eda0400f8c7f24137c2c1919086ea97bba))
* integrate bandwidth limiting and extract retry utility ([d2675f9](https://github.com/amigo-labs/amigo-krake/commit/d2675f9317f081d60498d456458db708e984ffc5))
* integrate Tauri into workspace (tauri-workspace-integration spec) ([07f89da](https://github.com/amigo-labs/amigo-krake/commit/07f89dabb40854e5f0dac9ca1983fd044febf012))
* library-based extraction, selective PAR2, NZB metadata wiring ([5c55ade](https://github.com/amigo-labs/amigo-krake/commit/5c55adee5072faa8876466e046d3b85ff98c9da0))
* local index cache, category folders, plugin suggestion API ([0e6f96d](https://github.com/amigo-labs/amigo-krake/commit/0e6f96d1ee197226d6851f0abb3ccaba599044af))
* make /spec skill collaborative and interactive ([4fe55ca](https://github.com/amigo-labs/amigo-krake/commit/4fe55cacec6a07c8189dde1a4c79d15320dba821))
* make retry options configurable via config.toml ([a34e02b](https://github.com/amigo-labs/amigo-krake/commit/a34e02be0cb2ffb69e9c01ab34b9d2bfb240f87c))
* nightly CI pipeline — CLI binaries, Docker multi-arch, Tauri installers ([35923a2](https://github.com/amigo-labs/amigo-krake/commit/35923a2beb0c0f6fbd715c4a98450a0bab4298df))
* nightly versioning with date-stamped tags and releases ([fc1bd84](https://github.com/amigo-labs/amigo-krake/commit/fc1bd84549d8c98b13454df000556e84bfe8f09d))
* NZBGet API credentials + fix 6 more bugs ([6aa486b](https://github.com/amigo-labs/amigo-krake/commit/6aa486bf64a59765c755fe96af47f257a334fd7f))
* NZBGet-compatible JSON-RPC API for Sonarr/Radarr integration ([5b01cbd](https://github.com/amigo-labs/amigo-krake/commit/5b01cbd9983e8cbabe96752bf1cce15cff0e82be))
* PAR2/unrar config toggles, RSS background poller, NZB watch folder ([52b94cd](https://github.com/amigo-labs/amigo-krake/commit/52b94cd6f1442d5f5af29a19bd24fe6689ef4225))
* plugin spec test runner with test/assert/assertEqual/assertNotNull ([5cac74c](https://github.com/amigo-labs/amigo-krake/commit/5cac74c8c577c8354113e67a744928ab6bd00431))
* plugin strategy foundation — generic extractor, multi-hoster priority, protocol hints ([ff71b5f](https://github.com/amigo-labs/amigo-krake/commit/ff71b5f1bbb0230a22b8fd5cc896da23f4b912da))
* plugin strategy foundation — generic extractor, multi-hoster priority, protocol hints ([78d57da](https://github.com/amigo-labs/amigo-krake/commit/78d57da8c0ae42d2dd3e13a78cf35e00d4b7ceee))
* **plugin-runtime:** Ed25519-signed plugin registry index (audit [#31](https://github.com/amigo-labs/amigo-krake/issues/31)) ([300cfd5](https://github.com/amigo-labs/amigo-krake/commit/300cfd5ba99e22b107373572e0c39aad508917d2))
* **plugin-runtime:** expose the plugin host API as `krake` ([302ed27](https://github.com/amigo-labs/amigo-krake/commit/302ed271e799b2f7e4714945cb0880337e3af61c))
* **plugin-runtime:** QuickJS as a WebAssembly guest module (AC-01) ([48ce491](https://github.com/amigo-labs/amigo-krake/commit/48ce491241b1c707a4644a274dcf0d1a65fc2be3))
* **plugin-runtime:** run every plugin in a Wasm instance (AC-01 to AC-06, AC-08) ([eea66fe](https://github.com/amigo-labs/amigo-krake/commit/eea66fe4e92cd069401bce94bf0d7a83326be120))
* **plugin-runtime:** run every plugin in a WebAssembly instance; [#99](https://github.com/amigo-labs/amigo-krake/issues/99) leftovers ([4c9de86](https://github.com/amigo-labs/amigo-krake/commit/4c9de86f6f8386cffa96d6ca6ea88f78f89f3121))
* **plugin-runtime:** run plugin JavaScript in an out-of-process host ([331b413](https://github.com/amigo-labs/amigo-krake/commit/331b413f920c1f6f7763655c32b4ce810fbd1a4a))
* **plugin-runtime:** run plugin JavaScript in an out-of-process host ([0bd109c](https://github.com/amigo-labs/amigo-krake/commit/0bd109cba2a1adaffa45a06eb0b7d80749cd16bc))
* **plugin-runtime:** SSRF blocklist on plugin HTTP calls ([2e7acf9](https://github.com/amigo-labs/amigo-krake/commit/2e7acf990bd93ea86f84a24ee2bd48458e1c6ebb))
* **plugin-runtime:** Wasm executor for plugin JavaScript (AC-02 to AC-13, AC-15 to AC-17) ([82955de](https://github.com/amigo-labs/amigo-krake/commit/82955de41fe39d18d6c0353c496176fe17336a60))
* **plugin-sdk/account:** AccountConfig, Session, AccountStatus plumbed through definePlugin ([d9b2e54](https://github.com/amigo-labs/amigo-krake/commit/d9b2e541922b039b077a7cb113622fbccb63286d))
* **plugin-sdk/browser:** Browser with redirects, cookies, referer, clone ([d6bb96a](https://github.com/amigo-labs/amigo-krake/commit/d6bb96ad5e7241e861a5748251ad19e1fed4c049))
* **plugin-sdk/browser:** case-insensitive Headers ([f043f0a](https://github.com/amigo-labs/amigo-krake/commit/f043f0a8a4758ab33f8f1b8c78253daf18b2d4bd))
* **plugin-sdk/browser:** CookieJar with RFC-6265-style domain matching ([dc39b4b](https://github.com/amigo-labs/amigo-krake/commit/dc39b4b8a2ee92c3493cd1a93030bbc9d076143c))
* **plugin-sdk/captcha:** captcha namespace with auto-detection and host bridge ([51f9637](https://github.com/amigo-labs/amigo-krake/commit/51f9637093859543fb297d91ca623563f7b071bc))
* **plugin-sdk/cli:** amigo-plugin CLI with new/validate/test subcommands ([4048ee4](https://github.com/amigo-labs/amigo-krake/commit/4048ee4d2dd3a5aded6b74e177069fa84ae1d3c5))
* **plugin-sdk/container:** RSDF, CCF, DLC parsers + detect() ([16fadc8](https://github.com/amigo-labs/amigo-krake/commit/16fadc83932e95a9fcae55f902cec5dad8656940))
* **plugin-sdk/errors:** PluginError, ErrorCode, factories with never-return typing ([652aa55](https://github.com/amigo-labs/amigo-krake/commit/652aa5540906416bc3801bc97d499e11359bbaf7))
* **plugin-sdk/extraction:** regex, json, encoding, html helpers ([76f4dbe](https://github.com/amigo-labs/amigo-krake/commit/76f4dbebd4ac765e540b7d8dc728bcfbe19a7905))
* **plugin-sdk/form:** Form extraction + Page.getForm + Browser.submitForm ([413ebab](https://github.com/amigo-labs/amigo-krake/commit/413ebabe63811d66cfedabebf1d0affdafe7b085))
* **plugin-sdk/host:** define host-api request/response types ([b831ccb](https://github.com/amigo-labs/amigo-krake/commit/b831ccb2a763087b14a356694f40a9c2e6a273fa))
* **plugin-sdk/host:** HostApi interface + injection hooks ([16e52fe](https://github.com/amigo-labs/amigo-krake/commit/16e52fe0e0e3c15bdb2968f388bb6d9e33d33723))
* **plugin-sdk/host:** MockHostApi with request recording and error injection ([1e30a5b](https://github.com/amigo-labs/amigo-krake/commit/1e30a5b70d96908c932ecc86b609d2c973d309a9))
* **plugin-sdk/host:** Phase 1 tests + drop node-only deps from mock ([17b15fc](https://github.com/amigo-labs/amigo-krake/commit/17b15fc69d14e9239ea4cbe0684a6c2086757340))
* **plugin-sdk/javascript:** run/unpack wrappers with permission gate ([a9b8e9d](https://github.com/amigo-labs/amigo-krake/commit/a9b8e9d3a6e1a6a0cb26c4ee962b6b3cd19534d8))
* **plugin-sdk/media:** HLS master+media, DASH, variant selection helpers ([829f0dc](https://github.com/amigo-labs/amigo-krake/commit/829f0dc835d1a35c152ddee5a26f4e7380bfe859))
* **plugin-sdk/plugin:** shared types, PluginContext, definePlugin, defineDecrypter ([65220e8](https://github.com/amigo-labs/amigo-krake/commit/65220e8e1a9f3168cfad55b310327d60312f2de0))
* **plugin-sdk/utility:** parseSize, parseDuration, parseDate, formatFilename ([629b5dd](https://github.com/amigo-labs/amigo-krake/commit/629b5ddb82933c81172790b190727acacdcaf466))
* **plugin-sdk:** introduce @amigo/plugin-sdk (plan + implementation) ([12fb9fc](https://github.com/amigo-labs/amigo-krake/commit/12fb9fc4fc3e2406c617d706b1e92ac34d1d6f34))
* plugins test always enables verbose/debug logging ([4713026](https://github.com/amigo-labs/amigo-krake/commit/47130264603fc00877b714f209d97d545a5ee64a))
* **plugins:** declare apiVersion and permissions.domains ([5b2489f](https://github.com/amigo-labs/amigo-krake/commit/5b2489f7ecef2ed7726f1ff8bf5951c7191121fd))
* rename CLI binary to amigo-dl and enhance direct download functionality ([74a8088](https://github.com/amigo-labs/amigo-krake/commit/74a8088cfaebb073dfec0d4016cee3711e7c378c))
* resolve() returns DownloadPackage with grouped downloads ([d8e257e](https://github.com/amigo-labs/amigo-krake/commit/d8e257ee5d924109f40aeaf3b356c756bdd6019c))
* server statistics UI behind feature toggle ([ca53033](https://github.com/amigo-labs/amigo-krake/commit/ca5303311e4160d0cf8d8daf21aa404b5779589a))
* **server/auth:** cookie + api-token auth, setup-guard, proxy-aware IP ([d2db4d4](https://github.com/amigo-labs/amigo-krake/commit/d2db4d4d8c5729fea7411ce72ba707553961602d))
* **server:** add bulk download endpoints and fix the retry action ([c4cb317](https://github.com/amigo-labs/amigo-krake/commit/c4cb317edece110982cd3975cb446a060460fec5))
* **server:** add POST /api/v1/downloads/container for DLC import ([b7c4c7d](https://github.com/amigo-labs/amigo-krake/commit/b7c4c7d66f6993b4410023ec2dadac3178aac07a))
* **server:** background plugin auto-update task (opt-in) ([2f0fa57](https://github.com/amigo-labs/amigo-krake/commit/2f0fa5717c66725cd30ee459ca5d6ab1b380b56e))
* **server:** bind to loopback by default, add bearer-token auth, tighten CORS ([a09d19c](https://github.com/amigo-labs/amigo-krake/commit/a09d19c6d92b2b222262707b7f8987c2eb3030c4))
* **server:** bind to loopback by default, add bearer-token auth, tighten CORS ([e8ded1c](https://github.com/amigo-labs/amigo-krake/commit/e8ded1cc2649e42047d0d0e3f0461e7ccc214c39))
* **server:** device-pairing endpoints + headless setup from env vars ([05e427b](https://github.com/amigo-labs/amigo-krake/commit/05e427bf4237d77b8658af303026bc92037939cd))
* **server:** live setup_complete + end-to-end auth flow tests ([fc37062](https://github.com/amigo-labs/amigo-krake/commit/fc370626f51a94ef6a699194c7887b1a6ad6a635))
* **server:** name the session cookie krake_session ([e3d6980](https://github.com/amigo-labs/amigo-krake/commit/e3d6980655af2692c46b4df3244655a2bb648ef4))
* **server:** send X-Krake-* webhook headers ([8e578ce](https://github.com/amigo-labs/amigo-krake/commit/8e578ce3d918a5d74bc94fa8a29aff14236cba6e))
* **server:** setup wizard + login/logout/me endpoints ([ef137fa](https://github.com/amigo-labs/amigo-krake/commit/ef137faaca2695a91ef704367c0aa38e15188c67))
* **site:** add marketing site skeleton + pnpm workspace refactor ([b60f139](https://github.com/amigo-labs/amigo-krake/commit/b60f139c72409dc58f3db4762c6c798d66756d50))
* stream progress over WebSocket and add URL-based routing ([f4a8199](https://github.com/amigo-labs/amigo-krake/commit/f4a8199eff04b6a2c1effcfaea41d04e0bbabe3c))
* suggest plugin installation when URL matches registry ([f116473](https://github.com/amigo-labs/amigo-krake/commit/f116473cc0a7a51e2ce5f3e5f488bf0a54fd85ae))
* **tauri:** register the krake:// deep-link scheme ([18bb697](https://github.com/amigo-labs/amigo-krake/commit/18bb69775a27c10bf9c1867391eb695e9bc3cf43))
* **tauri:** use the Kr pixel monogram as the app icon ([ad0a94d](https://github.com/amigo-labs/amigo-krake/commit/ad0a94d1063a90a47489a41967b9e2731d523320))
* TUI output modes, multi-hoster plugins, XFS plugin, host-API extensions ([4cdc898](https://github.com/amigo-labs/amigo-krake/commit/4cdc89824085b985a5db69717e966c4613112a8a))
* **ui:** add accessible primitives to @amigo/ui ([ee41e07](https://github.com/amigo-labs/amigo-krake/commit/ee41e0715da47ad9500775ca2496ebff088a5cde))
* **ui:** one accent, the brand lime, instead of six palettes ([519a2a4](https://github.com/amigo-labs/amigo-krake/commit/519a2a48508b2134d800f4f40c05aa9a0d1e2b63))
* **ui:** split accent tokens by role and self-host Inter ([4d7d8c8](https://github.com/amigo-labs/amigo-krake/commit/4d7d8c8fb87e61a0312aeaff61f07a5d05c96f78))
* **ui:** use the Kr pixel monogram as favicon, PWA and in-app logo ([0547a9e](https://github.com/amigo-labs/amigo-krake/commit/0547a9e3848c5cb858b29eb670cdcb83c03c4e64))
* Usenet protocol dispatch, categories, duplicates, queue reorder ([d106dc2](https://github.com/amigo-labs/amigo-krake/commit/d106dc2d2a98a5d193fb148967850e8d32780920))
* VS Code dark mode, unified side panel, plus button in header ([361e4b8](https://github.com/amigo-labs/amigo-krake/commit/361e4b898753e96726757d9818d88d31750f6f5f))
* VS Code dark mode, unified side panel, plus button in header ([6f3871b](https://github.com/amigo-labs/amigo-krake/commit/6f3871bb68b551c2e7ebf7f05e43dbd21756630e))
* **web-ui:** "Command Deck" UI/UX redesign — palette, mobile nav, motion, i18n & a11y ([d46b593](https://github.com/amigo-labs/amigo-krake/commit/d46b59301338060cfad47eb8e1fdec1c9b5024ce))
* **web-ui:** bring the off-system screens into the design system ([5ec52bf](https://github.com/amigo-labs/amigo-krake/commit/5ec52bf544614aaac1c32ae7505d5281b6a556e9))
* **web-ui:** command palette, mobile nav, header status bar, motion system ([9579b07](https://github.com/amigo-labs/amigo-krake/commit/9579b07ca612a4502eb7d77afe18e003af669cef))
* **web-ui:** download rows lay out by their own width, denser phone list ([b5a0ab7](https://github.com/amigo-labs/amigo-krake/commit/b5a0ab70f469cd5503e01f9307e906a37c3b01eb))
* **web-ui:** drop the colour palette setting ([41e284f](https://github.com/amigo-labs/amigo-krake/commit/41e284ff1f43e4773ae650081c304e9010db435e))
* **web-ui:** i18n completion, focus traps, undo, settings a11y ([148f82a](https://github.com/amigo-labs/amigo-krake/commit/148f82ab05273b6cfe3d64a29cfe0acc3bd31f38))
* **web-ui:** make the history page usable ([4df456d](https://github.com/amigo-labs/amigo-krake/commit/4df456db0a2cc376ae9f000f9411235f9eacca28))
* **web-ui:** offline empty state, plugin enable/disable toggle ([46fbea3](https://github.com/amigo-labs/amigo-krake/commit/46fbea3958e09e48ebb148b8428214c13ac62a36))
* **web-ui:** setup wizard, login page, pairing modal, auth gate ([271b9ed](https://github.com/amigo-labs/amigo-krake/commit/271b9edcfcd3c79eeaea9db4607fc27f0e1db9c9))
* **web-ui:** show a download's host instead of its full URL ([4ea25aa](https://github.com/amigo-labs/amigo-krake/commit/4ea25aa606dbe2444e5915bf5c296068a7c4f3ed))
* **web-ui:** unify the download list and make it keyboard-operable ([ea86852](https://github.com/amigo-labs/amigo-krake/commit/ea86852e3c9af64baf6380b2f6309827055d7f73))
* **web-ui:** wire up the plugin marketplace ([39924fd](https://github.com/amigo-labs/amigo-krake/commit/39924fde315dc542dc04abecb4d3a0e407c496e7))
* wire captcha, webhook, and notification endpoints end-to-end ([3008952](https://github.com/amigo-labs/amigo-krake/commit/30089521d3b9b70a3cf0f7d448d37cfb3bf7505d))


### Bug Fixes

* /spec-fix updates affected specs when fix changes spec'd behavior ([c8374a9](https://github.com/amigo-labs/amigo-krake/commit/c8374a9148b9e529bf2a526d39ccf8c7297e6528))
* adapt to rand 0.10 API changes ([232fc7a](https://github.com/amigo-labs/amigo-krake/commit/232fc7a1f148b11483d3b88745cc18986b9f9f58))
* add libclang-dev to Docker build for rquickjs bindgen ([396ba06](https://github.com/amigo-labs/amigo-krake/commit/396ba0686c7c88d46f90a87581be40f2f86c2156))
* add protobuf-compiler and Cargo.lock for CI builds ([fd22ab0](https://github.com/amigo-labs/amigo-krake/commit/fd22ab0f9c05f4923c948d94be17a4fffa4acb77))
* add protobuf-compiler and Cargo.lock for CI builds ([b3f3104](https://github.com/amigo-labs/amigo-krake/commit/b3f3104d441d4071bdd731d8e92870870dc23d7c))
* address all Copilot review comments on PR [#15](https://github.com/amigo-labs/amigo-krake/issues/15) ([655ef30](https://github.com/amigo-labs/amigo-krake/commit/655ef30a70bc39fecf8bd53f8fb454a230c109c5))
* address Copilot review — bound login RL map, persist full reorder set ([11080c8](https://github.com/amigo-labs/amigo-krake/commit/11080c8df3edbb8ab85959069c396d5b463bf518))
* address Copilot review comments on PR [#14](https://github.com/amigo-labs/amigo-krake/issues/14) ([46a2342](https://github.com/amigo-labs/amigo-krake/commit/46a2342ad04494e95716fe8c6152b19042ee38f1))
* address Copilot review comments on PR [#19](https://github.com/amigo-labs/amigo-krake/issues/19) ([a0d2095](https://github.com/amigo-labs/amigo-krake/commit/a0d2095af03a0851c20f608396c72d38b8046e16))
* address Copilot review comments on PR [#19](https://github.com/amigo-labs/amigo-krake/issues/19) ([250e874](https://github.com/amigo-labs/amigo-krake/commit/250e874fdd7ddbfc849bdc2a735ece918de586bb))
* address Copilot review comments on PR [#21](https://github.com/amigo-labs/amigo-krake/issues/21) ([af1016f](https://github.com/amigo-labs/amigo-krake/commit/af1016f18a3f52e788405fc8861c8e4b783ed971))
* address HIGH-severity security tickets ([#29](https://github.com/amigo-labs/amigo-krake/issues/29), [#30](https://github.com/amigo-labs/amigo-krake/issues/30), [#32](https://github.com/amigo-labs/amigo-krake/issues/32)) ([65da865](https://github.com/amigo-labs/amigo-krake/commit/65da865fdd91bed7d840a628bb22c0955b6c29e4))
* address PR [#72](https://github.com/amigo-labs/amigo-krake/issues/72) review — .sig asset, owner cache bound, blank secret ([d84fa17](https://github.com/amigo-labs/amigo-krake/commit/d84fa1739c342da16395d5ab3ee356dbc63047e9))
* bugs, dead code, misleading stubs, and UX gaps across backend and web-ui ([db7525f](https://github.com/amigo-labs/amigo-krake/commit/db7525f4b01f1d9b642dfccb4278dca4bc1a1748))
* bugs, dead code, misleading stubs, and UX gaps across backend and web-ui ([c3215d6](https://github.com/amigo-labs/amigo-krake/commit/c3215d6a5df1d2249a0d863a5e50849ce6ae83fa))
* change default port from 8080 to 1516 to avoid servarr conflicts ([c82a140](https://github.com/amigo-labs/amigo-krake/commit/c82a1400637f43be13a87c11be9d3e57490dacaf))
* CI failures — suppress lib dead_code warnings, fix Docker image tags ([4785154](https://github.com/amigo-labs/amigo-krake/commit/4785154dc08abe3a1a305e828b34efd98ad33c00))
* CI pipeline failures from Tauri workspace integration ([2428a39](https://github.com/amigo-labs/amigo-krake/commit/2428a39a7ac62f5248494763d39eddd80c558960))
* **ci:** build cleanly on the current stable toolchain; clarify signer naming ([66af253](https://github.com/amigo-labs/amigo-krake/commit/66af253ab60116c55e53498290387cd32085a6ef))
* **ci:** bump versions without release-please's rust strategy ([3bf9f35](https://github.com/amigo-labs/amigo-krake/commit/3bf9f354618aa9d4ee6de802a22b8dfdc87a03c2))
* **ci:** bump versions without release-please's rust strategy ([0ea62c7](https://github.com/amigo-labs/amigo-krake/commit/0ea62c72492bae217dff8592ffaad68d6412ae5e))
* **ci:** grant packages:write to release-please caller workflow ([b8288e6](https://github.com/amigo-labs/amigo-krake/commit/b8288e6e664a2b348586e3d693115258f93e9826))
* **ci:** grant packages:write to release-please caller workflow ([0939968](https://github.com/amigo-labs/amigo-krake/commit/09399686cfe7d73cbb47e6e06c683b921082e848))
* **ci:** green main again: rustix 1.1.6 for cargo audit, release PR auto-merge without squash ([cb5d890](https://github.com/amigo-labs/amigo-krake/commit/cb5d8903ff8ad44bc9683da8461be770ab04576a))
* **ci:** migrate web-ui, plugin-sdk, and docker build to pnpm ([b2402e9](https://github.com/amigo-labs/amigo-krake/commit/b2402e9ccbcb6b68a4d10c3c3ccc1b66060a1a4b))
* **cli:** drop unused cancel sender in direct download path ([528fb44](https://github.com/amigo-labs/amigo-krake/commit/528fb44df3eac94e0b9d9c2c3a30451f0090e305))
* **cli:** satisfy clippy::manual_checked_ops for download progress ([0568acc](https://github.com/amigo-labs/amigo-krake/commit/0568acc0e03a03aa666527dccd7e17f29947317b))
* code review audit — bugs, dead code, and missing integrations ([0c51df1](https://github.com/amigo-labs/amigo-krake/commit/0c51df1eff5f846473046b5424a4c02c12eb6e9d))
* code review audit — bugs, dead code, and missing integrations ([b879ce0](https://github.com/amigo-labs/amigo-krake/commit/b879ce0536b90cc78af5022a231a92972329ceda))
* code-review cleanup — security, download-engine correctness, plugins, web-ui ([29fbb1c](https://github.com/amigo-labs/amigo-krake/commit/29fbb1c93603a3b63bc7d12fa69bdb608af1df15))
* copy web-ui dist to correct path in Docker build ([b82c2eb](https://github.com/amigo-labs/amigo-krake/commit/b82c2ebbc6221dba0a89738fca674c7d314e3fbc))
* **core,plugin-runtime:** address PR review on segment streaming & cache ([02ee1a8](https://github.com/amigo-labs/amigo-krake/commit/02ee1a8f45f9982e051f0a35408a19a3fd507b9a))
* **core/chunk:** kill the underflow in ChunkPlan::split ([6618743](https://github.com/amigo-labs/amigo-krake/commit/6618743c919c8e141d0361e0a15f778a10f467b0))
* **core/coordinator:** bigger event buffer, share progress channel across retries ([9e36ef8](https://github.com/amigo-labs/amigo-krake/commit/9e36ef8a45b9c05176505ad0afc6f4bf339fe8d7))
* **core/http:** fsync downloaded files before declaring success ([928e9cf](https://github.com/amigo-labs/amigo-krake/commit/928e9cffbeb35b1c6eb877be8a787f401aebc5a5))
* **core/nntp:** hostname validation, IO timeouts, explicit TLS verify ([f5c792b](https://github.com/amigo-labs/amigo-krake/commit/f5c792b85b6ccfd6904956155b12a06509a49dbf))
* **core/postprocess:** block ZIP-Slip via symlink entries ([aee87db](https://github.com/amigo-labs/amigo-krake/commit/aee87db601296e890bcfe15b2dd4efbfb5454c61))
* **core:** a paused download keeps the progress it had reached ([e4cf217](https://github.com/amigo-labs/amigo-krake/commit/e4cf21730f2803b1ad6b069c99aeacb6bada29a6))
* **core:** address Copilot review on [#58](https://github.com/amigo-labs/amigo-krake/issues/58) ([481149a](https://github.com/amigo-labs/amigo-krake/commit/481149accb525255a2b4e00aeb11696d04499f7e))
* **core:** allow cancellation during retry and harden speed calc ([1aa2de1](https://github.com/amigo-labs/amigo-krake/commit/1aa2de16c295356e8167e1bfba46761d59c61a00))
* **core:** bound WAL growth with autocheckpoint ([4e81121](https://github.com/amigo-labs/amigo-krake/commit/4e81121d0d386bc66d5af8422090959d974eb93d)), closes [#38](https://github.com/amigo-labs/amigo-krake/issues/38)
* **core:** cancel the in-flight task before retrying a download ([a47edfd](https://github.com/amigo-labs/amigo-krake/commit/a47edfd3f169b66e88f2723b17e9462961183194))
* **core:** close TOCTOU race in add_download_with_options ([#53](https://github.com/amigo-labs/amigo-krake/issues/53)) ([470f921](https://github.com/amigo-labs/amigo-krake/commit/470f92177f506f3a484329803897daff7acbe6f1))
* **core:** close TOCTOU race in add_download_with_options ([#53](https://github.com/amigo-labs/amigo-krake/issues/53)) ([eb75821](https://github.com/amigo-labs/amigo-krake/commit/eb758214adf43258536dce1266695919b8bfeb2d))
* **core:** enforce retry base_delay floor to prevent hot-loop ([2da7773](https://github.com/amigo-labs/amigo-krake/commit/2da7773177420cd5a1a25ca174b23b9e7e8d8638)), closes [#37](https://github.com/amigo-labs/amigo-krake/issues/37)
* **core:** log completed downloads in the history ([0d1e692](https://github.com/amigo-labs/amigo-krake/commit/0d1e692a92f434aeeaac43e8c59a634a476add4a))
* **core:** mark a download as retrying during the backoff, not during the retry ([0ff59bf](https://github.com/amigo-labs/amigo-krake/commit/0ff59bf14d52423c6d97d125dba81f09701fd1e2))
* **core:** migrate rand usage to the 0.9 API ([6fc5363](https://github.com/amigo-labs/amigo-krake/commit/6fc536357ff37fd5989ae5b34a4a3c0ff27b5529))
* **core:** record filename and size of plain HTTP downloads, fail fast on 4xx ([4b08fd9](https://github.com/amigo-labs/amigo-krake/commit/4b08fd9cad30366121c677d9be4f4a90c59ee9c2))
* **core:** repair Usenet binary path, bandwidth hang, chunk corruption, pause ([fbb5c2b](https://github.com/amigo-labs/amigo-krake/commit/fbb5c2b5f0ba52b524e93e395bf68bcc83ac8fe0))
* **core:** require checksum for self-update, guard 7z/RAR/tar against zip-slip ([f82fbd8](https://github.com/amigo-labs/amigo-krake/commit/f82fbd8e07cf9ece4321fad1a3612e9411a088f9))
* **core:** robustness fixes across NZB, DASH, config, chunking, NNTP, coordinator ([402d46d](https://github.com/amigo-labs/amigo-krake/commit/402d46dbf2dbbe37873a4b64869a9893235457c7))
* **core:** tear down progress reporter and sibling chunk tasks on chunk failure ([ca3df34](https://github.com/amigo-labs/amigo-krake/commit/ca3df341db22d60eac1d3fb4b9769c2a4507d845))
* **core:** upgrade rusqlite to 0.41 for the libsqlite3-sys advisory ([f273800](https://github.com/amigo-labs/amigo-krake/commit/f27380008ae2a99647ed7e6e73f4319789afa917))
* **core:** verify reassembled chunk size before rename ([b2fe9fb](https://github.com/amigo-labs/amigo-krake/commit/b2fe9fb2db959593220b83ddbe3c5a232fb0aee9)), closes [#34](https://github.com/amigo-labs/amigo-krake/issues/34)
* correct addToast argument order across all pages ([ad8b015](https://github.com/amigo-labs/amigo-krake/commit/ad8b0154b7a55b405ee60a5da6097ec8058f3a1b))
* critical security and logic bugs across codebase ([9af9e16](https://github.com/amigo-labs/amigo-krake/commit/9af9e16d254539bec83c55ad2697d7103abcaeb8))
* critical security and logic bugs across codebase ([a8a0185](https://github.com/amigo-labs/amigo-krake/commit/a8a0185b5447efc72253061979a6cb2629a1badb))
* **deps:** make rand 0.10 bump compile (openssl/tar cargo group update) ([43c62ee](https://github.com/amigo-labs/amigo-krake/commit/43c62ee3bce9308ca4f955803326fa4db6220353))
* **deps:** make tauri 2.11.1 + rand 0.9 bump (PR [#61](https://github.com/amigo-labs/amigo-krake/issues/61)) build green ([a1d14b1](https://github.com/amigo-labs/amigo-krake/commit/a1d14b17962797e7a76c403ca1cd2ec89e599f1d))
* **deps:** override fast-uri to &gt;=3.1.2 to clear pnpm audit ([f949c25](https://github.com/amigo-labs/amigo-krake/commit/f949c25ba607dd2897c37d60e9ace76e1eda8c05))
* **deps:** override svelte/ws/devalue to clear pnpm audit ([01b410f](https://github.com/amigo-labs/amigo-krake/commit/01b410f5639ba07347f6ce002b02dcd46b95e3af))
* **deps:** pin vulnerable transitive npm deps via pnpm overrides ([1645049](https://github.com/amigo-labs/amigo-krake/commit/1645049efcf9107c7d608b2b4456b4568ac1635d))
* **deps:** replace sevenz-rust with sevenz-rust2 and bump vulnerable crates ([292bcda](https://github.com/amigo-labs/amigo-krake/commit/292bcda56271c2c3a6a313056ea9529b41530f3f))
* **deps:** resolve cargo and pnpm audit advisories ([7270e69](https://github.com/amigo-labs/amigo-krake/commit/7270e69842c1d4de57b7524afb1f3d754950446f))
* **deps:** resolve cargo and pnpm audit advisories ([5ddf08a](https://github.com/amigo-labs/amigo-krake/commit/5ddf08a2c77084d00061b00eb51bee97fa8b1b58))
* **deps:** resolve cargo-audit advisories (anyhow bump + quick-xml ignores) ([fa990a8](https://github.com/amigo-labs/amigo-krake/commit/fa990a8bb4c807edc5d593c3c09462cd88dcc9a7))
* **deps:** update hyper and the Tauri updater for open advisories ([f5c586c](https://github.com/amigo-labs/amigo-krake/commit/f5c586c398f61ff4d561f8a55b2b271789618f89))
* **docker:** add entrypoint with PUID/PGID and volume checks ([3088a0d](https://github.com/amigo-labs/amigo-krake/commit/3088a0d2e73be08409cac75801440a8bc88af63c)), closes [#42](https://github.com/amigo-labs/amigo-krake/issues/42)
* don't check off acceptance criteria in spec files ([6d6b526](https://github.com/amigo-labs/amigo-krake/commit/6d6b526cea5ab297c426b85df0e10aa8c3d73702))
* **extractors/youtube:** TTL + LRU on player-JS cache, deadline on QuickJS ([ee69510](https://github.com/amigo-labs/amigo-krake/commit/ee695105cd6488bf123f829d9b9cd033e5bb72ba))
* harden coordinator, resume, extraction, plugin updates, JSON parsing ([2ec8b78](https://github.com/amigo-labs/amigo-krake/commit/2ec8b783f17952ddb5fd3382c1b239b012eb7ffe))
* make webhook signing mandatory ([#29](https://github.com/amigo-labs/amigo-krake/issues/29)) ([40844ff](https://github.com/amigo-labs/amigo-krake/commit/40844ff50157df3526bcb37dc13328b7131f6c39))
* multiple bugs across core, plugins, web-ui, and docker ([ae75e59](https://github.com/amigo-labs/amigo-krake/commit/ae75e5910b3b1b30c30d552dc3675b6b75e0b85d))
* multiple bugs across core, plugins, web-ui, and docker ([f451bf9](https://github.com/amigo-labs/amigo-krake/commit/f451bf9f3ccac0dff2e399bcf116c967e08ebdc8))
* multiple bugs across core, plugins, web-ui, and docker ([689eae8](https://github.com/amigo-labs/amigo-krake/commit/689eae868897a3c5c136540c4be548084b325364))
* multiple bugs across core, plugins, web-ui, and docker ([9d110d8](https://github.com/amigo-labs/amigo-krake/commit/9d110d8827232bb66712ff59b2039f695902f6c4))
* nightly build missing server binary and broken artifact upload ([51d09b5](https://github.com/amigo-labs/amigo-krake/commit/51d09b56b93ec4b973e1c8ab7361bde6837a5cb3))
* nightly build missing server binary and broken artifact upload ([d6c920a](https://github.com/amigo-labs/amigo-krake/commit/d6c920acee5dda8229037d8c7299293328762946))
* path-traversal, resume corruption, zip-bomb, plugin updates, JSON parsing ([88964a6](https://github.com/amigo-labs/amigo-krake/commit/88964a6f19010fecc455c6d0a1fc6e703560a258))
* plugin sandbox hardening ([#78](https://github.com/amigo-labs/amigo-krake/issues/78), [#80](https://github.com/amigo-labs/amigo-krake/issues/80)–[#85](https://github.com/amigo-labs/amigo-krake/issues/85)) + green dependency audits (supersedes [#86](https://github.com/amigo-labs/amigo-krake/issues/86)) ([351f9d8](https://github.com/amigo-labs/amigo-krake/commit/351f9d8c27b72da11260a8c47b4667945cc0295f))
* **plugin-runtime:** address review findings on the plugin host ([f9ed787](https://github.com/amigo-labs/amigo-krake/commit/f9ed7871cc724b58e4100fa6a7819d5cb85a809c))
* **plugin-runtime:** cap regex / HTML / base64 inputs ([d8c0982](https://github.com/amigo-labs/amigo-krake/commit/d8c09821e57fde222a6b930e2c391d52ae15c921))
* **plugin-runtime:** collapse redirect match arm (clippy 1.96 collapsible_match) ([f75868b](https://github.com/amigo-labs/amigo-krake/commit/f75868b6a495b016b7be6df256a829428253282d))
* **plugin-runtime:** enforce per-plugin storage quota ([a9d6db8](https://github.com/amigo-labs/amigo-krake/commit/a9d6db85972663f51aff004a1956c2f486e27d23))
* **plugin-runtime:** enforce resolve/postProcess timeouts via interrupt handler ([f0bc0bb](https://github.com/amigo-labs/amigo-krake/commit/f0bc0bb6ac896fa5d793a6fb5cc2c88e0c29d5c9))
* **plugin-runtime:** harden the plugin sandbox boundary ([b528714](https://github.com/amigo-labs/amigo-krake/commit/b528714bd129594192a0b77fe08ad166f227f922))
* **plugin-runtime:** inject registry Ed25519 pubkey at compile time ([e2a2d48](https://github.com/amigo-labs/amigo-krake/commit/e2a2d489ee3612cec0d789a87c63448b37a87b00))
* **plugin-runtime:** isolate plugin storage by declared id, harden HTTP ([7157bcd](https://github.com/amigo-labs/amigo-krake/commit/7157bcd9550874b4818a2317375511bb21a889fd))
* **plugin-runtime:** per-plugin cookie jars ([4aff9a2](https://github.com/amigo-labs/amigo-krake/commit/4aff9a24ecb08e2c4648d013fe5b3d9525d377dd))
* **plugin-runtime:** settle a Promise returned by resolve() ([b329343](https://github.com/amigo-labs/amigo-krake/commit/b3293431eea25e4cccb562cf6b1660bacbd5df39))
* **plugin-runtime:** show a thrown non-Error value as its string form ([60f6e46](https://github.com/amigo-labs/amigo-krake/commit/60f6e462be098c27fb28e48d13b36ef76719b7a9))
* **plugin-runtime:** validate registry candidates before they reach disk ([10ffee3](https://github.com/amigo-labs/amigo-krake/commit/10ffee39c5d097e3b1d2ac99989d5d1f4dbf5386))
* **plugin-runtime:** Wasm executor reports a missing return value like the native engine (AC-06, AC-07) ([808d29c](https://github.com/amigo-labs/amigo-krake/commit/808d29ca0b9cffbd691e0081390fc3533b9bea97))
* **plugin-sdk:** address Copilot review comments ([b12dd32](https://github.com/amigo-labs/amigo-krake/commit/b12dd321689fe3878ee9f17c0f176812703cc742))
* **plugins:** correct link pairing, honor XFS wait, validate keys before store ([14d010e](https://github.com/amigo-labs/amigo-krake/commit/14d010e6ce946cbfdfda9b43a50cdb52de54f667))
* **plugins:** strip anchor tags to a fixed point (CodeQL: incomplete sanitization) ([c38d71f](https://github.com/amigo-labs/amigo-krake/commit/c38d71fcde95d5970546db32b4299efff9089f01))
* prevent panic on malformed NZB with missing &lt;/file&gt; closing tag ([f6f62f2](https://github.com/amigo-labs/amigo-krake/commit/f6f62f23eeb2b94e9c871128a91413246458dd2b))
* queue stall, non-atomic history, regex perf, test masking ([e019ebe](https://github.com/amigo-labs/amigo-krake/commit/e019ebee969968a3274612f623cbb9aa1f01f71f))
* re-enable Click'n'Load in Tauri via amigo-server crate ([d305615](https://github.com/amigo-labs/amigo-krake/commit/d30561549014a0161d91f88509a72032b175e4a5))
* re-sync agent skills (acceptance-criteria headings in any case) ([0801589](https://github.com/amigo-labs/amigo-krake/commit/080158912ff1652797877fd94b7acb622925b5b2))
* re-sync agent skills (commit gate, spec-build completion) ([634c469](https://github.com/amigo-labs/amigo-krake/commit/634c469370de499aa14815c301c90c22cd146092))
* resolve all CI clippy warnings and unused import errors ([e8d2026](https://github.com/amigo-labs/amigo-krake/commit/e8d20261a482bbb62ac3cd0be7644e6445fb5433))
* resolve all CI pipeline failures (clippy, svelte-check, warnings) ([196c98f](https://github.com/amigo-labs/amigo-krake/commit/196c98fa8ea54ae36dae0654562141490cbafa0a))
* resolve clippy redundant_closure warning in ZIP extraction ([53e8cbb](https://github.com/amigo-labs/amigo-krake/commit/53e8cbb439e19c8db3904a7cb1a7606e55bf104d))
* resolve clippy redundant_closure warning in ZIP extraction ([5037c1c](https://github.com/amigo-labs/amigo-krake/commit/5037c1c040ea177fad20a1188efcc87a598b2042))
* resolve nightly build errors — missing icons, clippy warnings, and release-build stub ([beb08a0](https://github.com/amigo-labs/amigo-krake/commit/beb08a06ddf4825c0f90817a993d47e5ccfac0e3))
* resolve nightly build failures across all platforms ([53e4461](https://github.com/amigo-labs/amigo-krake/commit/53e44617552d84732e2b53a345fa82bdd656c021))
* resolve nightly build failures across all platforms ([804810a](https://github.com/amigo-labs/amigo-krake/commit/804810a0a493269cee31910f417660cefd5ff623))
* resolve nightly build failures across macOS, aarch64-linux, and … ([93be711](https://github.com/amigo-labs/amigo-krake/commit/93be7113a2e5f695c7793dfd262b3ba6a5374c98))
* resolve nightly build failures across macOS, aarch64-linux, and Tauri installers ([8be1cda](https://github.com/amigo-labs/amigo-krake/commit/8be1cdad428782757ec06deece1e703bf197dd6a))
* resolve pre-existing clippy warnings in server crate ([a4eb6b3](https://github.com/amigo-labs/amigo-krake/commit/a4eb6b3b43e495ac54aa73e6954e69ba6c12fb1d))
* restore libclang-dev in Docker for rquickjs (plugin-runtime) ([f852009](https://github.com/amigo-labs/amigo-krake/commit/f8520098951c010a0a001ffe62f7047e31d153cd))
* scope WebSocket download events per owner ([#32](https://github.com/amigo-labs/amigo-krake/issues/32)) ([d28e98b](https://github.com/amigo-labs/amigo-krake/commit/d28e98b90fb98a9e1e9c54a595ba492c76e355ed))
* **sdk,core:** correct container detection ordering and DLC docs (F1/P4) ([5eda1ea](https://github.com/amigo-labs/amigo-krake/commit/5eda1ea540215d8095d8661a88e10cb0b817e456))
* **server:** apply baseline security headers to every response ([f2e6b5d](https://github.com/amigo-labs/amigo-krake/commit/f2e6b5d47113a2f0d98e6adcae8895f3427f4992))
* **server:** avoid Instant underflow in feedback rate limiter ([6179539](https://github.com/amigo-labs/amigo-krake/commit/617953991d7031337ae0b6d5bc869d07b30ce182))
* **server:** cap upload body size for /downloads/nzb and /downloads/container ([050481c](https://github.com/amigo-labs/amigo-krake/commit/050481c35595a5757de708d6190d77cd37a50971))
* **server:** close NZBGet JSON-RPC auth bypass ([868d3c7](https://github.com/amigo-labs/amigo-krake/commit/868d3c71fc1ddd583ee2ecfc160ab3c5ae27cf57))
* **server:** mark session cookies Secure over HTTPS, clear both on logout ([2fff42f](https://github.com/amigo-labs/amigo-krake/commit/2fff42f74ae7125ee03b84d2496e7a14267097e1))
* **server:** mark session cookies Secure over HTTPS, clear both on logout ([f213cfb](https://github.com/amigo-labs/amigo-krake/commit/f213cfb7e77622001f4423e5dd4dc909b3be3925))
* **server:** no duplicate NZBGet history entries, history delete removes the archive entry ([1d16e73](https://github.com/amigo-labs/amigo-krake/commit/1d16e7377f8db25365d506cdb204f860a0c27b89))
* **server:** persist webhooks to config and keep the dispatcher in sync (SV3) ([c62ec08](https://github.com/amigo-labs/amigo-krake/commit/c62ec0895c3b4dcc65d86e25e9b09738e02c7b50))
* **server:** redact secrets in GET /api/v1/config ([dc8113d](https://github.com/amigo-labs/amigo-krake/commit/dc8113d1aaab5b6392a0abdbb78e1b28fefd4bb4))
* **server:** report live progress of running downloads over REST ([0d826e4](https://github.com/amigo-labs/amigo-krake/commit/0d826e4611844e8ed5b5ef4603cefd0d71065a92))
* **server:** scrub secrets from feedback before submitting ([dfb6a5d](https://github.com/amigo-labs/amigo-krake/commit/dfb6a5dd57b1e537566cea761dac35da44314e2f)), closes [#36](https://github.com/amigo-labs/amigo-krake/issues/36)
* **server:** SSRF guard for webhook + RSS outbound URLs ([517f40d](https://github.com/amigo-labs/amigo-krake/commit/517f40d43871c5a173904498418596abdeb160f1))
* **server:** SSRF-guard download paths, rate-limit login, close IP-block gaps ([2699954](https://github.com/amigo-labs/amigo-krake/commit/2699954db1692a373a594dc109863b371ddfdf42))
* **server:** URL allowlist + body cap on Click'n'Load endpoints ([96d5ac5](https://github.com/amigo-labs/amigo-krake/commit/96d5ac5dea8a11d1ccc33b152b44c74d7fe5cfb0))
* **site:** make `npx wrangler deploy` work from workspace root ([1ea6512](https://github.com/amigo-labs/amigo-krake/commit/1ea65122375251ef4d2fb4a073307b255de9a9d0))
* **site:** make `npx wrangler deploy` work from workspace root ([a72ec46](https://github.com/amigo-labs/amigo-krake/commit/a72ec464ba6e12e6edf9544e59cc8514ef109607))
* **site:** move wrangler.jsonc into site/ for Workers Builds ([bcad54d](https://github.com/amigo-labs/amigo-krake/commit/bcad54d4ac248482e5b110d151d69d56091e04b7))
* **tests:** address Copilot review on PR [#60](https://github.com/amigo-labs/amigo-krake/issues/60) ([1623015](https://github.com/amigo-labs/amigo-krake/commit/16230156aa1e23b26a34a9708f13f39f0b4a771c))
* UI polish — hover states, focus styling, empty states, dead code cleanup ([3ace985](https://github.com/amigo-labs/amigo-krake/commit/3ace98586a0c6bdd561a7d009a93fa24528312ee))
* **ui:** full-height pause icon, muted knob on a switched-off toggle ([f4be71a](https://github.com/amigo-labs/amigo-krake/commit/f4be71a68c771e7d453aad3d218f8734b75b5289))
* **updater:** install release archives and sign them for self-update ([8852168](https://github.com/amigo-labs/amigo-krake/commit/8852168037dfbb9104e28486c0d1d25031fa3a06))
* **updater:** install release archives and sign them for self-update ([72bdffd](https://github.com/amigo-labs/amigo-krake/commit/72bdffdd403c5e251b0ec7956ff3d30c0eb1be42))
* URL decoding corruption, XML parser panics, timer leaks, DnD bug ([c734193](https://github.com/amigo-labs/amigo-krake/commit/c7341938eaea83936f966c2fe2335b44c6609690))
* use cross tool for aarch64 in release-build + add Tauri signing pubkey ([ed9a95a](https://github.com/amigo-labs/amigo-krake/commit/ed9a95a8dcd9d48d5211cc5e864655a67ed3a273))
* verify Ed25519 signature on core self-update ([#30](https://github.com/amigo-labs/amigo-krake/issues/30)) ([94be21f](https://github.com/amigo-labs/amigo-krake/commit/94be21faaf3537b4fffe518c7351a8344547d7b5))
* **web-ui:** add a layer manager and fix the broken drag and captcha trap ([1fce7cd](https://github.com/amigo-labs/amigo-krake/commit/1fce7cdd43ff9f51398d06b486cfcf33ce6cec22))
* **web-ui:** address Copilot review on [#65](https://github.com/amigo-labs/amigo-krake/issues/65) ([cc0d961](https://github.com/amigo-labs/amigo-krake/commit/cc0d961d8a75c374e7d820ec6f641840d54e8a26))
* **web-ui:** cleanup popstate listener via onMount lifecycle ([9088dc7](https://github.com/amigo-labs/amigo-krake/commit/9088dc7bee30b3f2ba292ed69a5670513c7898b3))
* **web-ui:** correct byte formatting units and clamp magnitude index ([d718b59](https://github.com/amigo-labs/amigo-krake/commit/d718b59dc98e17e261107df08ce0aa46a5412a4c))
* **web-ui:** guard shortcuts in inputs, wire reorder + report, i18n, ws proxy ([93ff487](https://github.com/amigo-labs/amigo-krake/commit/93ff4877cbcc2cd64e4b3ea869573614a00e9671))
* **web-ui:** keyboard-operable context menu ([5e2efd8](https://github.com/amigo-labs/amigo-krake/commit/5e2efd8c28af8666517fbac4e9d6775b88db43a7))
* **web-ui:** name the saved file in the completion notification ([115febb](https://github.com/amigo-labs/amigo-krake/commit/115febba8523673c82bd500127063ed89e0c98a3))
* **web-ui:** read WebSocket events from the flat server payload ([45d554a](https://github.com/amigo-labs/amigo-krake/commit/45d554ac4565a691f3470d8d50bf7bbbc42a01b1))
* **web-ui:** readable download rows: file name, failure reason, phone layout ([065d5a7](https://github.com/amigo-labs/amigo-krake/commit/065d5a707babd4f1bf5620b0453b0fe9257b99e0))
* **web-ui:** safe context-menu delete, in-flight action guards, i18n coverage ([95a005d](https://github.com/amigo-labs/amigo-krake/commit/95a005de9151f2dbc48bcc42c67e50d347f26a8e))
* **web-ui:** shortcut hints as key caps, no doubled stats, dependent toggles ([ea5497f](https://github.com/amigo-labs/amigo-krake/commit/ea5497fc465a1eb80aa99fb016abc83fa6ed25ad))
* **web-ui:** show download progress, names and failure reasons; one accent; denser Downloads screen ([6b6a4fb](https://github.com/amigo-labs/amigo-krake/commit/6b6a4fb72ac4a2a4b54f6b49cf90c9101977405f))
* wrap env var mutations in unsafe blocks for Rust 2024 edition ([3463281](https://github.com/amigo-labs/amigo-krake/commit/34632812bd5f2bf2c32b3ea284f9f1b7723e2d91))
* Zip Slip path traversal + tar absolute path extraction ([7f7c112](https://github.com/amigo-labs/amigo-krake/commit/7f7c1126065b9255d23c7a40e18f236bc8dcb869))


### Performance Improvements

* **build:** add tuned release profile (fat LTO, codegen-units=1, strip) ([ef5e3f3](https://github.com/amigo-labs/amigo-krake/commit/ef5e3f35f9ea0371deb9ba352aa0a059550fca4b))
* **build:** set panic = "abort" for the release profile ([f71fb82](https://github.com/amigo-labs/amigo-krake/commit/f71fb8227852a7eb9afe51e23e92978dda339fb0))
* **core,server:** buffer download writes and stream chunk reassembly ([203bb3a](https://github.com/amigo-labs/amigo-krake/commit/203bb3a053e07717c48fbdc592949090b867592e))
* **core:** stream HLS/DASH segments to disk instead of buffering in RAM ([27e32e5](https://github.com/amigo-labs/amigo-krake/commit/27e32e508286af07ece7f1fb76cd9c1a1befc88a))
* **core:** tune SQLite pragmas, cache statements, batch chunk inserts ([74446d7](https://github.com/amigo-labs/amigo-krake/commit/74446d7a2a48eabd5a62a22b432a2482eb523d2c))
* **deps:** trim tokio feature set and drop it from extractors ([12291c5](https://github.com/amigo-labs/amigo-krake/commit/12291c5349779c9bf62358a714969b5e7688ef10))
* **extractors,server:** hoist hot-path regexes to LazyLock statics ([5efda97](https://github.com/amigo-labs/amigo-krake/commit/5efda970fcc4fe1a34a090f12828e506ee3bb97e))
* **extractors:** store cached player JS as Arc&lt;str&gt; ([f39f8d9](https://github.com/amigo-labs/amigo-krake/commit/f39f8d9758a29f1be57c81e74a49014f8a124bf1))
* **plugin-runtime:** cache compiled regexes; harden base64 and JS interop ([3279558](https://github.com/amigo-labs/amigo-krake/commit/32795583270d4b71425098ab231f294a46799cea))
* **plugin-runtime:** cache SWC transpilation and registry URL regexes ([cc91feb](https://github.com/amigo-labs/amigo-krake/commit/cc91feba861f86de82885b28530824c3120c1277))
* **web-ui:** code-split pages and isolate the Svelte vendor chunk ([ad02aad](https://github.com/amigo-labs/amigo-krake/commit/ad02aad8c7af6e4d7940462a58427e8a32b26487))
* whole-repo performance optimization (build, core, extractors, plugins, web-ui) ([bc95f03](https://github.com/amigo-labs/amigo-krake/commit/bc95f03b8698d36460807a08c472223a3b4c8a0a))


### Code Refactoring

* move amigo-named data paths to krake ([1f67687](https://github.com/amigo-labs/amigo-krake/commit/1f67687dae86b4ef9c2defb47869009dc7420aca))
* name the desktop executable Krake ([3f68392](https://github.com/amigo-labs/amigo-krake/commit/3f68392dc041504608c168b70dccb865f81bbe0e))
* rename the AMIGO_* environment variables to KRAKE_* ([98c4d19](https://github.com/amigo-labs/amigo-krake/commit/98c4d1950db200c04ba6d2350dbd837676c28172))
* rename the app to Krake ([5a7f26c](https://github.com/amigo-labs/amigo-krake/commit/5a7f26c14e3dd4db520519b2a31578995b753e4e))
* rename the binaries to krake, krake-server and krake-plugin-host ([b2d1b7b](https://github.com/amigo-labs/amigo-krake/commit/b2d1b7bd6329f090017194fcf5fb00305dc1d86b))
* rename the workspace crates to krake-* ([1aa1559](https://github.com/amigo-labs/amigo-krake/commit/1aa155951ad495926fc08cf1925e90c5888a897d))


### Build System

* move the npm workspace packages to the [@amigo-labs](https://github.com/amigo-labs) scope ([57646c3](https://github.com/amigo-labs/amigo-krake/commit/57646c3f6f6aa006c847e19889b69beecd642e0d))
* publish the container image as ghcr.io/amigo-labs/krake ([3929364](https://github.com/amigo-labs/amigo-krake/commit/3929364c549f23276ee3113d87a5fc939ce818f2))

## Changelog
