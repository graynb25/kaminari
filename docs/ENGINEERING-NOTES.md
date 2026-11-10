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

## 2026-11-08 UI polish phase 1

Context. The Papyrus styling review found that Kaminari shares most of the component grammar but still hard-codes accent and status colors, repeats glass borders inline, and lacks press and touch feedback. Phase 1 of `docs/PLAN-UI-POLISH.md` was the low-risk pass.

What we shipped. Accent aliases (`--accent-wash`, `--accent-wash-strong`, `--accent-line`, `--accent-line-strong`) and status aliases (`--success-wash`, `--warning-line` and their siblings) resolve from the active theme. Component CSS and every Svelte file now use them instead of literal violet, amber and rose RGB values. `.on-art` is the single surface for labels over artwork. `.glass` and `.glass-strong` carry border, inset highlight and shadow, and the four dialogs no longer repeat those inline. Poster cards have a 2px focus ring, a press scale and a `hover: none` fallback for quick actions. Paragraphs wrap with `pretty`, body text uses optical sizing, and `.nums` is available for tabular figures. Reduced motion removes the poster and source card lift, image zoom and press transforms.

What we measured. At 1280 the source grids report four equal columns. The poster focus ring computes to 2px solid `rgb(96 94 240)` with a 3px offset. The licenses dialog computes to a 1px `rgb(255 255 255 / 0.14)` border, the inset highlight plus deep shadow, and its 22px radius. Daylight library, browse, sources and the licenses dialog were checked at 1280, and the dark library and licenses dialog were checked at the same size. The library reported no horizontal overflow at 1024, 1600 or 2560. Under reduced motion the poster card and its image both computed `transform: none`.

What we did not do. Cascade layers, the page header and modal atoms, artwork blur-up, callouts and settings navigation stay in phases 2 and 3. Theme atmosphere gradients keep their per-theme values on purpose.

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

## 2026-11-08 Embedded libmpv P0 spike

Context. The WebView player cannot cover HEVC, ASS and HDR reliably. P0 had to prove the native surface, compositing and fallback before any packaging or license commitment.

What we shipped. A default-off `kaminari.libmpv` flag, `MpvView`, typed `mpv_embed` commands with a fixed intent allowlist, a dev harness at `?mpvHarness=1`, gitignored DLLs in `src-tauri/lib`, and `scripts/prepare-libmpv-dev.ps1` to copy them into the debug target. PlayerView remains the default and the app stays opaque outside the player route.

What we measured. Z-order passed in a 1280x800 window and at 2560x1440 fullscreen: video visible under the glass bars, diagnostics and error panel. First frame was 1.82 and 1.94 s for the archive.org Popeye MP4, 0.59 s for a 10 s MP4 and 0.23 s for local 1080p 10-bit HEVC. HEVC used `hevc` and `hwdec-current=d3d11va`; CPU was 0.5 percent of the machine, GPU video 3.5 percent average and 10.6 percent peak. With the DLLs missing, SourceBrowser mounted PlayerView, showed the fallback toast and played. Two open and close cycles left `activeLabel` null. DLLs are 101,853,696 and 391,680 bytes uncompressed.

What we did not do. No DLLs are tracked, bundled or added to `bundle.resources`. The pre-P1 native package is in `licenses/native` with the license texts, artifact hashes and source offer, but the exact FFmpeg commit and lawyer sign-off remain open. No installer delta, clean install or update test yet. The wrapper rejects JSON arrays, so `http-header-fields` is one comma separated string and header values containing commas are unsafe; protected MP4 and HLS stay on the proxy. mpv rejects the `start` property, so resume seeks on `file-loaded`. P1 is blocked on license sign-off, a dedicated watch window or proven route transparency, the proxy header path, and packaging tests.

## 2026-11-10 Source watched marks without playback

Context. Source progress was only written by the player after roughly 90 percent of an episode, or when it ended. The AniList detail page already had Mark all, but that table is separate from source progress. A user who had already watched a show had to replay every episode for the source library and the player to count it.

What we shipped. Source episode rows now have a watched toggle, and the Episodes header has Mark all watched and Clear. Mark all writes every source episode as watched in one SQLite transaction, adds the show to the source library if it was not saved, and queues AniList progress through the highest mapped episode when the show came from Watch. Clear deletes the local source progress rows and does not touch AniList. Progress changes refresh the source library, Continue Watching and the AniList shelves.

What we measured. The public domain sample returned 40 Popeye episodes. Mark all showed 40 watched, the source library card showed 40 watched and episode 40 at 6:02 of 6:02, and the button disabled. Marking episode 1 unwatched showed 39, and marking it again restored 40. Clear returned the page to 0 watched and removed the rows. 118 Rust and 93 frontend tests pass.

## 2026-11-10 Sources cleanup and first-run checklist

Context. The Sources page still led with an Aniyomi registry reader. Its packages are Android APKs and QuickJS cannot run them, so the panel offered a dead end. A new install also had no in-app explanation of what to do first.

What we shipped. The registry panel, its URL state, list, filters and stat cards are gone from `SourcesView.svelte`. The backend metadata commands remain for a possible future runtime. The four stat cards now show native extensions, Mangayomi sources, allowed hosts and runtime memory. A first-run Getting started modal explains adding a source, finding a title and watching; it is dismissible, stores `kaminari.onboardingDone`, and reopens from Settings, About. Verified in a debug build: the card appears on a fresh profile, Add a source sets the key, the registry text is absent, and the About button reopens the card. 117 Rust and 88 frontend tests pass.

Release check. 0.26.4 was published as a signed release. A host install of 0.26.3 found 0.26.4, downloaded it, closed itself, let the installer replace the files, and the installer reopened 0.26.4. The app reported `You are up to date.` after the update, the library SHA-256 stayed `724969E7BE368118E48B73CFF5B021DB82099979E2648B17ED0DE0113F1E0FF4`, and uninstall kept the database. This is still a host result, not a clean VM result. It does close the Windows in-app update item for the fixed flow.

## 2026-11-10 Windows updater restart race

Context. The public feed went live, so the signed updater could be tested for real. A host install of 0.26.1 found 0.26.2, downloaded it, and showed Restart to apply.

What we measured. Clicking Restart started a new 0.26.1 process while a `Kaminari-0.26.2-installer` process was still running. The installed exe stayed at 0.26.1, the installer stayed alive, and the library hash was unchanged. Tauri's Windows updater launches the NSIS installer and exits the app. The UI passed `restartAfterInstall: false`, so the installer did not restart the new version, and the UI's Restart button raced the file replacement.

What we shipped. 0.26.3 passes `restartAfterInstall: true` on Windows and removes the Restart button there. The app closes, the installer replaces the exe, and the installer launches the new version. Non-Windows keeps the download and Restart path. The updater message is centralized in `updateInstallMessage` and covered by tests. A 0.26.1 or 0.26.2 install should use the 0.26.3 installer once for that jump, then use the fixed in-app flow.

Also in this pass: the public release feed returned 200 and 0.26.2 reported up to date. CI was fixed by tracking `src-tauri/lib/README.md` so the Tauri resource glob has a match in a clean checkout. The tag-triggered release workflow was removed because it cannot provision the local libmpv payload or sign with the local key.

## 2026-11-10 Secret scan, signing key move and local install test

Context. Before any public repository decision, the full git history needed a secret scan, the updater key needed a safer home, and the installer path needed an install, upgrade and rollback check. Windows Sandbox was the preferred clean VM, but it failed to start twice in this environment.

What we measured. Gitleaks 8.30.1 scanned 114 commits and about 2.18 MB. It found two matches: an `access_token` in an auth parser test and a `token` in a proxy documentation sample. Both were verified fake. The current test uses `test-token`, the doc uses `example-token`, and `.gitleaks.toml` allowlists only the two historical commits. The rerun scanned the same history and reported no leaks.

The updater private key, public key, password and signer output lived in `%APPDATA%\app.kaminari.desktop`, beside the SQLite library. They were moved to `%APPDATA%\kaminari-signing` with inherited ACLs removed and the current user granted full control. The app data folder now has zero signing files. `docs/RELEASE-UPDATER.md` uses the new path.

The host install test, after confirming no Kaminari install was present:

- 0.26.1 installer `/S`: exit 0, installed `kaminari.exe` reported 0.26.1.
- 0.26.2 installer `/S` over it: exit 0, version 0.26.2, `kaminari.db` SHA-256 unchanged.
- 0.26.1 installer `/S` as rollback: exit 0, version 0.26.1, database hash unchanged.
- Uninstaller `/S`: exit 0, install directory removed, database survived.
- Database SHA-256 before, after upgrade, after rollback and after uninstall: `724969E7BE368118E48B73CFF5B021DB82099979E2648B17ED0DE0113F1E0FF4`.

What remains. This was a host test, not a clean machine. Windows Sandbox produced a server process but no guest or result file on two attempts. The clean VM install, update and rollback pass stays open. The local result does prove the installer version path, the upgrade path, the rollback path, the uninstall path and that app data is not touched.

## 2026-11-10 Autoplay, recommendations and source settings

Context. Watching one episode ended at a dead screen, AniList detail pages showed no similar titles, and episode rows repeated the series art because the source returned no episode thumbnail. AniKoto already had a per-episode thumbnail switch, but the compatibility layer had no way to set it.

What we shipped. Autoplay next episode with a five second countdown, a Play next button and a Settings toggle. More like this reads AniList recommendations in the detail query and renders up to 12 clickable cards. The compatibility layer exposes `getSourcePreferences()` through a source settings dialog, stores values as `pref.<key>` and parses `"true"` and `"false"` as booleans. The episode row now shows the source thumbnail only; it no longer substitutes the series cover. The dialog supports switches and single choice lists, and explains that other types are not editable yet.

What we measured. AniKoto's Episode thumbnails switch saved `anikoto_pref_ep_thumbnails=true` and the next Naruto detail loaded 220 TVDB episode images with zero empty tiles. Autoplay reached the end card, showed a 3 second countdown, and started episode 2 at 12 seconds with no error. The Naruto detail returned 12 recommendation cards; clicking My Hero Academia opened its detail. 117 Rust and 88 frontend tests pass.

## 2026-11-10 AniList progress from source playback

Context. Manual episode marks on an AniList detail page enqueued progress, but watching a source show did not, because the source id is not an AniList id.

What we shipped. A Watch handoff now carries the AniList media into the source browser. When a linked episode is marked watched at 95 percent or at the end, the app calls the existing `watched_set` path with the source episode number, bounded by the AniList episode count. The queue is deduplicated per episode for the session. The source detail shows an AniList chip so the link is visible. Source shows opened directly from Sources stay unlinked.

What we measured. The queue and conflict path already have Rust tests. Live progress was not written to the connected account in this session because the project rule is not to exercise AniList writes against the user's account. The linked UI path needs an opt-in live check.

## 2026-11-10 Portable zip

Context. The NSIS installer is the normal distribution, but some users want a folder they can run without installing.

What we shipped. `scripts/package-portable.mjs` copies `Kaminari.exe`, the libmpv `lib` folder, `LICENSE`, `README.md`, `CHANGELOG.md` and a `PORTABLE.txt` into a versioned folder and zips it. It is exposed as `pnpm package:portable`. The build is installer-free, not data-free: settings and the SQLite library still live in the normal Windows app data folder, so replacing the folder keeps user data.

## 2026-11-10 Long HLS playlist stall

Context. Playback stopped around 28 seconds and the player offered a retry. The stream was a 23 minute VOD episode with 283 HLS segments.

What we measured. The proxy capped each stream at 256 child URLs. Reconciling the 283 segment playlist evicted the least recently used children to make room, which was the first 27 segments. The player had buffered them, then requested one after the buffer and got a 404 from the local proxy. The browser logged seven 404s for the same child and hls.js raised a fatal error.

What we shipped. The child cap is now 4096 and the playlist line cap is 20000, enough for a four hour VOD at four seconds per segment. A regression test reconciles a 283 segment playlist and asserts every child survives. A live AniKoto run played for three minutes and seeked to 600 and 1200 seconds with no proxy errors.

## 2026-11-10 AniList to source handoff

Context. Browse recommendations came from AniList but clicking Play only opened a note that playback needed a source. The user wanted a recommendation to open on a source.

What we shipped. Watch on an AniList detail, card or hero opens a modal. It lists enabled sources, searches each one with the title, and sorts the matches by normalized title score. Choosing a result opens the existing source browser at that show. The search box can be corrected without closing the modal. No source list is bundled, and a missing source shows an Open Sources action.

What we measured. A live Naruto handoff found the exact AniKoto and HiAnime results, opened AniKoto, and loaded 220 episodes. The matcher has unit tests for punctuation, exact, prefix, token and tie ordering.

## 2026-11-10 Mangayomi source shim

Context. The app had a working QuickJS host and a public-domain sample, but no real anime catalog. The user's core requirement is watching anime through user-supplied extensions, so a Mangayomi compatibility layer was the next feature after downloads and updates.

What we tried. The existing host was synchronous and rejected Promise results with `ASYNC_UNSUPPORTED`. A standalone probe evaluated a Mangayomi-style `DefaultExtension` after a bootstrap, called an async method, and drained the QuickJS microtask queue with `execute_pending_job`. The probe then ran the live AniKoto source against the production bridge shape.

What we measured. AniKoto preview found three anime sources in one 73 KB file. Search returned 26 Naruto results in 0.5 s, detail 0.6 s, 220 episodes 0.7 s, and resolve 6.1 s. Resolve returned two HLS variants with `Referer` and `User-Agent` headers from `fetch.nexabloom.top`. Episode 1 played 800p with 302 subtitle cues. HiAnime returned 28 results and 220 episodes; its ZokoAnime stream played at 800p. HiAnime's remote VTT needed the loopback file proxy and a conditional `crossorigin` video attribute. The native public-domain sample still returned 24 Popeye results and played after the same change.

What we shipped. Promise dispatch in `ext_host`, a wildcard host rule for compat sources, a random bytes bridge and 60 s search/detail timeouts with 120 s resolve. A bootstrap in `mangayomi.rs` provides `MProvider`, `Client`, `SharedPreferences`, `Document`, console, base64, UTF-8, URL, timers and crypto. The adapter maps the four native operations. The Sources page can inspect a raw URL, show metadata and SHA-256, and install only after an explicit trust prompt. Compat manifests use `hosts: ["*"]`; plain HTTP, localhost, private addresses and URL credentials stay blocked. Inline subtitles are converted to WebVTT; remote subtitles are proxied with stream headers. Episodes are sorted chronologically. The native sample remains on its original path.

Limits. Manga, filter UI, source preference UI and native WebAssembly are not supported. Some sources carry their own JavaScript WASM interpreter and work. `edl://` streams are an external mpv path. A shim update does not rewrite an installed wrapper; reinstall the source. Cloudflare challenges are not solved. No source list ships.

## 2026-11-10 Preferred playback quality

Context. A source can return several streams for one episode. The Play button used the first one, so a 480p dub could start ahead of a 1080p sub even when the user wanted a specific height.

What we shipped. A stored `kaminari.preferredQuality` preference with Auto, Best, 4K, 1080p, 720p, 480p and 360p choices. Settings has the full control and the show page has a quick selector. Auto keeps source order. Best picks the highest measured height. A numbered choice picks an exact match, then the highest below the target, then the lowest above it. Selection stays inside the first stream's server and audio variant, so changing quality does not silently switch from sub to dub. The preference is included in settings export and import.

What we measured. 75 frontend tests pass, including height parsing for `800p`, `1920x1080`, `1080p` and `4K`, exact and nearest selection, variant preservation, and settings fallback. A live AniKoto show selected a 720p stream and the player reported `HLS · 720p - HD-1 [Sub]`.

## 2026-11-10 Auto update

Context. Updates were manual installers. The updater needs a signed Tauri artifact and a hosted manifest.

What we shipped. `tauri-plugin-updater` v2 and `@tauri-apps/plugin-updater`, the `updater:default` capability, `bundle.createUpdaterArtifacts`, the local public key and the GitHub `latest.json` endpoint. Settings shows the current version, a Check for updates button, available version, date and notes, download progress, install, and Restart to apply. A quiet mount check treats an unreachable feed as a non-blocking warning.

What we measured. The 0.26.0 signed installer built at 36.58 MiB with a 440-byte `.sig` and a local `latest.json`. The release binary reported version 0.26.0 and the Settings check failed because the repository is private. 0.26.1 maps that missing manifest to a neutral state: `No public release feed is published for this build. Updates stay local until a signed release exists.` The app stays open. The installer and signature have not been run on a clean VM yet.

What remains. A signed public release with `latest.json`, a clean VM install, an update from an older signed build, and a rollback test. The private key stays outside the repository. `docs/RELEASE-UPDATER.md` records the build and publishing steps.

## 2026-11-10 Stream downloads

Context. The source browser could play a stream but not keep it. P3 asks for downloads to a user folder where the user has the rights.

What we shipped. A Rust download manager with a `download_start` command, `download_cancel`, and `downloads_list`, progress events, `.part` files and collision-safe names. Direct MP4 and OGV downloads, plus simple HLS media playlists. Master playlists pick the highest-bandwidth variant. Encrypted HLS, separate audio groups with a URI, and byte-range playlists fail with clear messages. The Source Browser has a Download button, a one-time content-rights confirmation, a folder picker saved in `localStorage`, and inline progress with cancel. Settings Data shows the downloads folder.

What we measured. The sample `popeye__2_.mp4` downloaded 791,922 bytes and ffmpeg read it as a 7.10 second H.264/AAC MP4. Progress moved from running to done, and the file landed under the chosen folder. 13 Rust download tests cover name sanitizing, collision suffixes, master selection, media playlist parsing, relative URLs and rejection paths. 110 Rust and 54 frontend tests pass. The Download button and downloads folder row render.

What we did not do. No live HLS or encrypted HLS download. HLS files may need external mpv. Downloads are in-memory for the session, with no resume, queue persistence or library integration. The 0.25.0 installer was not built.

## 2026-11-09 AniList write queue

Context. Read sync and account connect were done, but local status and progress changes had no path back to AniList.

What we shipped. `anilist_sync_queue` with one row per media. Status changes, watched changes and mark all enqueue the latest status and watched count, replacing the previous row and resetting attempts and last_error. Library add and remove do not enqueue. A 30 second background push and a manual Sync now use the keychain token, query the server entry, and apply one conflict rule: if the server entry is newer than the queued row, the server wins and the local row is dropped with a tracing warning; matching entries are dropped without a write; everything else calls SaveMediaListEntry. Network and rate errors keep the row and update attempts and last_error. Settings shows the pending count and a Sync now button.

What we measured. Unit tests cover enqueue and replacement, add/remove not enqueueing, watched progress, failure counters, conflict decisions, MediaList and mutation parsing, and status mapping. 97 Rust and 54 frontend tests pass. In the real app the queue was 0 before and after a read pull, pending was 0, a background tick after 35 seconds made no writes, and the connected account stayed untouched.

What we did not do. No live SaveMediaListEntry test, by request. The 0.24.0 installer was not built before parking; the 0.23.0 installer is the latest on disk. Downloads, auto update, and the P1 VM and legal checks remain.

## 2026-11-09 AniList read sync

Context. OAuth connect landed, but the local library had no path to pull the viewer's AniList list.

What we shipped. A `MediaListCollection(userId, type: ANIME)` query, an `anilist_sync_pull` command, a single SQLite transaction that upserts media and library rows, and status mapping for CURRENT, REPEATING, PLANNING, COMPLETED, PAUSED and DROPPED. Progress is clamped to the known episode count. Watched rows 1..progress use `INSERT OR IGNORE` and write no history rows. Local-only rows and local watched rows above server progress stay. Settings has a Sync from AniList button and refreshes the library through `onDataChanged`.

What we measured. The connected `graynb25` account has no anime entries, so the live pull returned 0 titles; that is the correct empty-list result. The local-only Tempal row stayed, history stayed at 2, and a second sync returned 0 again with identical counts. Unit tests cover two entries, status mapping, clamped progress, no history, idempotent re-sync and local-only survival. 87 Rust and 54 frontend tests pass. The 0.23.0 installer is 37,880,187 bytes (36.13 MiB); the installed build kept the connected viewer and returned 0 again.

What we did not do. No live non-empty pull because the account has no anime entries. The write queue and conflict handling are next.

## 2026-11-09 AniList OAuth connect

Context. P3 started with AniList read access. The app had public metadata only and no account state.

What we shipped. The AniList implicit grant with client ID 56484 and no client secret. The clean authorize URL omits `redirect_uri`; AniList uses the registered `kaminari://auth/callback`. Deep-link and single-instance plugins route the callback into Rust, Viewer validates it, and the token is stored in Windows Credential Manager under service `app.kaminari.desktop` and user `anilist`. Settings has an account block with the viewer name, avatar, Connect and Disconnect. No token reaches Svelte or localStorage.

What we measured. The clean URL without `redirect_uri` authorized; the earlier URL with `redirect_uri` produced `unsupported_grant_type` and a malformed browser URL. The real login returned viewer `graynb25`, id 8314048, with the default avatar. A synthetic callback with a fake token was rejected and did not overwrite the keychain; after the fix it leaves the account connected and shows an error line. 85 Rust and 54 frontend tests pass. The 0.22.0 installer is 37,878,262 bytes (36.12 MiB); an in-place install kept the keychain and the connected viewer.

What we did not do. No list pull or write sync yet. No explicit expiry timestamp; Viewer validation handles revoked or expired tokens. AniList has no refresh token, so expiry means reconnecting.

## 2026-11-09 Settings export and library backup

Context. P3 needed portable local data before sync. The library lives in WAL SQLite and had no UI path to copy or restore it.

What we shipped. A settings JSON with theme, accent and proxy. A SQLite backup through `VACUUM INTO` and a restore through `rusqlite::backup::Backup` with table validation and migrations. Native dialogs through `tauri-plugin-dialog`. Data actions for backup, restore, export and import, with a restart prompt after restore.

What we measured. The backup was 106,496 bytes with media 1, library 1, watched 2 and source_library 1. A test library row was added, the backup restored, the library returned to 1 and the test row was gone. A non-Kaminari SQLite file was rejected with the missing table names. The settings JSON was 162 bytes and round-tripped through export and read. The native Backup library dialog opened and cancelled cleanly, and the button re-enabled. 54 frontend and 78 Rust tests pass, with no page errors. The 0.21.0 installer is 37,745,278 bytes (36.00 MiB); the installed build reported the four Data actions and wrote a 106,496-byte backup.

What we did not do. The native file selection for import and export was not automated; the commands and dialog opening were tested. Restore needs a restart, and the backup is not encrypted.

## 2026-11-09 Accent presets

Context. The P2 plan left accent picker hooks open, and the app had one fixed violet accent.

What we shipped. Five presets: Violet, Ocean, Forest, Ember and Rose. A `data-accent` attribute on the root selects token sets for dark and light themes, and the poster, source card and player surfaces keep the dark accent values over art. Settings has a labelled swatch row with `aria-pressed`, the choice persists in `localStorage["kaminari.accent"]`, and `initAccent()` runs before mount. `parseAccent` falls back to violet for unknown values.

What we measured. Dark root `--volt-400` is `#8b8aff`, `#6ea8ff`, `#62c99a`, `#ff9457` and `#ff7fa4` for the five presets. Light Ocean resolves to `#2f6fd6`, and a poster art surface resolves Ocean to `#6ea8ff`. The selection persisted across reload. No horizontal overflow at 1280, 1024 or 900, and no page errors. 49 Vitest tests pass, including `parseAccent` for a known preset and an unknown one. The settings accent row is a text plus swatch control, not color alone. The 0.20.0 installer is 37,659,877 bytes (35.92 MiB), and the installed build resolved Ocean to `#6ea8ff`; the DLL payload is unchanged.

What we did not do. No custom color picker, only presets. No full WCAG contrast audit for every accent and text pair. Violet stays the default.

## 2026-11-09 500-item grid gate

Context. The P2 plan asked for a 500-item grid check before deciding whether the library grid needs virtualization.

What we shipped. A dev route at `?perfHarness=1` that mounts 500 real `PosterCard` components in the library grid with cached data-URL covers, no network images. It reports its mount time and exposes a scroll container for the measurement script.

What we measured. The harness mounted 500 cards in 68.5 to 76.4 ms. Scrolling the full grid height over 5 seconds at 1280, 1440 and 2560 held 169.2, 179.8 and 177.6 fps on an AMD Radeon RX 6600, with no frame over 20 ms at 1280 and 1440. At 2560 the p99 was 11.0 ms, the max was 22.2 ms, and the longest over-20 ms streak was one frame. A software-rendering pass on Microsoft Basic Render Driver at 1280 held 167.2 fps, p95 11.1 ms, max 11.2 ms and no frame over 20 ms. No run had a three-frame streak.

What we did not do. No integrated GPU laptop run. Software rendering is a conservative proxy, not the same hardware class. Virtualization is deferred because the gate passed without it.

## 2026-11-09 P1 packaging with bundled libmpv

Context. The maintainer allowed local bundling and a draft counsel packet. The installer now ships the two LGPL DLLs from the gitignored `src-tauri/lib` payload.

What we shipped. `bundle.resources` is `["lib/**/*"]`, `pnpm check:libmpv` verifies both pinned hashes, `beforeBuildCommand` runs the gate, the Native libraries group is marked bundled with hashes and the source offer, and `docs/LEGAL-REVIEW-DRAFT.md` holds the counsel packet.

What we measured. The 0.19.0 NSIS installer is 37,646,594 bytes (35.90 MiB). The release exe is 20,895,232 bytes. The native payload is 102,245,376 bytes uncompressed (101,853,696 plus 391,680). The installer installed per-user to `%LOCALAPPDATA%\Kaminari`; both DLL hashes in `lib` matched the pins. `mpv_embed_status` reported available true, dllDir the install `lib`, devDir null and lastError null. With `kaminari.libmpv` on, Popeye episode 1 played through the installed DLLs: 0:14 of 7:36, activeLabel main, no fallback toast. After close, activeLabel was null. Silent uninstall removed the install directory and both DLLs; the app data directory and SQLite file stayed.

What we did not do. No clean Windows 11 VM, update in place, rollback, code signing or SmartScreen pass. Public distribution waits for counsel review of the draft.

## 2026-11-09 Keyboard path and focus

Context. The no-mouse path from Library to the source browser, player and back worked, but Escape in the player closed both overlays and focus fell to the body. The source library card also had no visible focus ring because the card clips its own overflow.

What we shipped. PlayerView stops immediate propagation for every key it handles, so Escape, Space, K, J, L, arrows, M, F, T, V, comma, period, 0 and D stay inside the player while Ctrl K and other app shortcuts still pass through. SourceBrowser focuses the played episode after the player closes, and restores the library source card on close, with a `data-source-show` fallback when the original element is gone. Source library cards get a card-level focus ring. Episode Play buttons and source cards carry stable data attributes for the restore.

What we measured. Library to source card by Tab took 21 stops. Enter opened the source browser, 4 more Tabs reached Play episode 1, and Enter opened the player. Space paused and resumed, L moved 2.75s to 12.98s, K paused, and 1 did not change the Library nav while playing. Escape left the source browser open and focused Resume episode 1; a second Escape closed it and focused the source card. 47 Vitest tests pass and no page errors were logged.

What we did not do. No automated DOM test for the path; the verification is manual through CDP.

## 2026-11-09 System tray menu

Context. The app could only be reached through its window. Windows users expect a tray icon that can bring a background app back or quit it without hunting for the window.

What we shipped. A `tray.rs` module using Tauri's `TrayIconBuilder`, a native menu with Show Kaminari and Quit, and a left click that hides a visible window or shows and focuses a hidden one. The icon uses `default_window_icon`; the handle is stored in managed state. Tray setup failures log a warning and do not block startup. Window close still exits.

What we measured. The icon appeared in the Windows 11 overflow as a `SystemTray.NormalButton` named `Kaminari`. A UI Automation invoke hid the main window and a second invoke showed it again. The native menu opened at 2284,1177 with a 177 by 59 rect; the app exited during the menu probe, consistent with the Quit item. The popup exposes no UI Automation children, so the Show item was not invoked directly. Tooltip rendering, right-click at other DPI scales, and multi-monitor placement are not asserted.

What we did not do. No close-to-tray, no tray badges or playback controls, and no macOS or Linux tray check.

## 2026-11-09 Cascade layers

Context. `src/app.css` was unlayered, so base element rules and component classes silently beat Tailwind utilities. `hidden lg:inline-flex` never hid the online pill, `truncate` never truncated, and `text-xs` never set a button size.

What we shipped. Moved 260 rule blocks into `@layer base` and `@layer components`: 16 base and 244 components. Kept 36 blocks unlayered: focus rings, the overlay lock, the daylight hero, scrim and rank overrides, pointer and touch fallbacks, player route state, toast offset, text-stroke fallback, and the three reduced-motion blocks. Selector text, property order and values are unchanged.

What we measured. 316 rule blocks before and after, with none missing and none added. 14 rendered states at 1280, 1024 and 900 in both themes diffed against the 0.17.0 baseline. The visible changes are the intended utility wins: the online pill hides below 1024, long stat values truncate, and buttons take their declared text size. No overflow, clipping or page errors. Reduced motion reports a 0s page animation and no stagger or kenburns, a focused button keeps its 2px violet outline, and an open dialog locks `#content` to `overflow: hidden`. The palette, shortcuts dialog, source browser and in-app player were checked separately.

What we did not do. No values were rewritten, no `!important` was added, and P1 packaging still waits on lawyer sign-off.

## 2026-11-09 Settings navigation and FatalState

Context. Settings was one long page with no section navigation, and an app or view failure had no composed surface beyond a toast or a blank region.

What we shipped. A sticky Settings rail at 1024 and wider, a wrapping chip row below, section IDs and scroll margins, and active tracking with click override and bottom detection. The nav order follows the page: Appearance, App, Playback, Data, Updates, Licenses, About. The stat cards are a 2 by 2 grid and the section cards stack below 1280 so the narrower rail column stays readable. `FatalState` uses the brand mark, a mono error code, Retry and Copy diagnostics, and redacts URL query tokens, bare token assignments, and Authorization, Cookie and Set-Cookie values. It renders on boot snapshot failure and through a Svelte view boundary. The boot path does not claim to catch Rust or database failures before the WebView.

What we measured. At 1280 and 1024 the rail is visible, the chips are hidden, and there is no horizontal overflow. At 900 the rail is hidden and the chips are visible. Clicking Playback and About sets the correct active item, and a manual wheel scroll updates it again. The stat cards and section cards stay readable at the narrower content width.

What we did not do. Cascade layers. Pre-WebView Rust and database startup failures remain outside the Svelte boundary.

## 2026-11-08 Modal, Callout and ProgressBar atoms

Context. Each dialog carried its own focus trap, Escape handling, overlay lock, panel material and scroll shell. Callouts and progress bars were hand rolled with different tones and widths.

What we shipped. `Modal.svelte` owns the backdrop, glass panel, focus trap, Escape, focus restore, a shared overlay lock count, sticky header, scroll body and width variants. Legal, Licenses, Shortcuts and the search palette use it; SourceBrowser stays full screen. `Callout.svelte` has info, success, warning and danger tones with status or alert roles. `ProgressBar.svelte` handles determinate and indeterminate states with progressbar ARIA and a transform based fill. They replaced the offline banner, library and source errors, palette rate limit, registry and install progress, source search and resolve progress, and the detail episode bar.

What we measured. In the running app, typing "1" in the palette leaves the view unchanged, Escape closes only the palette, Legal keeps focus inside and returns it to Terms, Licenses autofocuses search and returns focus to Licenses, and hls.js filters to one row. The detail bar reports 2 of 12. The pass also found and fixed double-encoded middle dots and palette arrow keys. All 31 Vitest tests pass and the build passes.

What we did not do. Cascade layers, Settings navigation and FatalState. Those remain phase 3.

## 2026-11-08 Proxy cookie jar and live refresh

Context. Some sources set a session cookie on the playlist fetch that segments and keys then need. hls.js cannot see or forward those cookies, and the earlier proxy only forwarded the extension seed Cookie header.

What we shipped. A per-entry `Arc<Mutex<CookieJar>>`, memory only. It captures `Set-Cookie` on every upstream response, including redirects, playlists, keys, maps, segments and subtitles. It parses Domain, Path, Max-Age, Expires, Secure and SameSite, sends matching cookies on every request and redirect, and never sends Secure cookies over http. The cap is 32 cookies with 4 KB per cookie; the oldest is dropped first. The extension supplied Cookie seeds the jar once and is stripped from repeated request headers. The embedded dot domain rule is in place with a warning that a public suffix list is deferred. Revoke drops the entry and jar. Live playlists refetch on every request and keep child ids stable by URL plus byte range across sliding windows.

What we measured. 67 Rust tests pass, 43 proxy tests. Coverage includes a playlist cookie sent to a segment, expiry, Secure over http, cross-entry isolation, seed once, duplicate names, the cap, revoke, and three refreshes of a live sliding window. `httpdate` is now a direct dependency but was already in the lock.

What we did not do. No public suffix list, no browser SameSite enforcement, and no real cookie-gated live source yet. Manual QA with a real source remains.

## 2026-11-08 Phase 2 atoms

Context. Phase 2 had PageHeader and the artwork lifecycle. The remaining atoms were BrandMark, staggered grids, a static theme preview and a segmented control with an honest use.

What we shipped. BrandMark for the title bar, footer, empty state and Settings About, with duplicate mark styles removed. `stagger-grid` with 22ms steps capped at 18, applied to the library, source results and source cards. Wrappers animate, not the cards, so poster hover and disabled source opacity survive. A compact static Storm and Daylight preview uses each mini's own scoped theme tokens. `SegmentedControl` has a roving tabindex and Arrow, Home and End focus, and is used for the LegalDialog Terms and Privacy tabs. The EmptyState `mark` prop replaces the generic icon and the main library empty state uses it.

What we measured. 31 Vitest tests pass. Storm and Daylight resolve their own `--void-950` values, #08080f and #f6f7fc, in both active themes. ArrowRight focused Privacy and Enter kept it selected; Home and End moved focus. A staggered poster kept its hover transform matrix(1.03, -4). Settings shows three BrandMarks. The theme minis measure 210 by 109 and the Appearance card is 220px tall. No page errors.

What we did not do. No FatalState, settings navigation, or shared callout and progress primitives; those are Phase 3. SegmentedControl has one honest use, LegalDialog; no detail tabs or sort control exists yet.

## 2026-11-08 HLS master playlist proxy

Context. P1 handled media playlists, but many sources return a master playlist with variants, audio and subtitle groups and session keys. Master playlists returned 501 and fell back to mpv.

What we shipped. The parser now handles both master and media. Master rewrite covers next-line and inline `EXT-X-STREAM-INF`, `EXT-X-I-FRAME-STREAM-INF`, `EXT-X-MEDIA` audio and subtitles, `EXT-X-SESSION-KEY` and `EXT-X-SESSION-DATA`. Variants are child playlists; a request for a variant is rewritten as a media playlist. Depth is capped at 4 with a typed `playlist-depth` failure. Every child is checked against the extension manifest allowlist at rewrite time; a missing host returns a typed `child-not-allowed` failure. Relative URLs resolve against the final redirect URL. CORS, tokens and range behavior are unchanged.

What we measured. 55 Rust tests pass, 31 proxy tests. Coverage includes master variants, variant request rewrite, I-frame, session key and data, nested depth cap, allowlist pass and fail, and relative redirect base. The app boots with the frontend unchanged.

What we did not do. No cookie jar, no live playlist refresh beyond child reuse, no public suffix handling. P3 owns those. Cross-origin children are allowed only when the extension manifest lists them; a source that redirects to an undeclared CDN fails closed.

## 2026-11-08 HLS media playlist proxy

Context. P0 handled MP4 with headers. HLS needed the manifest and segment URLs rewritten so hls.js never leaves loopback and never needs custom headers. Keys, init maps and byte ranges had to stay on the same loopback path.

What we shipped. `/s/{id}/manifest.m3u8` and `/s/{id}/seg/{child}`. The parser rewrites segment URI lines, EXT-X-KEY, EXT-X-MAP, EXT-X-MEDIA, EXT-X-SESSION-KEY, EXT-X-PART, EXT-X-PRELOAD-HINT and EXT-X-SESSION-DATA, resolves relative URLs, substitutes EXT-X-DEFINE, keeps IV and BYTERANGE tags, and drops malformed or undefined URIs. Children are in memory, capped at 256, reused by upstream URL plus byte range, and carry their own content type and range. A playlist range is authoritative for that child, even if the client sends a different Range. Unranged children still forward the client Range. Playlist 2 MB, key 64 KB, subtitle 512 KB caps. Master playlists return 501 so the player falls back to mpv. CORS and token checks cover the new routes.

What we measured. 49 Rust tests pass, 25 proxy tests. Coverage includes relative rewrites, child reuse, header and range forwarding, two byte ranges on one URL, key and subtitle caps, malformed URI drop, the 2 MB cap, master rejection and CORS. 31 Vitest tests pass. The artwork visual pass rebuilt the app cleanly.

What we did not do. No master playlist rewrite, no cross-origin child allowlisting, no external `stream.subtitles` proxy, no cookie jar and no live playlist validation. Master and cross-origin children are P2; cookie jar and live are P3. An app-level HLS test still needs a real source whose manifest and CDN hosts are in the `.kext` allowlist.

## 2026-11-08 Artwork loading lifecycle

Context. Posters, continue cards, history thumbs, hero banners and the detail banner either showed a broken image icon or popped in without a reserved frame or loading state. The design spec asks for a blurhash promise that the app had not met.

What we shipped. `Artwork.svelte` reserves the frame, paints a shimmer, fades the image from a 16px blur when it decodes, checks `complete` and `naturalWidth` for cached and broken files, and renders a fallback snippet on error. Poster, continue, source continue, history, hero and detail use it. A broken hero or detail banner cascades to the blurred cover, then the void tile. Reduced motion shows the image immediately with no blur or transform transition. The wrapper owns the size, so decode cannot shift layout.

What we measured. 114 Artwork nodes rendered on Browse, 37 ready and 77 lazy below the fold. Loaded library and history images had no shimmer. A forced broken poster URL moved to `data-status="failed"`, removed the img and showed the initial letter. No horizontal overflow at 1024, 1600 or 2560. A full reload with a layout-shift observer measured CLS 0.0007 across one shift. After gating the shimmer to a 200px viewport margin, a 114-artwork harness dropped from 114 to 21 running shimmer animations, and the live Browse view showed zero at rest once images loaded. A synthetic 500-poster scroll averaged 7.6ms per frame, with one 111ms insertion frame. The artwork state machine has 10 Vitest tests.

What we did not do. No blurhash pipeline; CSS blur-up is the interim. No server-side image cache change.

## 2026-11-08 Loopback proxy for protected MP4

Context. MP4 streams that require `Referer`, `User-Agent` or a seed `Cookie` fail in the WebView because a `<video>` element cannot set request headers. External mpv already carries them through `--http-header-fields`. P0 adds a loopback path for the in-app player without widening the extension sandbox.

What we shipped. `src-tauri/src/proxy.rs` binds `127.0.0.1` on the first free port in `39321` to `39328`. If all eight are busy the server stays off and `proxy_status.enabled` is false. Entries live in memory only: 128 bit opaque id, 128 bit token, 2 hour idle TTL, 4 hour absolute TTL, 64 entry cap, 60 second sweep. `GET` and `HEAD /s/{id}/file` forward `Range` and relay `206`, `Content-Range`, `Content-Length` and `Accept-Ranges`. A 200 response to a range request is converted by skipping at most 16 MB, then a 206 is synthesized. A seek beyond that returns 502 instead of reading an unbounded body. Upstream bodies stream through 64 KB chunks with no full body buffer.

Security. Register checks the URL against the extension manifest host allowlist, then resolves DNS and blocks loopback, private, link-local, multicast, unspecified and `169.254.169.254` addresses. Each redirect is resolved and checked again, capped at 3 hops, and non http(s) targets are refused. The client pins the checked addresses with `resolve_to_addrs`. Headers are limited to `User-Agent`, `Referer`, `Origin`, `Cookie` (the seed value until the jar lands), `Authorization` (same origin only), `Accept` and `Accept-Language`, with CR, LF, NUL, count and byte caps. Every response carries `Access-Control-Allow-Origin: *`, `Access-Control-Allow-Headers: Range`, and exposes `Content-Range`, `Content-Length` and `Accept-Ranges`. A valid `OPTIONS` returns 204 with a 600 second max age and validates the same token as `GET`. CSP now lists the exact eight loopback origins in `connect-src` and `media-src`; a Rust test parses `tauri.conf.json` and fails on a missing origin, an extra loopback origin or a wildcard.

Frontend. `shouldProxy` is true only for MP4 with non-empty headers. HLS stays direct and falls back to mpv. The Settings Playback card toggles the `kaminari.proxy` localStorage flag, shows `127.0.0.1:<port>` with a Ready chip, and disables the toggle when `proxy_status.enabled` is false. `SourceBrowser` registers before play and revokes after the close progress save. Copy still uses the upstream URL. Player diagnostics show `Proxied: yes` or `no` and never include the token.

What we measured. On this machine the release `kaminari.exe` grew from 18,778,624 bytes at `af15951` to 19,529,216 bytes with P0, a delta of 750,592 bytes (733.0 KiB, 4.0 percent). The lock gained five crates: `axum`, `axum-core`, `httpdate`, `matchit` and `serde_path_to_error`. `futures-util` and `tokio-util` were already in the tree; both are direct dependencies now for the 64 KB stream wrapper. All 37 Rust tests pass, including 13 proxy tests for tokens, headers, ranges, the skip cap, redirects, SSRF, CORS and CSP. In the running app a WebView fetch of `/healthz` returned `{ ok: true, port: 39321 }`. An `archive.org` metadata URL through the proxy returned 200 and 3,062 bytes. Forcing the sample MP4 through the proxy returned 502 because `archive.org` redirects to `dn720308.ca.archive.org`, a host outside the sample manifest. Direct sample playback stayed at readyState 4 with no error. The installer size was not measured in this pass.

What we did not do. No HLS manifest or segment rewrite, no cookie jar or `Set-Cookie` capture, no live or master playlists and no subtitle proxy. `axum` needs the `tokio` feature for `axum::serve`, so the feature list is `http1`, `json`, `query` plus `tokio`. P1 starts with `/s/{id}/manifest.m3u8` and `/s/{id}/seg/{child}`.

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
