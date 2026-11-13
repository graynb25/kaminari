# Roadmap

Status: active. Done means the acceptance check passes, not that code exists. Each phase updates `CHANGELOG.md` and `docs/ENGINEERING-NOTES.md` in the same commit.

## P0: Browse and player

Tauri shell with custom title bar. Svelte home with shelves from AniList. Search palette, browse filters, detail page. libmpv player with resume and history. Library add and remove. Image cache with blurhash. Diagnostics export.

Acceptance: on a clean Windows 11 VM with WebView2 present, a tester can search a title, add it to library, play a stream, close the app, reopen, and resume where they left off. Installer under 15MB before libmpv resources. Cold start under 500ms in release, warm cache. Search palette under 50ms local and under 600ms p95 against AniList. Play to first frame under 1500ms p95 on cached source. Every screen shows loading, empty, error, offline. Shortcuts dialog complete. axe reports zero critical.

## P1: Extensions

QuickJS host with 5 second timeout (10 for resolve) and 64MB cap. `.kext` loader, built-in public domain sample, Sources test panel, source browse and detail view. See `docs/PLAN-JS-EXTENSIONS.md` for the full contract. Signed registry and dev tooling follow once the host is reliable.

Status: host core, Sources test panel, source browse and detail, the in-app player, the source library with resume progress, source Continue Watching and merged history are done. Search, detail, episodes, resolve, playback and resume run live against archive.org. External mpv is the fallback. The loopback proxy P0 for header protected MP4, P1 media playlists, P2 master playlists and P3 cookie jar with live refresh are done. The embedded libmpv P0 spike passed z-order, HEVC and fallback on Windows. P1 packaging is enabled with the libmpv DLLs bundled from a gitignored payload, behind the default-off `kaminari.libmpv` flag. Public distribution waits for counsel review of `docs/LEGAL-REVIEW-DRAFT.md`. The VM clean install, update and rollback checks remain.

Decision: Suwayomi is manga-only and cannot run Aniyomi/yuzono anime extensions. Native `.kext` through QuickJS remains the only bundled provider model. The proxy now handles media and master playlists, keys, init maps, byte ranges, audio and subtitle groups, allowlisted cross-origin children, a per-entry cookie jar and live refreshes. Embedded libmpv is bundled on Windows; the default path is still WebView plus external mpv and the flag is off. P1 has the LGPL artifact verification, license texts and source offer, route-scoped transparency, and the proxy header path. Local packaging is enabled; installer size and VM tests remain. A user-installed anime server adapter can follow behind an experimental flag. No APK runtime or source list is bundled. A 2026-11-08 review found Miru is now Hayase, a BUSL-1.1 torrent client with no API, and Mangayomi is an Apache-2.0 Flutter client with no API. Neither is a server. Mangayomi's QuickJS anime extension format is the first optional adapter candidate, ahead of `miwayomi`, but it is unsigned and would need a trust prompt.

Acceptance: the built-in sample installs, searches archive.org, lists episodes, resolves a stream, and the timeout, memory, host denial and storage tests pass without network access. Legacy Aniyomi metadata import stays as it is: `pkg` to id, `apk` to artifact, `sources[]` to rows with string ids preserved. See `docs/EXTENSION-API.md` for the map.

## P2: Wow polish

Hero carousel with drift and crossfade, shelf physics and glow pass, skeleton shimmer, tray menu, accent picker hooks, empty and error copy pass, virtualized grid at 500 items. README screenshots switch from placeholders to real build shots. Keyboard path works without a mouse from library to player and back.

Status: the light theme landed in 0.10.0 with System, Dark and Light modes. The tray menu landed in 0.18.0. The Library to player to Library keyboard path is verified and fixed in 0.18.1. The 500-item grid harness holds 169 to 180 fps, or 167 fps with software rendering, so virtualization is deferred. The palette-to-playback GIF is in the README. Accent presets landed in 0.20.0. All planned P2 items are shipped; the integrated GPU laptop check remains. The README now uses real build screenshots in `assets/screenshots/`. A 2026-11-08 Papyrus styling review is distilled in `docs/PLAN-UI-POLISH.md`: token discipline, artwork lifecycle, page header and modal atoms, feedback parity, density. It recommends refinement, not a redesign. The UI polish plan is shipped through phase 3.

Acceptance: grid holds 60fps on integrated graphics, no dropped streak over 3 frames in the perf harness. Reduced motion build is fully static. Palette to playback GIF in README shows the real app. Bundle still under budget. No `any` crosses IPC.

## P3: Sync and extras

AniList OAuth read first, then write. Offline queue replays without dupes. Server wins conflicts with a toast and a log. Downloads to a user folder where licensed. Auto update from GitHub Releases with Minisign, one click restart. Settings export and import. Backup restores on a fresh PC.

Status: settings export and import plus SQLite backup and restore landed in 0.21.0. AniList OAuth connect landed in 0.22.0 with client ID `56484`, the `kaminari://auth/callback` deep link and keychain storage. Read sync landed in 0.23.0. The offline write queue and conflict handling landed in 0.24.0 with synthetic tests only. Stream downloads landed in 0.25.0 for direct MP4/OGV and simple HLS. Auto update and the Mangayomi-style JS extension shim landed in 0.26.0. AniKoto and HiAnime were tested live through search, detail, episodes, resolve and playback, including remote and inline subtitles. 0.26.1 fixed long HLS playlists evicting their first segments, added a Watch handoff from AniList titles to installed sources, and made the missing public update feed a neutral local-build state. 0.26.2 added autoplay and next episode, More like this AniList recommendations, per-source settings with real episode thumbnails, AniList progress from linked source playback, and a portable zip. The shim is the core feature; downloads and updates are infrastructure. The 0.29.0 local candidate adds an airing schedule with a next-up countdown, a cached weekly timeline, Aired and Later states, and a detail-page next-episode line. A signed public update feed, the P1 VM checks and counsel review remain.

Acceptance: progress syncs after a watch in under 30 seconds on good net. Airplane queue replays clean. Update installs stable to stable with library intact. Signed installer documents SmartScreen behavior. Rollback is reinstall plus intact SQLite file.

## Later

macOS and Linux builds, manga reader bridge, Trakt and MyAnimeList sync, picture in picture, multi window, Tauri mobile. None of this blocks P0. Proposals need a measured reason and a docs update first.
