# Kaminari

Aniyomi, but for PC.

A desktop anime app for Windows first, with care in every pixel. Fast startup. Keyboard first. A library you own in one SQLite file. AniList for data. Extensions you choose for more. No content ships in the box.

> Ships no content, no source list. It talks to AniList for data, and to extensions you choose to install for more. That is the whole idea.

![status](https://img.shields.io/badge/status-design--spec-blue) ![tauri](https://img.shields.io/badge/tauri-2.x-teal) ![svelte](https://img.shields.io/badge/svelte-5-orange) ![license](https://img.shields.io/badge/license-MIT-green) ![windows](https://img.shields.io/badge/windows-first-0078D6)

| Library | Search | Player | Extensions |
|---|---|---|---|
| Watching and planning with progress and history | Ctrl K palette, live plus cached offline | libmpv, ASS subs, J L K keys, diagnostics with D | Sandboxed QuickJS, signed .kext, permission prompts |

Screenshots live in `assets/screenshots/`: hero, library, detail, player, palette. Placeholders until P2, then real build shots. Every PR that changes UI updates the `/design` route and one screenshot.

## What it is

- **Core:** Tauri 2 plus Rust for shell, networking, database, extension host, player bridge
- **Interface:** Svelte 5 with Runes plus TypeScript plus Tailwind CSS 4
- **Data:** AniList GraphQL for titles, posters, scores, tracking
- **Player:** libmpv embedded for smooth HLS and MP4
- **Extensions:** sandboxed JS modules through QuickJS, signed registry JSON, hot reload in dev

## Why this stack

Tauri keeps the installer small and startup fast. Rust keeps the heavy parts honest. Network, storage, and trust live there. Svelte keeps the interface light and easy to shape by hand. libmpv handles video better than a web player will. QuickJS lets authors use plain JavaScript without writing Rust.

If you already know React, the Rust side does not care. The contract stays the same.

## Status

Working today: cinematic browse with five AniList rails, Top 10 Today, Ctrl K palette, detail pages, a local AniList library, watched episode tracking, Continue Watching, an Aniyomi registry reader that stores source metadata, a native JavaScript extension host with a source browser, details, episodes and stream resolve, an in-app player for MP4 and HLS, and a source library with resumable playback progress, source Continue Watching and merged history.

Next: embedded libmpv for HEVC, ASS and HDR. The app does not download APKs or run package code from the Aniyomi registry.

Windows is the first target. macOS and Linux come later.

## Run it

```bash
pnpm install
pnpm tauri dev
```

Builds land in GitHub Releases. No telemetry, no ads, no account needed to browse.

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
- `docs/ENGINEERING-NOTES.md`: measured numbers and decisions that reversed
- `docs/ROADMAP.md`: what comes next and what done means

## Contributing

See `CONTRIBUTING.md` for setup and review rules.

## License

MIT. See `LICENSE`. Unofficial and not affiliated with AniList. The name Kaminari means thunder in Japanese.
