# Changelog

Every visible change gets a line here. Keep lines short and specific.

## Unreleased

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
