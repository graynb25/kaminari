# Changelog

Every visible change gets a line here. Keep lines short and specific.

## Unreleased

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
