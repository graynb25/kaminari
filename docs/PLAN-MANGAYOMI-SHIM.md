# Mangayomi shim plan

Status: shipped in 0.26.0 on 2026-11-10. The feasibility probe passed against the live AniKoto source: search returned 26 shows, detail returned 220 episodes, and resolve returned two HLS streams with headers. The in-app test then played AniKoto and HiAnime with subtitles. Nothing in this plan changed the native `.kext` contract.

## Why

Kaminari runs its own `.kext` sources, but the catalog is empty unless someone writes them. The Mangayomi community already has JavaScript anime sources. Running a documented subset of that API turns those files into real sources without bundling a catalog.

This is not Aniyomi APK compatibility. It does not ship source code, an index, or a service. The user installs a third party file and accepts a trust prompt.

## What the probe proved

The current QuickJS host is synchronous, but the extension API is Promise based. The bridge stays synchronous and the host drains QuickJS microtasks after the call:

1. Evaluate the bootstrap that defines `MProvider`, `Client`, `SharedPreferences`, `Document`, polyfills and the adapter.
2. Evaluate the extension. Its top level `class DefaultExtension extends MProvider` and `const mangayomiSources` live in the same script scope.
3. Instantiate the class, assign `provider.source = mangayomiSources[index]`, and expose `globalThis.kaminari` with the four native operations.
4. Run the dispatch as an async IIFE. Rust gets a `Promise`, calls `ctx.execute_pending_job()` while it is pending, and reads the JSON envelope when it resolves.
5. Network calls in `Client` are synchronous host calls. `await` only yields microtasks, so no async Rust or event loop is needed.

The probe ran AniKoto against `https://anikototv.to` with the production bridge shape. It resolved `fetch.nexabloom.top` HLS variants and their required `Referer` and `User-Agent`.

## Supported subset

Native operations map to Mangayomi methods:

| Kaminari | Mangayomi | Mapping |
| --- | --- | --- |
| search | `search(query, page, [])` | `{ list: [{ name, link, imageUrl }], hasNextPage }` to `{ items: [{ id, title, cover }], hasNext }` |
| getShow | `getDetail(link)` | `{ name, imageUrl, description, genre, status }` to `JsShowDetail` |
| getEpisodes | `getDetail(link)` | `chapters[]` to `JsEpisode[]`; episode id is the chapter URL |
| resolveEpisode | `getVideoList(chapterUrl)` | `{ url, quality, headers, subtitles }[]` to `JsStream[]` |

Runtime globals provided by the bootstrap:

- `MProvider`, `Client.get/post`, `SharedPreferences`, `Document`, `Element`
- `console`, `window`, `navigator`, `location`
- `btoa`, `atob`, `TextEncoder`, `TextDecoder`, `URL`, `URLSearchParams`
- `crypto.sha256` and `crypto.getRandomValues`
- microtask based `setTimeout`, `clearTimeout`, no-op `setInterval`
- `fetch` as a Response-like wrapper over `Client`

Not provided: native `WebAssembly`, WebCrypto `subtle`, filesystem, process, environment, cookies, or credentials. Extensions that carry their own pure JS WASM interpreter work. ReAnime, OneAnime and Senshi do.

Known limits in the first release:

- Anime only. Manga sources are rejected.
- No filter UI. Searches use an empty filter array.
- No source preference UI. `SharedPreferences` returns null and stores values set by code, so defaults apply.
- Inline subtitle text is dropped. Only subtitle URLs are forwarded.
- `edl://` streams are passed through to external mpv only; the WebView player cannot play them.
- Some sites need a browser challenge. Kaminari sends no browser cookies and does not solve Cloudflare.
- An updated extension is installed by the user again. No auto update yet.

## Security model

A Mangayomi source is third-party code. It gets network access and a namespaced preference store. It does not get the database, AniList token, files, child processes or environment variables.

Install is explicit:

1. The user pastes a raw JavaScript URL and selects Inspect.
2. Kaminari downloads the file, caps it at 2 MB, computes SHA-256, evaluates only the metadata, and shows name, version, language, base URL and source URL.
3. The confirm panel says the code can contact any host and send it what it reads, and that only trusted sources should be installed.
4. Install re-downloads the file, verifies the same SHA-256, and stores the wrapped code as sideloaded with the host `*`.
5. A changed file at the same URL is a new trust decision. The hash is shown.

The manifest allowlist for compat sources is `*`. The existing network guard still rejects plain HTTP, localhost, private addresses and credentials in URLs. The loopback proxy applies the same wildcard rule to stream hosts and rechecks every redirect and child playlist.

## Files

- `src-tauri/src/mangayomi.rs`: bootstrap JavaScript, metadata preview, wrapper builder, mapping adapter and tests.
- `src-tauri/src/ext_host.rs`: Promise dispatch, wildcard host rule, random bytes bridge, compat timeouts.
- `src-tauri/src/commands.rs`: `mangayomi_preview` and `mangayomi_install`; reuse existing extension commands.
- `src-tauri/src/db.rs`: list rows mark `kind` as `mangayomi` from the id prefix; no migration.
- `src/lib/SourcesView.svelte`: inspect, trust and install panel; Mangayomi badge.
- `src/lib/api.ts`, `src/lib/types.ts`: preview and install calls, source types.
- `docs/EXTENSION-API.md`: async support and compat mode.

## Milestones

1. Host: Promise dispatch, wildcard hosts, random bridge, compat timeouts. Tests: async search succeeds, wildcard allows HTTPS and rejects HTTP.
2. Shim: bootstrap, adapter, metadata preview, wrapper builder. Tests with a stub extension cover all four operations and source selection.
3. Install: preview and install commands, SHA-256 pin, no bundling.
4. UI: inspect panel, trust copy, install and badge.
5. Live test: AniKoto search, detail, episodes, resolve and playback. Then a second source if reachable.
6. Release: 0.26.0 with auto update and the shim.

## Acceptance

- A user can install an AniKoto raw URL without editing a file. Done on the Sources page.
- Browse finds shows, a detail page lists episodes, Play opens a resolved HLS stream. Done live for AniKoto and HiAnime.
- Removing the extension removes its stored code and preferences. Covered by the extension removal path and storage test.
- No source list or source code ships with the app. The repo contains no source list. The two live sources were installed by hand for the test.
- Native `.kext` sources and the public-domain sample keep working. Rechecked after the Promise dispatch change: 24 Popeye results and playback.
- Rust and frontend tests, fmt, clippy and the style check pass. 117 Rust tests and 88 frontend tests passed at the 0.26.4 check.

## Measured results

- AniKoto: preview found 3 anime sources in one file, search 26 results in 0.5 s, detail 0.6 s, 220 episodes 0.7 s, resolve 6.1 s, two HLS variants with headers. Episode 1 played 800p with 302 subtitle cues.
- HiAnime: 28 search results, 220 episodes, ZokoAnime 800p HLS playback. Its remote VTT needed the loopback proxy and a conditional `crossorigin` video attribute; both landed.
- Native sample: 24 Popeye results. Playback still works after the conditional cross-origin change because the sample has no subtitle tracks.
- Source settings: AniKoto exposes an Episode thumbnails switch. Turning it on produced 220 distinct TVDB thumbnails for Naruto and no series art fallbacks.
- Autoplay: Naruto episode 1 ended, the card showed a countdown, and episode 2 started with no error.
- Recommendations: the Naruto detail returned 12 More like this cards; clicking one opened that title.

## Deferred

- Shim updates do not rewrite an already installed wrapper. Reinstall the source after a shim change.
- No filter UI, source preference UI, manga, native WebAssembly or Cloudflare challenge handling.
- `edl://` streams remain an external mpv path, not a WebView path.
- The update endpoint is still private. A signed installer can be built locally, but publishing `latest.json` is pending.
