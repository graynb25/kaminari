# Native JS extension host plan

Status: M0, M1 and M2 are done. The host runs a real search against archive.org. M3 (source browse and detail) is next. No host code runs outside the bridge. This document is the contract we build against.

## Why this exists

Kaminari can read AniList and store an Aniyomi registry, but it cannot run a source. The Sources page is honest about that today. This plan adds a native JavaScript host so a `.kext` package can search a source, return shows and episodes, and resolve a stream.

This is not APK compatibility. Aniyomi extensions are Android/Kotlin bytecode and need a JVM sidecar. That is a separate decision and a separate repository. This host is for Kaminari's own packages.

## Decisions

These are the choices that shape the first release. Change one only with a written reason.

1. **Sync first.** Extension methods return plain values, not Promises. `net.fetch` blocks inside the host worker. QuickJS has no event loop by default, and a scraper does not need one. Promise support can land later without breaking sync packages.
2. **Fresh runtime per call.** Each operation creates a QuickJS runtime and context, evaluates the package, calls one method, and drops everything. This kills state leaks, makes timeouts trivial, and keeps one bad call from poisoning the next. Cold start is measured before we optimize.
3. **State lives in SQLite, not the runtime.** `storage.get` and `storage.set` write to a namespaced table, so a fresh runtime can still read what a previous call saved.
4. **Rust owns the bridge.** Extensions get `net.fetch`, `html.select`, `crypto.sha256`, `storage`, and `logger`. They do not get the filesystem, child processes, environment variables, raw sockets, or remote `eval`.
5. **HTTPS only, allowlisted hosts.** A manifest lists its hosts. Every request, including redirects, is checked against that list. Localhost and private ranges are blocked.
6. **Separate results from AniList.** Source results show in their own browse view first. Matching them to AniList entries is a later feature, not a requirement for the host to work.
7. **Unsigned packages first, signatures before a public registry.** Development installs are clearly marked sideloaded. Ed25519 signing and a registry land after the host is reliable.
8. **No APK execution in the main app.** If the JVM sidecar happens later, it stays a separate optional process.

## API v0.1

### Package

A `.kext` is a zip:

```
manifest.json
index.js
icon.png      optional
README.md     optional
```

Limits: archive 4 MB, `index.js` 2 MB, icon 256 KB, manifest 64 KB. Reject path traversal and more than 64 entries.

### Manifest

```json
{
  "id": "app.kaminari.publicdomain",
  "name": "Public Domain Animation",
  "version": "1.0.0",
  "apiVersion": "0.1",
  "entry": "index.js",
  "lang": "en",
  "baseUrl": "https://archive.org",
  "minHost": "0.3.0",
  "nsfw": false,
  "icon": "icon.png",
  "permissions": {
    "hosts": ["https://archive.org/*"],
    "storageKb": 512
  }
}
```

`id` is stable and reverse DNS. `apiVersion` is `0.1`. `hosts` entries are `https://host` or `https://host/path` with an optional `*` at the end. No wildcard subdomains in v0.1; add them only if a real package needs them.

### Provider

The package assigns one global:

```js
globalThis.kaminari = {
  search(query, page) { ... return { items, hasNext } },
  getShow(id) { ... return show },
  getEpisodes(showId) { ... return episodes },
  resolveEpisode(episodeId) { ... return streams }
};
```

All methods are synchronous. If a method returns a Promise, the host fails with `ASYNC_UNSUPPORTED` until v0.2.

Types:

```ts
type ShowSummary = {
  id: string;
  title: string;
  cover?: string;
  year?: number;
  score?: number;
  episodes?: number;
};

type ShowDetail = {
  id: string;
  title: string;
  cover?: string;
  banner?: string;
  synopsis?: string;
  year?: number;
  genres?: string[];
  status?: "airing" | "finished" | "upcoming" | "unknown";
};

type Episode = {
  id: string;
  showId: string;
  number: number;
  title?: string;
  thumbnail?: string;
  airedAt?: string;
  durationSec?: number;
};

type Stream = {
  url: string;
  type: "hls" | "mp4" | "dash";
  quality?: string;
  headers?: Record<string, string>;
  subtitles?: { lang: string; url: string; label?: string }[];
};
```

The host validates every returned object. A malformed row is dropped with a warning, not a crash.

### Bridge

- `net.fetch(url, { method, headers, body, timeoutMs })` returns `{ status, headers, body }`. Methods: GET and POST only. Body is a string. Default timeout 10 seconds. Response cap 5 MB. Redirects max 3, each checked against the allowlist.
- `html.select(html, selector)` returns `[{ text, html, attrs }]`, where `attrs` is a plain object. Backed by the Rust `scraper` crate, not a JS DOM.
- `html.attr(html, selector, name)` returns the first matching attribute or `null`. Convenience for simple pages.
- `crypto.sha256(text)` returns a hex string.
- `storage.get(key)` returns a string or `null`. `storage.set(key, value)` accepts a string, max 1 MB per extension. `storage.remove(key)`.
- `logger.info`, `warn`, `error` write to the app log with the extension id. No tokens, no response bodies.

### Errors

Throw an object with a code:

```js
throw { code: "NOT_FOUND", message: "Show is gone", retryable: false };
```

Codes: `NOT_FOUND`, `RATE_LIMITED`, `AUTH_REQUIRED`, `GEO_BLOCKED`, `PARSE_ERROR`, `HOST_DENIED`, `TIMEOUT`, `RESPONSE_TOO_LARGE`, `ASYNC_UNSUPPORTED`.

The host maps each to a typed error at the Tauri boundary and includes a short `diagnosticsId`.

## Runtime model

1. Look up the package by id in SQLite.
2. Create a QuickJS runtime.
3. Set memory limit 64 MB and max stack 1 MB.
4. Set an interrupt handler with a deadline. Search, show and episodes get 5 seconds. Resolve gets 10 seconds.
5. Create a context and inject the bridge.
6. Evaluate `index.js`.
7. Confirm `globalThis.kaminari` exists and the requested method is a function.
8. Call a host dispatcher that JSON-encodes the result.
9. Parse the JSON in Rust and validate the shape.
10. Drop the context and runtime.

The call runs on a blocking worker thread. It never blocks the Tauri command thread or the UI. One extension call can run at a time per source id in v0.1. Parallel searches across sources are queued or run in separate workers later.

Measured budgets:

- Cold runtime under 50 ms on this machine.
- Bridge overhead under 5 ms per call.
- Warm search under 2 seconds on a healthy source, excluding the site.

If cold start exceeds 50 ms, cache compiled bytecode before adding more features.

## Storage

New migration `0003_js_extensions.sql`:

```sql
CREATE TABLE js_extensions(
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  version TEXT NOT NULL,
  api_version TEXT NOT NULL,
  manifest JSON NOT NULL,
  code TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  trust TEXT NOT NULL DEFAULT 'sideloaded',
  installed_at INTEGER NOT NULL
);

CREATE TABLE js_storage(
  extension_id TEXT NOT NULL REFERENCES js_extensions(id) ON DELETE CASCADE,
  key TEXT NOT NULL,
  value TEXT NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY(extension_id, key)
);
```

The code is stored as text, not executed from disk. A package update replaces code and manifest but leaves storage. Removing a package removes its storage and logs.

## Install flow

1. User picks a `.kext` or presses **Install sample**.
2. Rust reads the zip, checks sizes and paths.
3. Parse and validate the manifest.
4. Check `apiVersion`, `minHost`, hosts, storage cap.
5. Store the package.
6. Sources page shows it with a `sideloaded` badge and the source count.
7. Enabling and disabling is a SQLite flag. It does not unload anything because runtimes are per call.

v0.1 does not download packages by URL. That waits for signing and a registry. The sample package is embedded in the binary for tests and first run.

## Sample package

`extensions/public-domain` is a real package that proves the host without piracy:

- Search: `https://archive.org/advancedsearch.php` with `collection:animationandcartoons`, `mediatype:movies`, and the query.
- Show: identifier, title, description, year, cover from `https://archive.org/services/img/<id>`.
- Episodes: `https://archive.org/metadata/<id>`, pick video files (`.mp4`, `.mpeg`, `.ogv`) and return one episode per file.
- Resolve: `https://archive.org/download/<id>/<file>` as an `mp4` stream.
- Hosts: `https://archive.org/*`.

This is public domain animation, not anime. It exists to prove search, details, episodes and resolve against a real site. The UI labels it as a demo.

## UI

Phase A, host proof:

- Sources page gets a **Native extensions** section above the Aniyomi registry.
- Buttons: **Install sample**, **Install .kext** (file picker), search/filter, remove.
- Each native package has a **Test search** action with a small result list and an expandable JSON view for diagnostics. This is a developer surface, clearly labelled.

Phase B, real browse:

- A package has a **Browse** action that opens a source view with a search box and a result grid.
- Selecting a result opens a source detail view with cover, synopsis, and episode list.
- Resolve shows stream count and quality. Playback is the next milestone.

Phase C, player:

- The existing `Play` note becomes a real resolve and player handoff.
- Stream headers and subtitles pass through to libmpv.

## Security

- The bridge is the only way out. No `fs`, `process`, `require`, `XMLHttpRequest`, `fetch`, or environment.
- Host allowlist is checked at request time and on every redirect.
- Block `http://`, `file://`, localhost, and private IP ranges. A real source should not need them.
- Response cap and timeout protect the app from a hostile or broken site.
- QuickJS interrupt handler stops runaway loops. Memory limit stops allocation bombs.
- This is in-process isolation, not an OS sandbox. A bridge bug is a Rust bug. Keep the bridge small and tested.
- Every installed package is visible with its host list. No silent installs and no bundled repos.
- Logs redact headers that look like cookies or authorization.

## Tests

Unit tests, no network:

- A package with static methods returns the expected JSON.
- Missing method returns a typed error.
- `net.fetch` to a non-allowlisted host returns `HOST_DENIED` before any request.
- Timeout fixture `while(true){}` is interrupted.
- Allocation fixture `new Array(1e8)` hits the memory cap or the timeout.
- `storage.set` then `storage.get` in two separate runtime calls returns the value.
- Malformed package returns a validation error.

Integration:

- Install the sample `.kext`, run a search, get episodes, resolve one stream. Marked manual and skipped in CI because it uses archive.org.
- A local fixture server could cover `net.fetch` and `html.select` in CI later.

CI stays offline. We do not want a flaky site to break a build.

## Milestones

**M0: plan and contract.** This document, the API note, and the decision record. Done when reviewed.

**M1: host core.** Runtime, bridge, JSON dispatch, limits, tests with inline packages. No UI yet.

**M2: install and Sources UI.** `0003` migration, `.kext` reader, built-in sample, install/remove, test search panel.

**M3: browse and detail.** Source result grid, source detail with episodes, resolve display. No playback.

**M4: player handoff.** Resolve streams, pick one, pass headers/subtitles to the player. Depends on the libmpv milestone.

**M5: signing and distribution.** Ed25519 manifests, trust prompt, registry JSON, `kaminari` dev CLI, update checks.

Acceptance for M1:

- Host runs a static inline package and returns typed results.
- Timeout, memory, host denial, and storage tests pass.
- No network in tests.
- `cargo clippy -D warnings` and the frontend build stay green.

Acceptance for M2:

- Sample package installs from the embedded bytes and from a `.kext` file.
- Test search returns real archive.org rows on this machine.
- Removing the package removes its storage.
- The Sources page shows trust and host details.

## Out of scope

- Android APKs, JVM sidecar, Android stubs, dex2jar.
- Torrents.
- Cloudflare solving. If a site needs a browser, it is not a v0.1 source.
- Background updates, downloads, sync.
- Arbitrary npm packages or a bundler.
- Signature enforcement before M5, but unsigned packages are labelled.

## Risks

- **Sync API limits authors.** Some APIs need async flows. v0.1 accepts that. The bridge is blocking, which is fine on a worker thread.
- **QuickJS memory limit** depends on the allocator feature. We must not enable `rust-alloc` or `allocator` features.
- **Scraper parity.** `html.select` is not a browser DOM. Some sites need JavaScript. Those sources are out of scope, not faked.
- **Site changes.** Every source breaks eventually. The host should return a readable `PARSE_ERROR`, not a blank page.
- **In-process risk.** A bridge escape is possible if the bridge is wrong. Keep it tiny, typed, and tested.
- **Cold start.** If it is slow, cache bytecode. Do not add a persistent runtime until measured.
- **Legal.** The host enables third-party sources. Ship no packages beyond the public domain sample, no registry, and a clear trust prompt.

## Open questions

1. Do we ship the sample package embedded, or as a `.kext` in `extensions/` that is installed on first use? Recommendation: embedded for tests, plus the folder as the authoring example.
2. Do we expose `score` and `year` as numbers only, or allow strings and coerce? Recommendation: coerce numbers, reject other types with a warning.
3. Do we allow source result covers from any https host, or require the cover host to be allowlisted? Recommendation: allow image hosts separately in the manifest, because covers often live on a CDN.
4. Do we take `sourceId` per extension, or one extension can ship many sources? Recommendation: one source per package for v0.1, multi-source later.

## What this unlocks

Once M1 and M2 are done, the app has a real extension runtime with a legal demo. M3 makes it useful to browse a source. M4 makes it play. M5 makes it distributable. None of this blocks the existing AniList, library, progress and history features.
