# Kaminari

Anime desktop app for Windows. AniList for discovery, sources you install for playback.

A desktop anime app for Windows first, with care in every pixel. Fast startup. Keyboard first. A library you own in one SQLite file. AniList for data. Extensions you choose for more. No content ships in the box.

> Ships no content, no source list. It talks to AniList for data, and to extensions you choose to install for more. That is the whole idea.

Kaminari is not an Aniyomi client. Aniyomi runs Android APK extensions and needs Android. Kaminari runs on Windows, reads AniList, and plays user-installed Mangayomi JavaScript sources through QuickJS. The Aniyomi registry is shown for metadata only, and APKs are never downloaded or executed.

![status](https://img.shields.io/badge/status-design--spec-blue) ![tauri](https://img.shields.io/badge/tauri-2.x-teal) ![svelte](https://img.shields.io/badge/svelte-5-orange) ![license](https://img.shields.io/badge/license-MIT-green) ![windows](https://img.shields.io/badge/windows-first-0078D6)

| Library | Search | Player | Extensions |
|---|---|---|---|
| Watching and planning with progress and history | Ctrl K palette, live plus cached offline | In-app MP4 and HLS, optional embedded libmpv, external mpv fallback, J L K keys, diagnostics with D | Sandboxed QuickJS, local .kext, explicit host allowlists |

Screenshots are captured from the current Windows build in the Storm theme and live in `assets/screenshots/`. Every PR that changes UI updates one screenshot.

| Browse | Library |
|---|---|
| ![Browse](assets/screenshots/browse.webp) | ![Library](assets/screenshots/library.webp) |
| **Detail** | **Player** |
| ![Detail](assets/screenshots/detail.webp) | ![Player](assets/screenshots/player.webp) |
| **Search palette** | **Sources** |
| ![Search palette](assets/screenshots/palette.webp) | ![Sources](assets/screenshots/sources.webp) |

Palette to playback, captured from the same build:

![Palette to playback](assets/screenshots/palette-to-playback.gif)

## What it is

- **Core:** Tauri 2 plus Rust for shell, networking, database, extension host, player bridge
- **Interface:** Svelte 5 with Runes plus TypeScript plus Tailwind CSS 4
- **Data:** AniList GraphQL for titles, posters, scores, tracking
- **Player:** in-app WebView playback for MP4 and HLS, with external mpv as the fallback for formats the WebView cannot play
- **Extensions:** sandboxed JS modules through QuickJS, local `.kext` packages, Aniyomi registry metadata

## Why this stack

Tauri keeps the installer small and startup fast. Rust keeps the heavy parts honest. Network, storage, and trust live there. Svelte keeps the interface light and easy to shape by hand. The in-app player covers MP4 and HLS; mpv handles the formats a web player will not. QuickJS lets authors use plain JavaScript without writing Rust.

If you already know React, the Rust side does not care. The contract stays the same.

## Status

Working today: cinematic browse with five AniList rails, Top 10 Today, Ctrl K palette, detail pages, a local AniList library, watched episode tracking, Continue Watching, an Aniyomi registry reader that stores source metadata, a native JavaScript extension host with a source browser, details, episodes and stream resolve, third-party Mangayomi anime sources behind an inspect and trust prompt, a Watch handoff that finds an AniList title in the installed sources, More like this AniList recommendations, per-source settings and per-episode thumbnails, an in-app player for MP4 and HLS with remote and inline subtitles, preferred playback quality and autoplay next episode, a loopback stream proxy for headers, cookies, HLS media and master playlists, stream downloads for direct MP4/OGV and simple HLS, a source library with resumable playback progress, source Continue Watching and merged history, System, Dark and Light themes with five accent presets, AniList account connect with keychain token storage and anime list pull, an offline AniList status and progress queue with conflict handling, settings export and import, a portable SQLite library backup and restore, a Windows tray menu with show and quit, signed auto update wiring, and an in-app license list generated from the build.

Mangayomi sources are added by URL on the Sources page. Kaminari downloads the JavaScript, shows the source metadata and SHA-256, and asks for explicit trust before storing it. It ships no source list and does not review third-party code. The compatibility layer supports the anime subset used by sources such as AniKoto and HiAnime: search, detail, chapters, video lists, `Client`, `SharedPreferences` and `Document`. Manga sources, a filter UI and native WebAssembly are not supported. See `docs/PLAN-MANGAYOMI-SHIM.md`.

Embedded libmpv is bundled behind the default-off `kaminari.libmpv` flag. Set `localStorage["kaminari.libmpv"]="true"` or add `?libmpv=1` to the app URL to opt in. The default player stays the WebView player with external mpv fallback. Public distribution waits for counsel review of `docs/LEGAL-REVIEW-DRAFT.md`; the dev spike passes z-order, HEVC and fallback on Windows. A 2026-11-08 review confirmed Suwayomi is manga-only and cannot run Aniyomi/yuzono anime extensions, so native `.kext` stays the bundled source model. The app does not download APKs or run package code from the Aniyomi registry.

Auto update uses the Tauri updater and signed NSIS artifacts. The configured GitHub endpoint stays empty while the repository is private; a signed public `latest.json` is required before the Check for updates button can find a release. `pnpm package:portable` builds a zip beside the installer. The portable build still uses the normal per-user data folder, so settings and the library survive replacing the folder.

When an AniList title is opened through Watch, finished source episodes are queued to AniList progress. Manual episode marks on an AniList detail page were already queued. Source playback opened directly from Sources is not linked to an AniList entry.

Windows is the first target. macOS and Linux come later.

## Run it

```bash
pnpm install
pnpm tauri dev
```

The installer build expects the two pinned libmpv DLLs in `src-tauri/lib/`. Download the archives listed in `licenses/native/README.md`, extract `libmpv-2.dll` and `libmpv-wrapper.dll` there, then run `pnpm check:libmpv`. The Tauri build runs the check automatically and stops on a missing file or hash mismatch.

Builds are produced locally with `pnpm tauri build`; the build verifies the pinned libmpv DLLs with `pnpm check:libmpv` before packaging. GitHub Releases will follow the first public build. No telemetry, no ads, no account needed to browse.

## Versioning

The root `VERSION` file is the only version source. `pnpm build` and `pnpm tauri` run `scripts/sync-version.mjs` first, which writes `package.json` and `src-tauri/Cargo.toml`. Tauri reads the package version, Vite injects `__APP_VERSION__` into the frontend, and Rust reads `CARGO_PKG_VERSION`. `src-tauri/build.rs` refuses a build where `VERSION` and `Cargo.toml` disagree.

## Licenses

Settings, and the footer, open a generated list of every Rust crate, npm package, font and built-in sample in this build. The rows carry versions, license identifiers, sources and full license texts where the installed package includes one. Native libmpv artifacts are bundled and listed under Native libraries with their hashes, source URLs and the source offer. Run `pnpm licenses:build` after dependency changes; it regenerates `public/licenses.json` from `Cargo.lock` and the installed node_modules.

## Principles

1. **Do not trust a measurement you have not taken.** Every choice gets measured before it ships, from bundle size to search latency.
2. **If the screen cannot see it, it cannot trust it.** Request counts, cache use, and source health stay visible where they matter.
3. **The library is yours.** One SQLite file you can copy, back up, and move. Uninstall leaves it alone.
4. **Small pages, honest copy.** Short sentences. Real errors. No fake success states.

## Docs

- `docs/ARCHITECTURE.md`: how the pieces fit
- `docs/DESIGN-SPEC.md`: tokens, components, motion
- `docs/EXTENSION-API.md`: public contract for extension authors, plus Aniyomi registry mapping
- `docs/PLAN-JS-EXTENSIONS.md`: native host decisions, API, milestones and tests
- `docs/PLAN-ANILIST-AUTH.md`: implicit grant, deep link and keychain plan
- `docs/LEGAL-REVIEW-DRAFT.md`: draft counsel packet for the bundled libmpv artifacts
- `docs/ENGINEERING-NOTES.md`: measured numbers and decisions that reversed
- `docs/ROADMAP.md`: what comes next and what done means
- `docs/TERMS.md`: terms of service for distributed builds
- `docs/PRIVACY.md`: local-first privacy policy
- `VERSION`: the single version source
- `public/licenses.json`: generated license list used by Settings

## Legal

Kaminari ships no content, no source list, and no APKs. It is a general-purpose player and extension host. You are responsible for the extensions you install and the content you access. Read the [Terms of Service](docs/TERMS.md) and the [Privacy Policy](docs/PRIVACY.md).

This project is not legal advice. If you distribute builds publicly, have a qualified lawyer review the terms for your jurisdiction.

## Contributing

See `CONTRIBUTING.md` for setup and review rules.

## License

MIT. See `LICENSE`. Unofficial and not affiliated with AniList. The name Kaminari means thunder in Japanese.
