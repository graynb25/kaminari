# Changelog

Every visible change gets a line here. Keep lines short and specific.

## Unreleased

- Project scaffold: README, AGENTS, CONTRIBUTING, LICENSE, SECURITY, and docs stubs for Kaminari
- CI: run Rust gates inside `src-tauri`, pin pnpm 10 with a committed lockfile, add Tauri build script and app icons
- App: wire SQLite migrations on first run, validate source ids, expose host limits in the boot snapshot
- AniList: live trending, search and detail commands with a token bucket and typed errors
- UI: real poster grid, hero backdrop, loading skeletons, error and empty states, frameless title bar with window controls
- Library: SQLite backed add and remove, your own shelf, hover action on posters
- Detail: banner, poster, genres, synopsis, episode grid, details sidebar, back path
