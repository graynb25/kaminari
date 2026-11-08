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

## 2026-11-08 Miru/Hayase and Mangayomi review

Context. A search result called Miru the closest PC-native Aniyomi/Suwayomi and Mangayomi a desktop client with an extension manager. We checked both against the native-first decision.

What we verified. Miru rebranded as Hayase. It is an Electron P2P torrent client with a Svelte interface and WebTorrent, not a server, and it has no general local API. Its extensions are ESM classes for torrent, NZB, subtitle and web seed results, run in a Web Worker, and mostly return magnets, so reuse would also require a BitTorrent engine. The app is BUSL-1.1 with a change date of 2029-04-01, so it cannot be bundled before then. Mangayomi is a Flutter client, Apache-2.0, v0.9.95 on 2026-11-01, with Dart extensions through d4rt, JavaScript extensions through QuickJS, and Mihon/Aniyomi APKs through the M-Extension-Server bridge. It has no headless mode and no general HTTP API; its only local server is the Go torrent byte server. Its JavaScript anime extensions are the useful find: they already run in QuickJS, and they return direct URLs with headers and subtitles. They still need async evaluation, an `MProvider`/`Client`/`Document` bridge, video extractors, result mapping, and a host policy because they declare no host allowlist and are unsigned. Community anime repositories exist, for example `Mallyd11/mangayomi-anime-extensions` and `m2k3a/mangayomi-extensions`, both Apache-2.0.

Decision. Neither project is a Suwayomi analogue and neither changes the native-first foundation. If an optional provider experiment is funded, the order is now: a Mangayomi JavaScript compatibility adapter behind a flag, then `miwayomi` for Aniyomi APKs, then Seanime as a separate app. Hayase stays a separate torrent app and is not a dependency. The adapter is estimated at 6 to 10 weeks for a useful prototype and needs a trust prompt because the format has no signatures. The acceptance test is three extensions covering an API source, an HTML scrape and an HLS extractor; if fewer than two work, stop.

## 2026-11-08 Suwayomi and anime server review

Context. Papyrus runs Suwayomi as its manga source engine. The user asked whether Suwayomi can run Aniyomi/yuzono anime extensions and, if not, whether an anime equivalent exists for Kaminari.

What we verified. Papyrus pins Suwayomi 2.4.2366, bundles a Temurin JRE and a 174.5MB server JAR, binds it to loopback behind `UI_LOGIN`, speaks GraphQL through one gateway, and exposes only an allowlisted image proxy to the web UI. Its installer is 257MB. Suwayomi itself is a Mihon/Tachiyomi manga server: its changelog removed anime support in v0.6.0 on 2021-12-29, and the request to bring it back (issue #387) has stayed open since 2022. Aniyomi extensions use a different source API and a video model, so this is not a store URL or manifest flag. No mature Suwayomi-equivalent for Aniyomi exists on 2026-11-08. `miwayomi` is the closest turnkey candidate, but it is a v0.2.9 alpha from August 2026 with about 30 stars, no API auth, and arbitrary APK code in one JVM. `M-Extension-Server` is a runtime component, not a server. Seanime is mature but uses GPL-3.0 JavaScript/TypeScript providers, not Aniyomi APKs.

Decision. Kaminari stays native-first. `.kext` through QuickJS remains the only bundled provider model. The next playback work is a loopback stream proxy with header, cookie and HLS manifest handling, then embedded libmpv for HEVC, ASS and HDR. A user-installed anime server adapter can be explored later behind an experimental flag, with the server bound to loopback, a per-launch token, a narrow proxy and no raw admin API in the WebView. Do not bundle an APK runtime or a source list. If a server is ever bundled, the terms and privacy docs must change because the no-APK posture would no longer hold.

## 2026-11-08 Version file and generated licenses

Context. The version was typed into ten places: `package.json`, `Cargo.toml`, `tauri.conf.json`, the nav, Settings, the footer, two HTTP user agents and a player test. Every bump risked a mismatch. The user asked for one version file. The user also pointed at Papyrus, which has a generated licenses page, and asked for the same for Kaminari.

What we shipped. The root `VERSION` file is now the only version source. `scripts/sync-version.mjs` writes `package.json` and `src-tauri/Cargo.toml`; `prebuild` and `pretauri` run it automatically. `tauri.conf.json` reads `../package.json`, Vite injects `__APP_VERSION__`, and Rust reads `CARGO_PKG_VERSION`. `src-tauri/build.rs` fails a direct cargo build when VERSION and Cargo.toml disagree. The Settings card and footer now open a generated licenses dialog. `scripts/generate-licenses.mjs` reads `cargo metadata` for the host triple, scans the installed node_modules, adds the bundled fonts, sample and Kaminari itself, and writes `public/licenses.json`. The page groups Bundled, Rust and JavaScript, filters by name, version, license or source, and opens one full license text at a time.

What we measured. The first generation produced 432 packages, 30 license identifiers, 20 full texts and a 204KB JSON file: 5 bundled, 341 Rust on `x86_64-pc-windows-msvc`, 86 JavaScript. No package was unrecorded and none reported a proprietary license. The live dialog rendered all 432 rows, the search narrowed hls.js to one row, and the footer and Settings both opened it. The Svelte a11y warning about a focusable `<pre>` was fixed by wrapping the text in a focusable region.

What we did not do. The licenses file is generated manually with `pnpm licenses:build`, not on every build, because `cargo metadata` and a full node_modules scan should not be a tax on frontend-only builds. The dialog has no acknowledgement ledger; Kaminari has no accounts. AniList remains a service credit, not a package row.

## 2026-11-08 Footer and in-app legal documents

Context. The Terms and Privacy drafts lived only in the repository. The user asked for a footer in the app with access to them.

What we shipped. `AppFooter.svelte` adds a token-themed footer at the end of the main scroll area: bolt mark, `No content or sources are included.`, `Metadata by AniList`, Terms, Privacy and the version. Short pages push it to the viewport bottom with a `min-h-full` flex column and a `flex-1` page slot. `LegalDialog.svelte` reads `docs/TERMS.md` and `docs/PRIVACY.md` through Vite raw imports, renders headings, lists, bold and inline code with a small local parser, and provides tabs, a draft notice, a focus trap, Escape close and focus restore. The main scroll area locks while the dialog is open.

What we measured. Settings at 2560 shows the footer on the viewport bottom without scrolling. Terms and Privacy both open with the correct h1, the dialog switches tabs, Escape closes it, focus returns to the Terms button, and pressing `1` while it is open does not leave Settings. Light and dark both render without unstyled text.

What we did not do. The contact, jurisdiction and repository placeholders are still unfilled. The dialog labels the documents as drafts. This is not legal advice.

## 2026-11-08 Legal drafts

Context. The app is a player and extension host with no shipped content. The user asked for a Terms of Service and a Privacy Policy before distribution.

What we wrote. `docs/TERMS.md` describes the app, the user's responsibility, third-party extensions, no warranty, liability limits and copyright complaints. `docs/PRIVACY.md` describes the local SQLite and WebView data, the direct AniList, registry, extension and stream requests, no telemetry, deletion paths and contact. Both link from the README.

What we did not claim. We did not write that the app is "for educational purposes" or that it cannot facilitate infringement. A label is not a legal defense. The documents describe facts and prohibit unlawful use. The strongest posture remains: no content, no source list, no APK execution, sandboxed extensions, user-supplied sources.

What is missing. `[[CONTACT_EMAIL]]`, `[[JURISDICTION]]` and `[[REPOSITORY_URL]]` are deliberate placeholders. Fill them before publishing, and have a qualified lawyer review the documents for the distribution jurisdiction. This is not legal advice.

## 2026-11-08 Missing AniList score

Context. The user pointed at SPY×FAMILY Season 4 in Top 10 and asked why it had no rating.

What we measured. A live AniList query for id 219714 on 2026-11-08 returned `averageScore: null`, `meanScore: null`, `status: NOT_YET_RELEASED`, `startDate.year: 2027`, `episodes: null`. The missing badge was correct data. The UI had simply hidden the slot when the score was null.

What we shipped. Poster cards now show `NR` in the same top-right slot, with the tooltip `AniList has no score yet`. The detail card already showed `n/a`. The hero still omits the score chip because a `Not rated` pill would crowd the meta row.

What we did not do. No fabricated score and no popularity fallback. Those would misrepresent AniList.

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
