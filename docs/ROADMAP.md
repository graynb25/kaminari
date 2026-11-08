# Roadmap

Status: active. Done means the acceptance check passes, not that code exists. Each phase updates `CHANGELOG.md` and `docs/ENGINEERING-NOTES.md` in the same commit.

## P0: Browse and player

Tauri shell with custom title bar. Svelte home with shelves from AniList. Search palette, browse filters, detail page. libmpv player with resume and history. Library add and remove. Image cache with blurhash. Diagnostics export.

Acceptance: on a clean Windows 11 VM with WebView2 present, a tester can search a title, add it to library, play a stream, close the app, reopen, and resume where they left off. Installer under 15MB before libmpv resources. Cold start under 500ms in release, warm cache. Search palette under 50ms local and under 600ms p95 against AniList. Play to first frame under 1500ms p95 on cached source. Every screen shows loading, empty, error, offline. Shortcuts dialog complete. axe reports zero critical.

## P1: Extensions

QuickJS host with 5 second timeout (10 for resolve) and 64MB cap. `.kext` loader, built-in public domain sample, Sources test panel, source browse and detail view. See `docs/PLAN-JS-EXTENSIONS.md` for the full contract. Signed registry and dev tooling follow once the host is reliable.

Status: host core, Sources test panel, and source browse and detail are done. Search, detail, episodes and resolve run live against archive.org. Player handoff is next.

Acceptance: the built-in sample installs, searches archive.org, lists episodes, resolves a stream, and the timeout, memory, host denial and storage tests pass without network access. Legacy Aniyomi metadata import stays as it is: `pkg` to id, `apk` to artifact, `sources[]` to rows with string ids preserved. See `docs/EXTENSION-API.md` for the map.

## P2: Wow polish

Hero carousel with drift and crossfade, shelf physics and glow pass, skeleton shimmer, tray menu, accent picker hooks, light theme check, empty and error copy pass, virtualized grid at 500 items. README screenshots switch from placeholders to real build shots. Keyboard path works without a mouse from library to player and back.

Acceptance: grid holds 60fps on integrated graphics, no dropped streak over 3 frames in the perf harness. Reduced motion build is fully static. Palette to playback GIF in README shows the real app. Bundle still under budget. No `any` crosses IPC.

## P3: Sync and extras

AniList OAuth read first, then write. Offline queue replays without dupes. Server wins conflicts with a toast and a log. Downloads to a user folder where licensed. Auto update from GitHub Releases with Minisign, one click restart. Settings export and import. Backup restores on a fresh PC.

Acceptance: progress syncs after a watch in under 30 seconds on good net. Airplane queue replays clean. Update installs stable to stable with library intact. Signed installer documents SmartScreen behavior. Rollback is reinstall plus intact SQLite file.

## Later

macOS and Linux builds, manga reader bridge, Trakt and MyAnimeList sync, picture in picture, multi window, Tauri mobile. None of this blocks P0. Proposals need a measured reason and a docs update first.
