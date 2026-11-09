# Changelog

Every visible change gets a line here. Keep lines short and specific.

## Unreleased

## 0.17.0 - 2026-11-09

- Settings: sticky anchor rail at 1024 and wider, chips below, active section tracking and scroll margins
- Settings: responsive 2 by 2 stat cards and single-column cards below 1280
- App: FatalState for boot snapshot failure and view crashes, with redacted diagnostics and retry
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
