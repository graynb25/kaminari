# Changelog

Every visible change gets a line here. Keep lines short and specific.

## Unreleased

- Schedule: rename the Later group to Beyond this week, with a note that it is not part of the selected day and a top border that separates it from the day timeline

## 0.29.0 - 2026-11-12

- Schedule: add a Schedule rail view with a next-up card, seven day strip, timeline rows, episode badges, live countdowns, Aired and Later states, and a cached AniList snapshot
- Detail: show the next airing episode and a live countdown on the detail page
- Data: schema version 4 adds the airing_schedule cache. 0.28.5 and older reject a 0.29.0 backup
- Version: bump to 0.29.0

## 0.28.5 - 2026-11-11

- Continue Watching: revert the 0.28.4 duration filter as too strict. Manual watched marks return to the shelf, including zero-duration mark-all rows. Source link timing and the duplicate AniList link repair from 0.28.3 stay unchanged
- Version: bump to 0.28.5

## 0.28.4 - 2026-11-11

- Continue Watching: restore watched rows that have a real playback duration. Zero-duration manual mark-all rows stay out. This fixes 0.28.3 hiding actual progress such as Reincarnated as a Sword
- Version: bump to 0.28.4

## 0.28.3 - 2026-11-11

- Sources: opening a result from AniList Watch no longer adds it to the source library. The source row and durable AniList link are created when playback starts or an episode is marked watched. The Library button still saves immediately
- Sources: keep one source show per AniList id per extension and clear legacy duplicate links on startup, so one season cannot queue progress to another
- Continue Watching: only real playback progress appears. Manual watched marks and zero-position rows stay in Library and History
- Version: bump to 0.28.3

## 0.28.2 - 2026-11-11

- Diagnostics: add a redacted JSON export from Settings, Updates and diagnostics. It contains app and OS versions, schema version, row counts and coarse feature flags, never titles, URLs, paths, tokens, extension IDs or settings values
- Data: add Keep history for 30, 90, 365 days or Forever. Only AniList and source history older than the cutoff are removed; watched marks, library, progress, Continue Watching and extension storage stay
- Data: keep only the newest three `kaminari.db.safety-*` copies on startup and after a restore
- Accessibility: one shared modal focus layer with background isolation, Tab containment, Escape and focus return; dialogs are named by their visible headings
- Accessibility: keyboard paths for the search palette, history range menu and quality menus; live status and error announcements; slider value text; reduced-motion delays and contrast token fixes
- Accessibility: Settings failure statuses use alert roles, and the clear-data dialog and Incognito setting are named from their visible headings
- Version: bump to 0.28.2

## 0.28.1 - 2026-11-11

- Security: extension storage values are encrypted at rest with Windows DPAPI in the current-user scope and stored as `dpapi:v1:<base64>`. Reads decrypt, writes encrypt, and legacy plaintext rows migrate automatically. Malformed or foreign values are treated as missing instead of being returned as ciphertext or encrypted twice
- Privacy: Settings, Privacy adds a Data at rest section. It explains that library, history and source rows remain plaintext and rely on Windows disk encryption, while extension storage is DPAPI-encrypted
- Data: add Clear all local data with confirmation. It removes the database and sidecars, safety, quarantine and legacy `.bak` copies, local settings and WebView data, keeps downloads, exported backups and the AniList token, and finishes locked files on the next start
- Compatibility: schema version 3 marks the DPAPI extension-storage format. Backups made by 0.28.1 are rejected by 0.28.0 and older, and older builds cannot read the encrypted extension values. Keep a backup before downgrading
- Version: bump to 0.28.1

## 0.28.0 - 2026-11-10

- NSFW: installed sources expose their manifest NSFW flag, show a badge, and are hidden from Sources and source search by default. Settings, Privacy adds a Show NSFW sources toggle
- Sources: add a durable AniList link on source library rows. A source opened from Watch keeps its AniList identity after restart, and Continue Watching merges by ID with the source row as the resume target
- Sources: install `.kext` packages through a native file picker instead of a free-text path
- Security: settings, backup and package paths use one-use dialog-scoped tokens, canonical paths and reparse-point checks
- Security: registry, Mangayomi and download fetches share a safe resolver with HTTPS enforcement, private-address rejection, redirect checks, DNS pinning and streaming caps
- Security: upgrade rusqlite to 0.41.1 and bundled SQLite to 3.54.1, clearing RUSTSEC-2026-0459
- Security: extension updates require an explicit review when the name, version, code or host list changes. Trust is computed in Rust, not supplied by a package or backup
- Security: proxy requests reuse shared entries, index children for constant-time reuse, reject over-budget playlists and cap rendered playlists at 4 MiB
- Security: host allowlists enforce path boundaries, ports, encoded separator rejection and wildcard subdomains
- Security: source removal deletes its library, progress and history rows. Episode counts, source metadata and duration values are capped
- Sync: one guard serializes AniList push and pull, queue batches are bounded, failures use exponential backoff and negative local actions no longer create unexpected AniList writes
- Portable: sign in can register the kaminari callback only after an explicit user action, with a visible reason when registration is unavailable
- CI: actions are pinned to commit SHAs, gitleaks is checksum verified, permissions are read-only and the workflow runs frontend tests and dependency audits
- Privacy: Incognito no longer writes recent searches, automatic background AniList sync pauses while it is on, and Settings, Privacy adds Clear WebView data. The privacy policy now discloses updater traffic, cache behavior and Incognito limits
- Security: downloads claim final and part files with `create_new` and random names, reject Windows reserved names and reparse points, and hard-cap concurrent downloads at 8
- Security: settings and backup writes use one shared `create_new` temporary file and an atomic replace that never deletes the target first
- Security: the CSP allows `worker-src 'self' blob:` so hls.js can create its worker in release builds
- Data: the `continue_visible` migration and backfill run in one transaction with a repair marker, so an interrupted upgrade cannot permanently hide old Continue rows
- Startup: a database or setup failure shows a native recovery dialog with Retry, Open data folder and Quarantine and start fresh instead of a panic. Quarantine moves the database and its sidecars aside without deleting them
- Maintenance: remove dev harnesses and unused API wrappers. Legacy Aniyomi registry and source commands are compiled only in debug builds and are absent from release builds
- Compatibility: schema version 2 adds the source AniList link. Backups from this build are rejected by older builds
- Version: bump to 0.28.0

## 0.27.0 - 2026-11-10

- Security: extension searches, details, episodes, preferences and stream resolve run off the Tauri main thread, with a per-source concurrency limit. A slow source no longer freezes the window
- Security: `html.select` caps nodes, text, HTML, attributes and total output, with UTF-8 safe truncation and deadline checks
- Security: external mpv accepts only absolute http or https URLs without credentials. Option-like and malformed playback or subtitle URLs are rejected before mpv starts
- Security: AniList sign in uses a one-time state value with a ten minute expiry and an exact callback match
- Security: backup restore validates the file with `integrity_check` and schema checks, makes a timestamped safety copy, restores through a scratch database and disables restored extensions until you review and re-enable them
- Security: the updater requires the signed version in the Minisign trusted comment. The release docs cover offline key custody and asset verification
- Version: bump to 0.27.0

## 0.26.9 - 2026-11-10

- Continue Watching: merge AniList and source rows through a normalized title matcher so the same show is listed once; the source row wins for resume and rewatch
- Version: bump to 0.26.9

## 0.26.8 - 2026-11-10

- Installer: set the Kaminari icon on the NSIS setup and uninstaller executables instead of the default NSIS icon
- Version: bump to 0.26.8

## 0.26.7 - 2026-11-10

- Player: add a gear menu with stream quality options for the current episode. Switching keeps the current position
- Settings and the source browser: replace the native quality dropdowns with an in-app quality menu so every option stays readable in dark and light themes
- Incognito: add a dedicated Settings, Privacy section. Playback skips source progress, history, Continue Watching and AniList updates while on; the player and source page show a badge. Manual watched marks are blocked
- History: compact toolbar with a time range filter (last hour, today, today and yesterday, all time), per-row delete and an in-app clear confirmation. Clearing history also empties Continue Watching
- Continue Watching: completed AniList shows and watched source shows return after re-marking, with a Rewatch label for source rows
- Browse: add a Coming Soon shelf under Trending for unreleased titles. Upcoming cards show UPCOMING and Plan, which saves them to the library as Planning
- Version: bump to 0.26.7

## 0.26.6 - 2026-11-10

- Security: update the zip crate to 9.5.0 and handle its fallible entry names. This clears the critical zip parser and extraction advisories for `.kext` package loading
- Tooling: update Vite to 6.4.6 and Vitest to 5.1.0. This removes the vulnerable Vite 5 copy under Vitest and clears the Vitest, @vitest/mocker and tinypool advisories
- Version: bump to 0.26.6

## 0.26.5 - 2026-11-10

- Source episodes: add Mark all watched and a watched toggle on every episode, so existing progress can be entered without replaying a full episode. Mark all also queues AniList progress when the source show came from Watch; Clear resets local source progress only
- Source library: progress changes from the source browser refresh the AniList shelves immediately
- Docs: cover the new controls and record the 40 episode host check
- Version: bump to 0.26.5

## 0.26.4 - 2026-11-10

- Sources: remove the Aniyomi registry panel until an APK runtime exists. The page now shows native extensions and Mangayomi sources only, with stat cards for native extensions, Mangayomi sources, allowed hosts and runtime memory
- Onboarding: add a first-run Getting started checklist for adding a source, finding a title, and watching. It is dismissible and reopens from Settings, About
- Docs: update the install guide, README, design notes and privacy page for the hidden registry and the new checklist
- Version: bump to 0.26.4

## 0.26.3 - 2026-11-10

- Updates: fix the Windows update flow. The app now closes, lets the NSIS installer replace the old exe, and restarts itself; the old build showed a Restart button while the installer was still writing
- Docs: add `docs/INSTALL.md` for the installer, portable zip, first run, adding a source without a bundled source list, and troubleshooting
- About: describe Kaminari as a Windows anime player, not an Aniyomi client
- CI: keep the Tauri resource glob valid in a clean checkout with a tracked `src-tauri/lib/README.md`, and remove the tag-triggered release workflow that could not ship the local libmpv payload or sign with the local key
- Security: the repository is public; branch protection, secret scanning, push protection, Dependabot and private vulnerability reporting are enabled
- Version: bump to 0.26.3

## 0.26.2 - 2026-11-10

- Playback: autoplay next episode with a five second countdown, a Play next button, and a Settings toggle
- Playback: source shows opened from an AniList Watch handoff queue finished episodes to AniList progress; manual marks already did
- Detail: More like this recommendations from AniList, clickable into the same detail flow
- Sources: per-source settings for Mangayomi sources, including AniKoto episode thumbnails; source series art is no longer shown as an episode thumbnail
- Sources: boolean source preferences round-trip correctly instead of returning the truthy string false
- Packaging: portable Windows zip beside the NSIS installer
- Docs: Kaminari is described as a desktop anime app, not an Aniyomi client; the Aniyomi registry remains metadata only
- Version: bump to 0.26.2

## 0.26.1 - 2026-11-10

- Playback: keep up to 4096 HLS child URLs per stream and allow 20000 playlist lines, so long VOD episodes no longer evict their first segments and stall at the first missing child
- Playback: add a regression test for a 283 segment, 23 minute episode
- Browse: Watch on an AniList title now searches installed sources, shows the closest matches, and opens the chosen result in the source browser
- Updates: a missing public release feed is shown as a neutral local-build state instead of a red failure
- Version: bump to 0.26.1

## 0.26.0 - 2026-11-10

- Extensions: install a raw Mangayomi JavaScript anime source from a URL, inspect its metadata and SHA-256, then confirm a third party trust prompt
- Extensions: Promise and async/await support in the QuickJS host, with a compatibility bootstrap for `MProvider`, `Client`, `SharedPreferences`, `Document`, crypto, timers, base64, UTF-8, URL and console
- Extensions: map Mangayomi search, detail, chapters and video lists onto the native search, detail, episodes and resolve operations
- Extensions: compat sources get an explicit any-HTTPS host grant while plain HTTP, localhost, private addresses and URL credentials stay blocked
- Playback: remote subtitle files are proxied with the stream headers; inline SRT/WebVTT is converted to WebVTT and loaded from a Blob track
- Playback: compat episodes are sorted chronologically
- Playback: preferred quality setting and a show page selector pick the closest stream in the same server and audio variant, with Auto, Best, 4K, 1080p, 720p, 480p and 360p choices
- Extensions: AniKoto and HiAnime verified live against the source browser and player
- Updates: Tauri updater plugin, signed updater artifacts and Settings check, download, install and restart states
- Build: signed updater artifacts need `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; the GitHub feed stays empty until a signed public `latest.json` exists
- Version: bump to 0.26.0

## 0.25.0 - 2026-11-10

- Downloads: direct MP4/OGV and simple HLS streams save to a chosen folder with progress, cancel and a one-time content-rights confirmation
- Downloads: master playlists pick the highest-bandwidth variant; encrypted HLS, separate audio and byte-range playlists are rejected with clear errors
- Build: downloads run in Rust with `.part` files and collision-safe names
- Version: bump to 0.25.0

## 0.24.0 - 2026-11-09

- AniList: offline status and progress queue with retry and server-wins conflict handling
- AniList: background push every 30 seconds, pending count and Sync now in Settings
- Version: bump to 0.24.0

## 0.23.0 - 2026-11-09

- AniList: pull the viewer's anime list into the local library with mapped statuses and watched progress
- AniList: idempotent merge keeps local-only rows, keeps longer local progress and never creates fake history
- Version: bump to 0.23.0

## 0.22.0 - 2026-11-09

- AniList: connect through the implicit grant, deep link callback, Viewer validation and OS keychain storage
- AniList: account block in Settings with viewer name, connect and disconnect
- Build: deep-link, single-instance and opener plugins, with the `kaminari://` scheme registered by the installer
- Version: bump to 0.22.0

## 0.21.0 - 2026-11-09

- Settings: export and import theme, accent and proxy settings as JSON
- Data: backup and restore the SQLite library with a restart prompt
- Build: native file dialogs through `tauri-plugin-dialog`
- Version: bump to 0.21.0

## 0.20.0 - 2026-11-09

- Settings: Violet, Ocean, Forest, Ember and Rose accent presets, persisted locally
- Styles: accent tokens for dark and light themes, including poster, source and player surfaces
- Docs: accent picker notes and refreshed screenshots for 0.20.0
- Version: bump to 0.20.0

## 0.19.0 - 2026-11-09

- Player: bundle `libmpv-2.dll` and `libmpv-wrapper.dll` in the installer, opt-in behind `kaminari.libmpv`
- Build: `pnpm check:libmpv` verifies the pinned DLL hashes before a Tauri bundle
- Licenses: Native libraries group now bundled, with versions, hashes, source URLs, texts and the source offer
- Docs: lawyer review draft, P1 packaging checklist and README build notes
- Dev: 500-item grid perf harness at `?perfHarness=1`, gate passes without virtualization
- Docs: palette-to-playback GIF from the real 0.19.0 build
- Version: bump to 0.19.0

## 0.18.1 - 2026-11-09

- Keyboard: Escape in the player closes only the player, handled keys stay scoped, and focus returns to the episode and source card
- Library: source cards show a focus ring that the card overflow was clipping
- Version: bump to 0.18.1

## 0.18.0 - 2026-11-09

- App: system tray icon with Show Kaminari and Quit, left click toggles the main window
- Docs: real build screenshots in `assets/screenshots/`, plus accurate player, extension and release claims in the README
- Version: bump to 0.18.0

## 0.17.1 - 2026-11-09

- Styles: base and component CSS moved into cascade layers, with 36 deliberate overrides and reduced motion kept unlayered
- Styles: utilities now win where the markup asks, so the online pill hides below 1024, long stat values truncate and buttons use their declared text size
- Settings: row values wrap at word boundaries instead of mid word
- Version: bump to 0.17.1

## 0.17.0 - 2026-11-09

- Settings: sticky anchor rail at 1024 and wider, chips below, active section tracking and scroll margins
- Settings: responsive 2 by 2 stat cards and single-column cards below 1280
- App: FatalState for boot snapshot failure and view crashes, with redacted diagnostics and retry
- App: fatal diagnostics redact bare token assignments outside URLs
- Version: bump to 0.17.0

## 0.16.2 - 2026-11-09

- Licenses: FFmpeg commit `cc583787b` verified from the winbuild job log, Native group names the exact source
- Docs: native license package marked complete; lawyer sign-off remains the only legal gate
- Version: bump to 0.16.2

## 0.16.1 - 2026-11-09

- Licenses: native libmpv license package with artifact hashes, texts and source offer, marked not bundled
- Dev: embedded libmpv P0 spike behind the default-off `kaminari.libmpv` flag, with a dev harness and gitignored DLLs
- Dev: libmpv requires the loopback proxy for headers and rejects comma header values
- Dev: proxy scope stays MP4 and HLS; DASH and embed with headers point to external mpv
- Docs: P1 readiness checklist and route-scoped transparency decision
- Version: bump to 0.16.1

## 0.16.0 - 2026-11-08

- UI: shared Modal with focus trap, Escape, focus restore and one overlay lock for legal, licenses, shortcuts and palette
- UI: Callout and ProgressBar atoms across offline, library, sources, search, resolve and detail progress
- UI: corrected double-encoded middle dots and palette arrow keys
- UI: global shortcuts stay blocked while any overlay is open
- Docs: embedded libmpv plan with licensing, log redaction and z-order gates
- Version: bump to 0.16.0

## 0.15.0 - 2026-11-08

- Player: per-entry cookie jar for protected HLS, capturing Set-Cookie on playlists, keys, maps, segments and redirects
- Player: live playlists refetch while child ids stay stable across sliding windows
- Docs: extension template with a synchronous source skeleton, packaging and install steps
- Version: bump to 0.15.0

## 0.14.0 - 2026-11-08

- Player: HLS master playlists, variant switching, audio and subtitle groups, session keys and data
- Proxy: every child URL is checked against the extension host allowlist, with a depth cap of 4
- UI: BrandMark across the title bar, footer, empty state and Settings
- UI: staggered library and source grids, with wrappers that keep hover and disabled opacity
- UI: compact static Storm and Daylight theme preview
- UI: accessible SegmentedControl with Arrow, Home and End focus, used for legal tabs
- Version: bump to 0.14.0

## 0.13.0 - 2026-11-08

- Player: HLS media playlists and segments proxy through loopback, including keys, init maps and byte ranges
- Proxy: separate byte-range children on the same URL, playlist range wins
- Artwork: shimmer, blur-up, cached image checks and broken image fallbacks across posters, cards, hero and detail
- Artwork: broken hero and detail banners fall back to the blurred cover or void tile
- Artwork: shimmer runs only near the viewport, so long shelves do not animate offscreen
- Version: bump to 0.13.0

## 0.12.0 - 2026-11-08

- Player: loopback proxy for header protected MP4, enabled from Settings and off by default
- Proxy: CORS headers for WebView fetch and HLS preflight
- Settings: Playback card toggles the loopback proxy and shows the port or Unavailable
- UI: shared page header atom for library, history, sources and settings, with a soft accent glow
- UI: legal and license dialogs fade and rise with the overlay instead of popping open
- Version: bump to 0.12.0

## 0.11.4 - 2026-11-08

- UI: theme-aware accent and status surfaces replace the hard-coded colors that drifted in Daylight
- UI: glass dialogs share one material with border, inset highlight and shadow
- UI: poster cards get a 2px focus ring, press feedback, and quick actions that stay visible on touch
- UI: keyboard focus starts in the content instead of the maximize control
- UI: source grids reach four columns at 1280 and wider
- Type: paragraph wrapping, optical sizing and tabular numerals
- Docs: Papyrus styling review distilled into `docs/PLAN-UI-POLISH.md`, phase 1 shipped
- Version: bump to 0.11.4

## 0.11.3 - 2026-11-08

- Source episodes: 96 by 56 thumbnail with an EP number badge, using the show banner or cover when the source has no episode image
- Docs: Suwayomi/anime server review, native-first source decision, loopback stream proxy and embedded libmpv next
- Version: bump to 0.11.3

## 0.11.2 - 2026-11-08

- Licenses: render the dialog at app root so it stays centered and fully visible on small windows
- Tests: 15 Vitest cases covering titles, covers, HTML cleanup, scores, rate limit copy and relative time
- Version: bump to 0.11.2

## 0.11.1 - 2026-11-08

- Cards: finished titles show a COMPLETED badge in amber with a check, matching the AIRING badge shape
- Version: bump to 0.11.1

## 0.11.0 - 2026-11-08

- Versioning: root `VERSION` is the single source; prebuild and pretauri sync `package.json` and `Cargo.toml` before the build
- Versioning: Tauri reads the package version, the frontend gets `__APP_VERSION__` from Vite, and Rust uses `CARGO_PKG_VERSION`
- Licenses: Settings card and footer link open a generated list of every package in the build, with search, groups, sources and full license texts
- Licenses: `pnpm licenses:build` regenerates `public/licenses.json` from `Cargo.lock` and the installed node_modules
- Version: bump to 0.11.0

## 0.10.3 - 2026-11-08

- Footer: persistent footer with brand mark, no-content notice, AniList credit, Terms, Privacy and version
- Footer: removed the old 0.7.0 home footer line now that the global footer exists
- Legal: in-app Terms of Service and Privacy Policy dialog with tabs, draft notice, focus trap and Escape close
- Legal: docs live in `docs/TERMS.md` and `docs/PRIVACY.md`, linked from the README
- Version: bump to 0.10.3

## 0.10.2 - 2026-11-08

- Cards: titles without an AniList score show NR instead of an empty score slot
- Cards: NR carries a tooltip and keeps the same badge position as a score
- Version: bump to 0.10.2

## 0.10.1 - 2026-11-08

- Icon: new squircle tile with a beveled volt bolt, deep indigo gradient, soft glow and rim light
- Icon: generated the full Windows, macOS, iOS and Android set from `assets/brand/icon.svg`
- Source episodes: the primary action is now Play, Resume or Rewatch instead of Resolve
- Source episodes: Play resolves and starts the first stream in one click, while Streams opens the full quality list
- Version: bump to 0.10.1

## 0.10.0 - 2026-11-08

- Themes: System, Dark and Light modes, persisted locally and initialized before the first paint
- Light: porcelain page, ink type, soft indigo shadows, airy hero and detail scrims, and a light title bar, nav rail, cards, palette and dialogs
- Themes: System follows the Windows light or dark preference live
- Appearance: theme picker in Settings with clear active state and keyboard access
- Media: poster badges, source cards and the player keep their dark surfaces in both themes so artwork stays legible
- Settings: corrected the version cards and removed the stale Appearance placeholder row
- Version: bump to 0.10.0

## 0.9.1 - 2026-11-08

- Continue Watching: clicking a source card now resolves that episode and resumes at the saved position
- Library: a source Resume button now starts playback instead of only opening the show
- Source library: cards carry the last episode id so resume can skip the detail step
- Version: bump to 0.9.1

## 0.9.0 - 2026-11-08

- Continue Watching: source runs join the shelf with cover, extension, resume position and a progress strip
- Continue Watching: the shelf now appears when only source progress exists, not only after saving an AniList title
- History: AniList watched events and source episode progress merge into one timeline, sorted by day
- History: source rows show In progress or Watched and reopen the show in the source browser
- Source continue: one card per show, using the episode with the newest progress
- Version: bump to 0.9.0

## 0.8.2 - 2026-11-08

- Hero: only trending titles with a real banner feed the carousel, so every slide is a clear full-bleed image with the slow zoom
- Hero: titles without a banner stay in the shelves; the poster fallback only appears if fewer than three banners exist
- Version: bump to 0.8.2

## 0.8.1 - 2026-11-08

- Hero: when AniList has no banner, show a clear poster card over the blurred cover instead of a fully blurred header
- Version: bump to 0.8.1

## 0.8.0 - 2026-11-08

- Visual pass: bundled Sora, Inter and JetBrains Mono, layered atmosphere, glass title bar, palette, player, toasts and dialogs
- Shell: richer title bar, nav rail active pill, page transitions, skip link, offline and error states
- Home: cinematic hero with score and meta chips, stronger scrims, rank rail spacing, continue watching and library preview polish
- Library and history: collection stats, source runs, grouped history, loading and error states
- Sources and source browser: registry and extension cards, installed toggles, searchable source results, stream rows
- Detail and player: status menu, score ring, episode progress card, glass player controls, fill, subtitles, diagnostics and keyboard coverage
- Accessibility: focus traps, restored focus, reduced motion, 44px touch targets in the player
- Version: bump to 0.8.0

## 0.7.0 - 2026-11-08

- Source library: add a JavaScript source show from the source browser, see it under Library, resume it later
- Playback progress: the in-app player saves position every 5 seconds and on pause, close and end; resume starts at the saved position; 90 percent marks the episode watched
- Source progress: per episode bars and Resume or Watched labels in the source detail view
- Database: `source_library` and `source_progress` tables
- Version: bump to 0.7.0

## 0.6.0 - 2026-11-08

- Player: in-app video surface with custom controls, keyboard shortcuts, speed, volume, fullscreen, subtitles and autoplay
- Player: HLS through a lazy-loaded hls.js chunk, MP4 and WebM through the WebView
- CSP: allow `media-src https: blob:` and `connect-src https: blob:` so playback is not blocked
- External mpv stays as the fallback for formats the WebView cannot decode
- Version: bump to 0.6.0

## 0.5.0 - 2026-11-08

- Player: external mpv handoff with headers, subtitles, start time and one process at a time
- Source browser: Play on a resolved stream opens mpv, Copy stays beside it
- Settings: live mpv discovery and version, with the KAMINARI_MPV override
- Version: bump to 0.5.0

## 0.4.0 - 2026-11-08

- Native source browser: search a JavaScript source, open a show, list episodes, resolve streams, and copy a stream URL
- Commands: `js_extension_detail`, `js_extension_episodes`, `js_extension_resolve`
- Sample: clean archive.org descriptions, nicer episode titles, duration parsing
- Version: bump to 0.4.0

## 0.3.0 - 2026-11-08

- Native JS extension host: one QuickJS runtime per call, 5s and 10s timeouts, 64MB memory cap, HTTPS host allowlist, blocking fetch, HTML selectors, SHA-256, SQLite storage and a logger
- Sample: Public Domain Animation reads archive.org, and a live test search returned real public domain rows
- Sources: native extension list, install sample, install a `.kext` path, remove, test search, host and trust display
- Library: status filters with counts, title filter, per-card status menu (watching, planning, completed, paused, dropped)
- History: new view (key 3) with recent watched episodes, relative times, and a link back to the series
- Fix: history rows order correctly when two events land in the same second
- Version: bump to 0.3.0

## 0.2.0 - 2026-11-08

- Project scaffold: README, CONTRIBUTING, LICENSE, SECURITY, and docs stubs for Kaminari
- CI: run Rust gates inside `src-tauri`, pin pnpm 10 with a committed lockfile, add Tauri build script and app icons
- App: wire SQLite migrations on first run, validate source ids, expose host limits in the boot snapshot
- AniList: live trending, search and detail commands with a token bucket and typed errors
- UI: real poster grid, hero backdrop, loading skeletons, error and empty states, frameless title bar with window controls
- Library: SQLite backed add and remove, your own shelf, hover action on posters
- Detail: banner, poster, genres, synopsis, episode grid, details sidebar, back path
- Frontend: cinematic app shell with TitleBar, NavRail, HeroCarousel, shelves, palette, toasts, shortcuts dialog, all per DESIGN-SPEC
- Polish: centered content column, hidden shelf scrollbars, coherent SVG chrome, sibling quick actions, responsive hero, first-run callout, ambient depth, palette focus trap
- Browse: 1440 content width, five parallel rails (trending page 1 and 2, airing, top, popular), Top 10 Today ranked rail, per rail retry, edge fade affordance
- Fix: shelf rows reset horizontal scroll when data lands, rank numerals scale down, top three ranks carry the volt accent
- Fix: AniList scores display on a 10 point scale with one decimal, with the raw 0 to 100 value in a tooltip
- Progress: episode watched toggles, mark all and clear, per series progress bars, continue watching rail, library watched counts
- Sources: live Aniyomi registry reader, 250 packages from the yuzono index, metadata only install into SQLite, NSFW and torrent badges
