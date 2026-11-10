# Extension API

Status: draft v0.1. The native JavaScript host is live, and a real sample source ships in `extensions/public-domain`. Methods may be synchronous or return a Promise. The host drains the QuickJS microtask queue and waits for the result. See `docs/PLAN-JS-EXTENSIONS.md` and `docs/PLAN-MANGAYOMI-SHIM.md`. Breaking changes bump `apiVersion`.

## Package

A `.kext` file is a zip. It holds `manifest.json`, `index.js` (bundled, max 2MB), `icon.png` at 256px, `README.md`. Name reads `<id>-<version>.kext`. Install checks zip integrity, manifest schema, Ed25519 signature over canonical JSON, `apiVersion` compat, host allowlist parse, size caps. Any fail refuses with a code and a log line.

`manifest.json` fields:

```json
{
  "id": "com.example.hianime",
  "name": "Example Anime",
  "version": "1.2.0",
  "apiVersion": "0.1",
  "entry": "index.js",
  "lang": "en",
  "baseUrl": "https://example.com",
  "minHost": "0.1.0",
  "nsfw": false,
  "icon": "icon.png",
  "permissions": { "hosts": ["https://example.com/*"], "storageKb": 512 }
}
```

`id` is stable and reverse DNS. `lang` follows the source lang, `all` means multilingual. `permissions.hosts` is an allowlist. Widening hosts on update asks again. No silent escalation.

Redirects count as hosts. If a resolved stream redirects to a CDN or media host, list that host in `permissions.hosts` too. The loopback proxy rechecks every redirect hop and fails closed with a 502 when a host is missing.

## Contract

```ts
export interface AnimeProvider {
  readonly id: string;
  readonly name: string;
  readonly version: string;
  search(q: SearchQuery): ShowSummary[];
  getShow(id: string): ShowDetail;
  getEpisodes(showId: string): Episode[];
  resolveEpisode(ep: EpisodeRef): Stream[];
}
export interface SearchQuery { text: string; page: number; }
export interface ShowSummary {
  id: string; title: string; posterUrl?: string;
  rating?: number; episodes?: number;
}
export interface ShowDetail {
  id: string; title: string; cover: string;
  synopsis?: string; year?: number; genres?: string[];
  status?: "airing" | "finished" | "upcoming" | "unknown";
}
export interface Episode {
  id: string; showId: string; number: number; title: string;
  thumbnailUrl?: string; airedAt?: string;
}
export interface EpisodeRef { id: string; showId: string; number: number; }
export type StreamKind = "hls" | "mp4" | "dash" | "embed";
export interface Stream {
  kind: StreamKind; url: string; quality?: string;
  headers?: Record<string, string>;
  subtitles?: { lang: string; url: string }[];
  embedProxy?: boolean;
}
```

Methods may be synchronous or `async`. Throw a typed error with code and message. The host validates results at the boundary and drops bad rows with a log, never a crash.

Bridge globals, frozen: `net.fetch(url, opts)` with timeouts and header presets. `html.query(doc, selector)` for simple parsing. `crypto.sha256(text)` plus hmac and random. `storage.get` and `storage.set` namespaced per extension. `logger.info`, `warn`, `error`. No filesystem. No child process. No env. All bridge calls are synchronous in v0.1.

## Error model

Use `{ code, message, retryable }`. Codes: `NOT_FOUND`, `RATE_LIMITED`, `AUTH_REQUIRED`, `GEO_BLOCKED`, `EMBED_ONLY`, `PARSE_ERROR`, `HOST_DENIED`, `TIMEOUT`. Keep messages short. The UI shows them as is. Full traces stay in the per source log viewer.

Timeout is 5 seconds, 10 for resolve. Memory cap is 64MB. One bad extension greys out its source. The app stays up.

## Mangayomi compatibility

Kaminari can wrap a raw Mangayomi JavaScript anime source without repacking it. The Sources page downloads the file, shows its `mangayomiSources` metadata and SHA-256, and asks for explicit trust. Install re-downloads the file, verifies the hash and stores a wrapped copy.

The wrapper provides `MProvider`, `Client.get/post`, `SharedPreferences`, `Document`, `Element`, `console`, `window`, `navigator`, `location`, `fetch`, `btoa`, `atob`, `TextEncoder`, `TextDecoder`, `URL`, `URLSearchParams`, `crypto.sha256`, `crypto.getRandomValues` and microtask timers. It maps `search`, `getDetail`, `getVideoList` and the `chapters` array onto the native operations. Episode ids are chapter URLs.

Supported subset: anime only, no filter UI, no source preference UI, no native WebAssembly, no filesystem or process access. Sources may carry their own pure JavaScript WASM interpreter. Inline SRT or WebVTT is converted to WebVTT and loaded from a Blob track in the WebView player. Remote subtitle files are proxied with the stream headers. `edl://` streams are for external mpv.

Host policy: a compat manifest uses `hosts: ["*"]`. Plain HTTP, localhost, private addresses and URL credentials stay blocked. The trust panel states that the source can contact any host and send it what it reads. The host has no access to the Kaminari database, AniList token, files or environment.

Source settings: if the source defines `getSourcePreferences()`, the Sources page exposes a Settings dialog. Switch and single choice list preferences are editable and stored as `pref.<key>` in the extension namespace. `SharedPreferences.get` returns a boolean for the stored strings `"true"` and `"false"`, so switch checks work as written. Multi select and free text preferences are shown but are not editable yet. Source changes apply the next time the show is opened.

## Registry

A registry is a signed JSON file with a list of extensions. Each entry holds id, name, version, min host, download URL, sha256, signature. Kaminari pins the key per registry URL. Trust levels read signed, sideloaded, disabled. NSFW hides by default behind a setting.

### Registry compatibility: Aniyomi style index

We stay compatible with the Aniyomi community index shape at `https://raw.githubusercontent.com/yuzono/anime-repo/repo/index.min.json`, so existing catalogs can convert without rework. That file is a JSON array. Each item looks like this:

```json
{
  "name": "Aniyomi: AnimeOnsen",
  "pkg": "eu.kanade.tachiyomi.animeextension.all.animeonsen",
  "apk": "aniyomi-all.animeonsen-v14.10.apk",
  "lang": "all",
  "code": 10,
  "version": "14.10",
  "nsfw": 0,
  "torrent": 0,
  "sources": [
    { "name": "AnimeOnsen", "lang": "all", "id": "8542735178285060053", "baseUrl": "https://www.animeonsen.xyz" }
  ]
}
```

Measured 2026-11-08: 250 packages, 304 sources, ids are decimal strings that exceed JS safe int. Sample `8542735178285060053` is larger than 2 to the 53. Full note lives in `docs/ENGINEERING-NOTES.md`.

Current app status: the Sources page fetches this index live, shows all 250 packages with NSFW and torrent badges, and stores selected entries in SQLite as metadata. It does not download an APK or execute package code. The QuickJS host is the next step.

Mapping to Kaminari:

- `pkg` maps to our extension `id`. Keep the full reverse DNS string. Example: `eu.kanade.tachiyomi.animeextension.all.animeonsen`.
- `apk` maps to our `.kext` artifact name plus `downloadUrl` and `version`. Parse version from the file name or the `version` field. Keep `code` as the upstream build number for update checks.
- `name`, `lang`, `version`, `nsfw`, `torrent` map straight across. `torrent: 1` marks a source the desktop client will not auto enable. It needs explicit opt in.
- `sources` array maps to our multi source entries inside one extension. One package can ship many langs or mirrors. Each entry keeps its own `name`, `lang`, `baseUrl`, and string `id`.
- Source `id` stays a string on every boundary: Rust TEXT, JSON string, TS string. Never parse to number. Never store as float. Convert at import and validate with a digits only check. Display the short tail in logs to keep lines readable.

Import converter sketch (Rust side):

```rust
// pseudo, real code lands with scaffold
struct LegacySource { name: String, lang: String, id: String, base_url: String }
fn import_legacy(pkg: &str, apk: &str, version: &str, s: LegacySource) -> SourceRow {
  assert!(s.id.chars().all(|c| c.is_ascii_digit()));
  SourceRow { ext_id: pkg.into(), artifact: apk.into(), version: version.into(),
    source_id: s.id, // TEXT, never INTEGER
    name: s.name, lang: s.lang, base_url: s.base_url }
}
```

Kaminari native registry entries add three fields the legacy shape lacks: `downloadUrl` for the `.kext`, `sha256` of the file, `signature` Ed25519 over canonical JSON. Legacy import fills `downloadUrl` from the mirror host plus `apk` name, then requires a fresh signature before it can publish as signed. Until then it installs as sideloaded with a permission prompt.

Revocation is a `revoked: [signature]` list. Client disables on next sync and toasts once per extension.

## Start from the template

`extensions/template` holds a working skeleton: `manifest.json`, `index.js`, `README.md`. Copy the folder, change `id`, `name`, `baseUrl` and `permissions.hosts`, then replace the four method bodies. Keep the methods synchronous and keep ids as strings.

Zip from inside the folder so the files land at the zip root, and name the archive `<id>-<version>.kext`:

```powershell
Compress-Archive -Path manifest.json, index.js, README.md -DestinationPath pack.zip -Force
Rename-Item pack.zip com.example.template-0.1.0.kext
```

Install it from the Sources page and run a search in the Sources test panel. `node --check index.js` catches syntax mistakes before you pack.

## Tooling

```bash
npm create kaminari-ext ./my-source
kaminari dev ./my-source
kaminari test ./my-source
kaminari pack ./my-source
kaminari publish ./my-source
kaminari keys gen
```

`dev` runs the sandbox locally with limits enforced and hot reload. `test` runs fixtures for search, show, episodes, resolve plus offline and 429 mocks. `pack` writes `.kext` plus sha256. `publish` signs and opens a registry PR. Template passes `create` to green `test` in under 2 minutes on a clean machine.

## Example

The package assigns one global. Bridge calls are synchronous in v0.1.

```ts
globalThis.kaminari = {
  search(q) {
    const res = net.fetch(`https://example.com/search?q=${encodeURIComponent(q.text)}`);
    const cards = html.select(res.body, ".card");
    return {
      items: cards.map((el) => ({
        id: el.attrs["data-id"], // keep string, may exceed 2^53
        title: el.text,
      })),
      hasNext: false,
    };
  },
  resolveEpisode(ep) {
    return [{ url: `https://example.com/stream/${ep.id}.m3u8`, type: "hls", quality: "auto" }];
  },
};
```

Non goals for v0.1: background downloads inside JS, torrent transports, DRM Widevine, raw iframes in WebView. Embed flows go through `embedProxy` in Rust so the WebView never hosts third party script.
