# Engineering notes

Live record of numbers we measured, decisions we reversed, and traps we found. Numbers here are from this machine checkout. If you remeasure, update the date and the result. Do not copy a number forward as fact.

Template for each entry:

```
## 2026-11-07 Issue name

Context. One short paragraph.

What we tried. What we measured.

What we shipped, or why we changed course.
```

## Start

Project has not shipped yet. Entries land as we build and test. First tasks on this page will be bundle size, cold start, and AniList rate limit behavior under real load.

Papyrus taught us a few habits that carry over here:

- Fan out looked faster on paper and lost real results in practice. We will measure search the same way, per source, before we trust it.
- A filter that quietly emptied a list looks like a result of zero. We will show what a source excluded, or we will not offer the filter at all.
- An unread or progress count is computed here, not trusted from a server that does not know this user. When a count can go stale, we mark it stale instead of guessing.

## Bundle budget

Target installer under 15 MB on Windows before libmpv resources. CI fails if we cross it. If we cross it, we look at what grew. No silent growth.

## Cold start

Target first paint before 500 ms on a mid range laptop, internal storage, warm cache. Measure with `pnpm tauri dev -- --measure` once the scaffold lands, and again in a release build. A debug number is not a release number.

## AniList rate limit

Public limit is 90 requests per minute. We plan a token bucket in Rust at 60 per minute sustained with a short burst allowance. Batch media fetches where we can. On `429` we read `Retry-After` and back off. Show the user a short wait message with seconds left, not a blank page.

## Images

Posters and banners come from AniList URLs. We cache on disk with an LRU at 512 MB and show a blurhash while loading. A grid of 500 items must scroll at 60 fps. That means virtualization on the frontend and a Rust cache that does not block the UI thread.

## Player

libmpv is the primary path. External mpv and VLC are fallback only, for machines where bundled libraries fail. The custom controls sit on top of the mpv surface, not inside a web video tag, so HDR and 4K stay real.

## Extensions

One runtime per extension. Calls time out at 5 seconds, 10 seconds for a resolve. Memory cap 64 MB. No filesystem, no child process, no env. One bad extension greys out its source. The app stays up.

## Database

SQLite with WAL. One file, checked in migrations, backup with `VACUUM INTO`. Library, progress, and history live here and nowhere else. Source ids above 2^53 stay strings on the frontend boundary.

## Updates

Desktop updates come from GitHub Releases with Minisign signatures. The updater polls `latest.json` and shows the version, size, and notes before it touches anything. Roll back is reinstall the prior `.exe`. The library file is left alone.

## 2026-11-08 Registry compatibility sample

Context. We want Kaminari to import the Aniyomi community index without rework, then sign our own entries. The reference file lives at `yuzono/anime-repo`, path `repo/index.min.json`.

What we measured. Fetched the file over HTTPS and parsed it with Python. Count was 250 packages and 304 sources. Top level keys per entry: `apk`, `code`, `lang`, `name`, `nsfw`, `pkg`, `sources`, `torrent`, `version`. One package holds one to nine sources. Sample package `eu.kanade.tachiyomi.animeextension.all.animeonsen` ships apk `aniyomi-all.animeonsen-v14.10.apk`, version `14.10`, code 10. Sample source `AnimeOnsen` holds id `8542735178285060053`, lang `all`, baseUrl `https://www.animeonsen.xyz`. That id is larger than 2 to the 53 (9007199254740991). Same pattern holds across the file, for example `2012957501886720048` and `8052146309028882026`.

What we shipped. Mapping is now in `docs/EXTENSION-API.md` under registry compatibility. `pkg` maps to our extension id. `apk` plus `version` and `code` map to our artifact name and update check. `sources[]` maps to rows with TEXT ids, never INTEGER, never float. Import validates digits only and keeps the string end to end, Rust to JSON to TS. Native registry adds `downloadUrl`, `sha256`, `signature`. Legacy rows install as sideloaded until signed. `torrent: 1` needs explicit opt in and stays off by default.
