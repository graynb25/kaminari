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

## 2026-11-08 Native JS host live search

Context. The app could read AniList and store Aniyomi metadata, but it could not run a source. The plan called for a sync QuickJS host with a Rust bridge.

What we shipped. A host in `ext_host.rs` with one runtime per call, 5s and 10s deadlines, 64MB memory, 1MB stack, an HTTPS host allowlist, blocking `net.fetch`, `html.select` through the Rust scraper, SHA-256, SQLite storage and a logger. `js_extensions` and `js_storage` tables, `.kext` zip reading, a built-in Public Domain Animation package, and a Sources test panel. Nine host tests cover static results, missing methods, async rejection, timeout, allocation, host denial, storage across fresh runtimes, HTML select, and kext round trip. Two DB tests cover install and storage cascade.

What we measured. On 2026-11-08 the sample was installed and searched through the live Sources page. archive.org returned 24 Popeye rows with covers and years. The host tests finish in about 0.10s total, so a cold runtime is below the 50ms budget on this machine. The sample search took well under the 5s cap.

What we did not do. `getShow`, `getEpisodes` and `resolveEpisode` are implemented in the sample but not wired to a browse UI yet. Async methods return `ASYNC_UNSUPPORTED`. Packages are unsigned and stay local. No registry.

## 2026-11-08 Icon refresh and episode actions

Context. The first icon was a flat lavender bolt on black. It read as a placeholder. The user asked for a professional icon. Source episode rows also said Resolve, which is host language, not a viewer action.

What we shipped. A new squircle tile: deep indigo gradient, volt halo, low floor glow, 4px rim, and one beveled bolt with a soft glow and shadow. The editable source is `assets/brand/icon.svg`; the full Windows, macOS, iOS and Android set came from `pnpm tauri icon assets/brand/icon-1024.png`. Source episode actions are now Play, Resume or Rewatch. Play resolves and starts the first stream in one click. A second Streams button opens the quality list with Copy and Open in mpv. Resolve stays in code and logs.

What we measured. The icon reads at 32px in the headless preview and the generated 32px PNG still shows the bolt silhouette. The 128px and 512px renders keep the rim and glow without muddying the edge. In the app, Play started the episode at 0:00, the row changed to Resume after progress saved, and Streams showed the MP4 row with Resume, Copy and the mpv fallback. No page errors.

What we did not do. No icon animation, no alternate icon, no transparency variant for the Windows taskbar. The 48px transparent margin is intentional so the squircle does not clip.

## 2026-11-08 Light theme and theme modes

Context. The app only shipped dark. That is the signature, but it is not a reasonable default for everyone. The user asked for a light mode and room for other styles.

What we shipped. A token layer with System, Dark and Light modes, a theme picker in Settings, localStorage persistence, a pre-mount `public/theme-init.js`, and a live `prefers-color-scheme` listener. Light is a porcelain page with ink type, indigo-tinted shadows and light glass. The home hero, detail banner and source hero use light scrims with ink copy. Poster art, source art and the player keep the dark token set in both themes so badges, progress strips and player controls stay legible over unknown artwork.

What we measured. Light primary buttons use white on `#605ef0` to `#4d4ad8`, about 4.7 to 1 at the top and 6.2 to 1 at the bottom. Muted ink `#6a6880` on the page passes 5 to 1. The theme picker switched dark, light and system in the running app, persistence survived a reload, and System followed the OS, which reported light on this machine. Screenshots at 1920 covered home, library, history, settings, sources, detail and source detail without dark artifacts on page chrome.

What broke and changed. Svelte pruned `[data-theme="light"] .detail-hero-scrim` rules inside component style blocks because the attribute never appears in that component's markup. The dark scrim survived, so light detail copy landed on dark art. Both hero scrims moved to global `app.css` and the issue disappeared.

What we did not do. Accent palettes, a high contrast mode, and a full axe pass on light. Those are follow-ups, not blockers.

## 2026-11-08 Source resume handoff

Context. Continue Watching cards said Resume but only opened the source detail. The user had to find the same episode and click Resolve, then Resume, before a frame played.

What we shipped. A source continue card passes its episode id into the source browser. After detail, episodes and progress load, the browser resolves that one episode and starts the first stream at the saved position. A source library Resume button does the same through `last_episode_id`, which `source_library_list` now returns. History rows still open the show without autoplay.

What we measured. Clicking the seeded Continue card at 120 seconds opened the player on episode 1. The video element reported `currentTime` 144.8 after 24 seconds of playback, and the card refreshed to 2:45 when the player closed. The source detail showed `Resume 2:46`. Both the player and the source browser closed on their own Escape presses.

What we did not do. Auto-resume from History, or from a source library card that has no saved progress. Those open the show and wait for a click.

## 2026-11-08 Source continue and merged history

Context. The source library could resume an episode, but the home shelf and the history page only knew about AniList. A user who only watched source shows saw an empty home and an empty history.

What we shipped. `source_continue` and `source_history` read from `source_progress` joined to `source_library` and the installed extension name. Home renders source cards inside Continue Watching with the cover, extension name, position over duration and a volt progress strip. History merges AniList watched events and source progress rows into the same day groups, newest first. Source rows carry In progress or Watched and open the show in the source browser.

What we measured. A seeded `PopeyeWithBettyBoop` row at 120 of 456 seconds rendered on home as `2:00 / 7:36` with EP 1 and RESUME, and on History as an In progress row one hour old. The window at 2560 by 1400 showed both lists without overflow. The source continue query uses `ROW_NUMBER()` per extension and show, so two episodes with progress produce one card, the newest. The Rust test covers that case.

What we did not do. Per event source history. The table stores one row per episode with its latest position, so History shows the latest state of each episode, not every pause. AniList events remain one row per watched mark.

## 2026-11-08 Native source browser

Context. Search worked, but a source had no detail, episode or resolve path in the UI.

What we shipped. `js_extension_detail`, `js_extension_episodes` and `js_extension_resolve` call the same host with `getShow`, `getEpisodes` and `resolveEpisode`. A full-screen SourceBrowser opens from a native extension card, runs an empty search on mount, opens a result, lists episodes and resolves a stream URL. The sample strips archive.org HTML and parses durations.

What we measured. On 2026-11-08 the sample opened an archive.org title, returned one episode, and resolved an MP4 URL. The whole path stayed under the 5s and 10s caps.

What we did not do. Playback. The resolve panel shows and copies URLs; libmpv is the next milestone.

## 2026-11-08 Hero poster fallback

Context. The user reviewed the new hero and said some headers were fully blurred. AniList has no `bannerImage` for several 2026 and upcoming titles, so those slides fell back to a blurred cover. The user then pointed at the Dragon Ball Super slide and said that clear full-bleed banner with the slow zoom is how every slide should display.

What we shipped. The hero now filters trending to entries with a real `bannerImage` before taking five slides. Every slide is a clear full-bleed image with the existing slow zoom. The eyebrow keeps the original trending rank by looking the slide up in the unfiltered list. Titles without a banner stay in Top 10 and the shelves. The blurred cover plus clear poster card remains only as a fallback when fewer than three banner titles exist.

What we measured. 17 of the first 24 trending titles have a banner on 2026-11-08, so the filter still fills five slides. The real app shows Black Clover Season 2 (#6) and ONE PIECE (#9) full bleed at 1920, each with the slow zoom, and the copy stays readable.

## 2026-11-08 Visual system pass

Context. The app worked but read as flat dark panels. The spec promised glass overlays, cinematic depth and confident type, and the implementation did not deliver.

What we shipped. Local Sora, Inter and JetBrains Mono woff2 subsets with OFL notices. Layered atmosphere, vignette and grain behind the shell. Glass for the title bar, palette, player controls, toasts and dialogs only. Rebuilt the hero, shelves, library stats, source library cards, history groups, source registry cards, detail score ring and status menu, and the full player control bar. Added focus traps, focus restore and reduced-motion handling. The UI session verified layouts with mocked IPC at 1600, 1280 and 1024 widths before the orchestrator ran the real app.

What we measured. The production build bundles about 225 KB of JS, 63 KB of CSS and 214 KB of fonts before compression. The main JS is 69 KB gzipped. The lazy hls.js chunk stays at 187 KB gzipped and only loads for HLS. The real Tauri app rendered Home, Library and Sources with live data at 1920 by 1080 without layout breaks.

What we did not do. Embedded libmpv/libass, blurhash, i18n keys and 500-poster virtualization. The docs mark those as later work. The player still uses the in-app WebView surface for MP4 and HLS, with mpv as the fallback.

## 2026-11-08 Source library and resume progress

Context. The player could play one episode, but it forgot everything when the window closed. Source shows had no library of their own.

What we shipped. `source_library` and `source_progress` tables, a source library card in the Library view, an Add or Remove Library button in the source detail, and a player progress callback. The player writes position every 5 seconds and on pause, close and end. A saved position below 95 percent offers Resume. Reaching 90 percent marks the episode watched and updates the card. Source detail rows show a progress bar and Resume or Watched.

What we measured. The seeded source library card showed `Popeye with Betty Boop`, `Episode 1`, `2:00 / 7:36`, and a Resume button. The sample's H.264 episode plays in the in-app player with the CSP fix from the previous note.

What we did not do. Source history and Continue Watching for source shows. The Library section covers resume for now.

## 2026-11-08 In-app player

Context. The external mpv handoff opened a separate window. That is not the design. Play should stay inside Kaminari.

What we shipped. A full-screen PlayerView with an HTML5 video surface, custom controls, keyboard shortcuts, speed, volume, fullscreen, subtitle tracks and buffering and error states. HLS loads through a lazy `hls.js` chunk. External mpv stays as an explicit fallback button for formats the WebView cannot decode. The CSP now allows `media-src https: blob:` and `connect-src https: blob:`.

What we measured. The public domain sample's H.264 Popeye MP4 played inside the app. The first attempts failed with media error code 4, which looked like a codec problem. It was the CSP: `default-src 'self'` blocked the media request. Adding `media-src https: blob:` fixed it. A video element does not need CORS for playback, but it does obey CSP.

What we did not do. Embedded libmpv, HEVC, ASS and HDR. External mpv remains the fallback. The next milestone is the libmpv wrapper, libmpv-2.dll, resource bundling and a transparent player surface.

## 2026-11-08 External mpv handoff

Context. Resolve returned a URL, but the app could not play it. M4 needed a real player without waiting for the embedded libmpv packaging work.

What we shipped. A `player` module discovers mpv through `KAMINARI_MPV`, PATH, common Windows install paths, then a dev `src-tauri/bin` path. It builds mpv arguments for headers, subtitles, start time and a default user agent, starts one process at a time, and closes the previous one. SourceBrowser now has Play beside Copy. Settings shows the mpv path or version. The app owns the process and kills it on exit.

What we measured. On 2026-11-08 mpv 0.42.0 was installed at `C:\Program Files\MPV Player\mpv.exe`. The public domain sample's resolved MP4 opened in its own mpv window with the title `Farmer's Field ... Episode 1` and a 4:15 duration bar. The app stayed up.

What we did not do. Embedded playback and custom controls. The mpv window is separate for M4. Embedded libmpv needs `libmpv-wrapper.dll`, `libmpv-2.dll`, resource bundling and a transparent player surface. That is the next milestone.

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
