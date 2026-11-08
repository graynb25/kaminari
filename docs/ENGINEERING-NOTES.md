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

## 2026-11-08 Library status and history

Context. The library could hold titles but not file them by state, and watch events had no page of their own.

What we shipped. `library_set_status` accepts five values and rejects anything else before it reaches SQL. The History view reads `history_list`, grouped by series and episode, ordered by the newest event. Two marks in the same second would otherwise swap places, so the query also orders by the newest row id. The view shows relative time, episode number, and opens the series. Status filters and a title filter live on the Library page, with counts computed from the same rows.

What we did not do. No bulk status edit and no history clear yet. Both are small follow-ups.

## 2026-11-08 Progress and Continue Watching

Context. The catalogue was read only. The library could save a title but not track it, so Continue Watching had nothing honest to show.

What we shipped. A watched table keyed by AniList id and episode. Marking an episode watched upserts the media row, adds the series to the library as watching if it is missing, inserts history, and sets progress to the highest watched episode. Unwatching recomputes the highest episode and drops the progress row when none remain. Continue Watching joins progress to library rows with status watching, ordered by updated_at. Seven Rust tests cover the round trip.

What we did not do. No player position yet. position_sec and duration_sec exist for the libmpv pass, but the current watched toggle writes zero. The UI does not pretend an episode played.

## 2026-11-08 CI shape and first live AniList call

Context. The first push went red twice. Both failures were boring and worth writing down.

What we tried. `cargo fmt` ran from the repo root where there is no `Cargo.toml`, so the Rust job never reached a compile. The web job asked pnpm for a frozen install with no lockfile in the repo. After those were fixed, `cargo clippy` in the Rust job failed because Tauri's context macro wants `frontendDist` to exist and the web build ran in a separate job. We merged the jobs: build the frontend first, then run fmt, clippy and tests in `src-tauri`. We also caught `tsc` emitting `src/main.js` beside the source, so the build script now runs `tsc --noEmit`.

What we measured. The trending query in `anilist.rs` ran against the live endpoint on 2026-11-08 and returned real rows. First page, three per page: Black Clover Season 2 (id 195604, score 87), Aoashi Season 2 (191788, score 80), Blue Box Season 2 (189123, score 81). All season year 2026, format TV. The shape matches our serde structs, including `coverImage.extraLarge`.

What we shipped. One CI job that fails on fmt, clippy, tests, type check, or build. No network in tests. The live call was a manual check, not a CI test. A wiremock fixture for 429 and malformed GraphQL comes with the cache work.

## 2026-11-08 Registry compatibility sample

Context. We want Kaminari to import the Aniyomi community index without rework, then sign our own entries. The reference file lives at `yuzono/anime-repo`, path `repo/index.min.json`.

What we measured. Fetched the file over HTTPS and parsed it with Python. Count was 250 packages and 304 sources. Top level keys per entry: `apk`, `code`, `lang`, `name`, `nsfw`, `pkg`, `sources`, `torrent`, `version`. One package holds one to nine sources. Sample package `eu.kanade.tachiyomi.animeextension.all.animeonsen` ships apk `aniyomi-all.animeonsen-v14.10.apk`, version `14.10`, code 10. Sample source `AnimeOnsen` holds id `8542735178285060053`, lang `all`, baseUrl `https://www.animeonsen.xyz`. That id is larger than 2 to the 53 (9007199254740991). Same pattern holds across the file, for example `2012957501886720048` and `8052146309028882026`.

What we shipped. Mapping is now in `docs/EXTENSION-API.md` under registry compatibility. `pkg` maps to our extension id. `apk` plus `version` and `code` map to our artifact name and update check. `sources[]` maps to rows with TEXT ids, never INTEGER, never float. Import validates digits only and keeps the string end to end, Rust to JSON to TS. Native registry adds `downloadUrl`, `sha256`, `signature`. Legacy rows install as sideloaded until signed. `torrent: 1` needs explicit opt in and stays off by default.

The Sources page now fetches the index live, renders 250 packages with a filter and 60 card batches, and stores selected entries in SQLite as metadata. No APK download, no package execution. A live load on 2026-11-08 returned all 250 packages; the app rendered them at 1440 width without a stall.
