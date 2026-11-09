# Loopback stream proxy for the in-app player

Status: planning only. No code changed. This doc is the build spec.

Goal: play header protected MP4 and HLS in the WebView without turning the app into an open proxy and without weakening the extension sandbox. Rust owns fetch, headers, cookies and manifest rewrite. Svelte only sees loopback URLs with an opaque id and a per entry token.

Non goals: DASH, DRM Widevine, torrent transports, background downloads, general purpose fetching from the WebView. Those stay out.

## 1. Problem statement and current flow

Sources return streams that need HTTP headers, cookies, or both. The WebView cannot send them for `<video>` or for hls.js segment fetches. Result: direct playback fails for protected origins, and HLS with per segment headers fails entirely.

Current flow, exact files and functions:

1. `src/lib/SourceBrowser.svelte`, `resolve(episode)` calls `jsExtensionResolve(extension.id, episode.id)` from `src/lib/api.ts`, which invokes `js_extension_resolve` in `src-tauri/src/commands.rs`. That calls `run_js_extension` with op `resolveEpisode` into `src-tauri/src/ext_host.rs`, `run()`. The extension returns `Stream[]` shaped by `docs/EXTENSION-API.md`: `kind` (`hls`, `mp4`, `dash`, `embed`), `url`, `quality`, `headers`, `subtitles`, `embedProxy`.
2. `src/lib/SourceBrowser.svelte`, `playStream(stream, episode)` sets `activePlayback` and mounts `src/lib/PlayerView.svelte` with prop `stream: JsStream` (`src/lib/types.ts`, `JsStream`). No proxy step exists.
3. `src/lib/PlayerView.svelte`, `setup()` branches on `stream.type === "hls"` or a `.m3u8` regex. MP4 sets `video.src = stream.url`. HLS lazy imports `hls.js`, calls `hls.loadSource(stream.url)` and `attachMedia(video)`.
4. Headers are ignored in app playback. They are only used on the external path: `src/lib/SourceBrowser.svelte`, `openExternal()` calls `playerPlay()` in `src/lib/api.ts`, which invokes `player_play` in `src-tauri/src/commands.rs`, which calls `PlayerState::play()` in `src-tauri/src/player.rs`. `build_args()` maps headers to `--http-header-fields` after stripping CR and LF. Subtitles pass as `--sub-file`.
5. CSP in `src-tauri/tauri.conf.json`, `app.security.csp`: `default-src 'self'; connect-src 'self' asset: http://asset.localhost ipc: https: blob:; img-src 'self' asset: http://asset.localhost https: data:; media-src https: blob: http://asset.localhost; style-src 'self' 'unsafe-inline'; script-src 'self'`. Direct `https:` media loads pass. Custom headers do not.

So the gap is precise: `PlayerView.setup` needs a URL it can load without custom headers, while Rust attaches the real headers, cookies and manifest fixes upstream. The proxy fills that gap with per entry loopback URLs.

Concrete symptoms today: a stream with `Referer` or `User-Agent` requirements plays in external mpv but stalls or 403s in app. An HLS master with relative segment URLs or `EXT-X-KEY` fails because hls.js fetches the key and segments without the source headers.

## 2. Threat model

Assumptions: extensions are untrusted JavaScript. The WebView may host attacker influenced strings (titles, URLs). Other processes may run on the same machine. The proxy must not become a general fetch oracle.

Principle: the proxy fetches only URLs registered through a Tauri command, only with headers from the allowlisted resolve result, only from the looped back WebView holding a fresh token. It never proxies an arbitrary URL supplied in an HTTP request line or query string.

| Threat | Severity | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| Loopback exposure, another local app or page hits the proxy | H | M | Plays protected content, burns source session, leaks bytes | Bind `127.0.0.1` only, never `0.0.0.0`. Require 128 bit token per entry as query param, constant time compare. Bounded TTL (2 h idle, 4 h absolute) and one show scope. Log every 403 without logging the token. |
| SSRF, extension tricks proxy into hitting intranet or cloud metadata | S | M | Reads `169.254.169.254`, router admin, local services | Upstream allowlist: only the resolved origin plus origins seen in its own rewritten manifests, each rechecked against the extension host allowlist. Block literal IPs in private, loopback (except the proxy itself, which is never a valid upstream), link local and multicast ranges, plus `169.254.169.254` by name and by IP. Resolve DNS then recheck the IP before connect. No redirects to a blocked host. Cap redirects at 3. |
| DNS rebinding, DNS flips from public IP to loopback between check and fetch | H | L | Same as SSRF, bypasses name check | Pin the resolved IP for the entry lifetime. Use a reqwest client with a custom resolver or resolve once in Rust and connect to that IP with correct SNI and Host. On redirect, repeat resolve plus check. The 2 h idle plus 4 h absolute TTL bounds the window. |
| Open proxy abuse, WebView or page crafts `/fetch?url=https://evil` | S | M | App becomes a relay for third parties, credentialed fetches | No URL parameter is ever honored. Route is `/s/{opaqueId}/...` with the upstream stored server side. Unknown id gives 404. Missing or wrong token gives 403. No forward proxy mode, no CONNECT. Fuzz the router for parameter smuggling. |
| Token leakage via logs, diagnostics, clipboard | M | M | Rated M, not H, because the token is per entry, loopback only, and bounded by the entry TTL. Anyone with the token plays one stream until expiry, not other sources | Tokens live only in memory. Never log token or full proxy URL. Diagnostics copy prints host `127.0.0.1`, stream type and opaque id prefix (first 6 chars), never the token. Clipboard copy of a proxied URL is disabled, copy shows the upstream host only. |
| Redirect abuse, upstream 302s to file, internal host, or oversized target | M | M | SSRF bypass, large download, protocol downgrade | Follow at most 3 redirects inside Rust. Reapply host allowlist and IP blocklist on each hop. Refuse protocol change from https to http except to the same registrable domain when the entry allows it, and log it. Refuse non http(s) schemes. Preserve only safe headers on redirect, drop Authorization unless same origin. |
| Header injection from extension strings | M | M | Request smuggling, log forgery, mpv arg breakout already handled | Strict header allowlist (see section 4). Reject names or values with CR, LF, or NUL. Reject `Host`, `Content-Length`, `Transfer-Encoding`, `Connection`, `Upgrade`, `Proxy-*`. Cap count at 16 and total bytes at 8 KB. `player.rs`, `build_args` already strips CR and LF for mpv, reuse the same sanitizer. |
| Cookie leakage across extensions or shows | H | L | Session theft within one source. Rated H, not S, because the jar is per entry and keyed by `(extensionId, topOrigin)`, so a bug leaks within one source session. It would become S if the jar were ever shared across extensions, which this design forbids | Cookie jar is per entry, never global. Never send cookies from source A to source B. `Cookie` header from the extension does not merge with the jar without explicit opt in. Clear jar on playback close and on source disable. Never persist cookies to SQLite. Never expose `Set-Cookie` values to JS. |
| CSP widening to loopback | M | H if wildcard, L if exact | Rated M with the exact 8 port list, because the WebView can reach only ports the app itself binds. It would be H with a wildcard port | Do not add `http://127.0.0.1:*` or `http://localhost:*`. List exact origins with fixed ports (see section 8). Keep `media-src` and `connect-src` additions scoped to those origins only. |
| Malicious playlists, huge tags, nested playlists, `file:` or `data:` URIs | M | M | Parser DoS, local file read attempt, infinite recursion | Parse playlists line by line with a 2 MB cap. Allow only http and https segment, key, map and subtitle URIs after resolution. Reject `file:`, `data:`, `blob:` inside upstream manifests. Cap playlist depth at 4 and segment count at 2000 per fetch. On violation, fail the entry with a typed error, keep the player up. |
| Decompression bombs (`gzip` playlist that expands 100x) | M | L | Memory exhaustion | Disable automatic decompression for playlist fetches or cap with a streaming decoder and a 4 MB expanded limit. Segments pass through as bytes, never decompressed in Rust. Reject `Content-Encoding` other than identity for segments unless the content type needs it. |
| Unbounded bodies (endless live segment or multi GB MP4 pulled into RAM) | H | M | OOM, disk fill | Never buffer full bodies. Stream with bounded chunks (see section 7). Enforce per response byte cap for playlists (2 MB) and keys (64 KB). For media, enforce idle timeout and client disconnect cancellation, no full body cap but no buffering either. Return 502 on upstream overrun for playlists, 504 on timeout. |
| Healthz port probe by local software, no token | L | M | A local probe learns the proxy port is open. No content, token, or cookie is exposed | `GET /healthz` returns only `ok` plus the port. No entry data, no counts, no URLs. Rate limit to one log line per minute. |

Scale: S means systemic. The bug escapes one stream and affects other apps, the machine, or all sources at once (open relay, intranet read). H means serious but bounded to one user session, such as session theft within one source or a credential leak that outlives the entry. M means confined to one playback session, such as a single expired token or one blocked playlist. L means cosmetic or noisy with no credential, bypass, or playback break, such as a counter glitch or a port probe with no auth impact. Likelihood is H, M, or L for this deployment. Impact names the concrete loss.

## 3. Architecture options

Constraints from `AGENTS.md`: Rust owns network. Svelte owns animation. No sidecars without a measured reason. Current deps in `src-tauri/Cargo.toml`: `tokio` with `full`, `reqwest 0.12` with `rustls-tls-webpki-roots, json, stream, gzip, blocking`, `tauri 2` with `custom-protocol`.

| Option | Dependency weight | Async fit | Range support | Testability | Maintenance |
|---|---|---|---|---|---|
| axum 0.8 on tokio | One direct dep plus transitive `hyper 1`, `tower`, `mime`. Small versus libmpv. Acceptable under the 15 MB installer budget, must be measured with `cargo bloat` | Native. Runs on the existing tokio 1 runtime. Handlers are async, streaming via `Body::from_stream` | Manual but simple: read `Range`, forward to reqwest, map 206 plus `Content-Range`. Full control | High. Router is a value. Spin it on an ephemeral port in `#[tokio::test]` and hit it with reqwest. No WebView needed | Low. Tokio team maintains it. Most examples match our shape |
| hyper 1.x directly | Lighter than axum by a little (no router, no tower layers) | Native async | Same manual work as axum, plus manual routing | Medium. More harness code per test | Medium. We own routing, method dispatch, 404 shapes |
| tiny_http | Tiny dep tree, sync API | Poor. Blocking accept loop on its own thread, fights the tokio design. Needs channels to bridge | Manual, sync reads | Low. Threading in tests, harder cancellation | Medium. Unmaintained pace, fewer security updates |
| Raw tokio `TcpListener` plus hand rolled HTTP | Zero new deps | Native but everything is manual: keep alive, chunked framing, pipelining edge cases | Hand parsed, high bug risk | Low. We would test our own HTTP server, which is the wrong test to own | High. HTTP edge cases are a permanent tax |
| Tauri custom protocol (`asset:` or registered scheme) | Zero TCP surface, zero new HTTP dep | Callback based, not a normal async server. Range must be emulated in the protocol handler | Weak. Protocol handlers get a request and return bytes. Partial content needs manual slicing and header synthesis, and hls.js behavior on custom schemes is uneven | Medium. Needs a running WebView for full coverage | Medium. Tied to WebView quirks per platform, harder to debug with curl |

Recommendation: axum 0.8 bound to `127.0.0.1` with a small fixed port set.

Measured reason: it is the only option that reuses the tokio 1 runtime we already ship, keeps range handling to about 40 lines, and lets us test the whole proxy (register, playlist rewrite, range, auth) with plain reqwest in `cargo test` and no WebView. Version 0.8 is the current stable and builds with tokio 1 and hyper 1, which reqwest 0.12 already pulls, so there is no runtime or transport mismatch. Version 0.7 would work but would forgo 0.8 security and maintenance fixes for no weight saving. The weight question is answered by measurement, not by hope: P0 must record `cargo bloat --release` before and after, plus the installer size check already in CI, and fail if growth exceeds 1 MB without a note in `docs/ENGINEERING-NOTES.md`. If axum pushes the bundle over budget, fall back to hyper directly, not to a hand rolled server.

Exact Cargo addition for P0:

```toml
axum = { version = "0.8", default-features = false, features = ["http1", "json", "query"] }
```

Keep `tokio` as is (`full` is already enabled). Reuse the existing `reqwest` with `stream` for upstream. Do not enable `http2`, `ws`, `multipart`, or `tower-log`. Add `mime = "0.3"` only if content type sniffing needs it, else use hardcoded match.

Routes (P0 through P3):

```rust
// GET /s/:id/manifest.m3u8?token=...  playlist fetch plus rewrite
// GET /s/:id/seg/:child?token=...      segments, keys, init maps, subtitles
// GET /s/:id/file?token=...            MP4 and progressive fallback
// GET /healthz                          loopback liveness, no token, returns 200 plus port
```

Bind: `TcpListener::bind("127.0.0.1:0")` at startup is tempting, but ephemeral breaks the static CSP. So bind `127.0.0.1` on a fixed default (for example `39321`) and try the next 7 ports on collision. Publish the chosen port through a command. CSP lists all 8 origins explicitly. See section 8.

## 4. Data model

Entries live only in memory. Nothing proxy related touches SQLite. Restart clears everything, which is a feature.

```rust
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct UpstreamHeaders(pub HashMap<String, String>);

#[derive(Debug, Clone)]
pub struct ProxyEntry {
    pub opaque_id: String,      // 128 bit hex, path safe
    pub token: String,          // 128 bit hex, query param, never logged
    pub extension_id: String,   // jar scope and logs
    pub upstream_url: String,   // exact resolve URL, never from HTTP input
    pub kind: StreamKind,        // HlsMaster, HlsMedia, Mp4
    pub headers: UpstreamHeaders,
    pub created_at: Instant,
    pub last_used: Instant,
    pub children: HashMap<String, ChildEntry>, // rewritten segment and key ids
}

#[derive(Debug, Clone)]
pub struct ChildEntry {
    pub child_id: String,       // short opaque id
    pub upstream_url: String,   // resolved absolute URL at rewrite time
    pub content_type: Option<String>,
    pub byte_range: Option<String>, // from EXT-X-BYTERANGE, passed through
}
```

Rules:

* Opaque ids: 16 random bytes from `rand` or `getrandom`, hex encoded. Not sequential, not the episode id. Child ids are scoped under the parent id, so guessing a child without the parent token is useless.
* Token: separate 16 random bytes, `subtle` constant time compare on each request. Token in query string (hls.js and `<video>` cannot set headers on subrequests, query is the only channel). Accept token once per entry family, same token for parent and children to keep hls.js URL building simple.
* Headers: allowlist `User-Agent`, `Referer`, `Origin`, `Cookie` (only if extension supplied it and jar is empty for that origin), `Authorization` (only same origin, never forwarded on cross origin redirect), `Accept`, `Accept-Language`, `Range` (managed by proxy, not blindly copied). Everything else is dropped with a debug log naming the header name only.
* Cookies: stored in a per entry jar, see section 6. The `headers` map holds the initial `Cookie` if the resolver gave one. The jar holds `Set-Cookie` results.
* Lifetimes: idle TTL 2 hours, absolute TTL 4 hours. Touch `last_used` on each hit, including hls.js playlist refreshes. The 2 hour idle covers paused playback: a user who pauses a film for over 30 minutes must still resume without a 403, and a slow live trickle must not evict mid show. The 4 hour absolute still bounds token and cookie lifetime even for an all day pause. A background sweep every 60 seconds evicts expired entries. Revoke on playback close, on source disable, and on explicit `proxy_revoke`. A player heartbeat (touch every 5 minutes while mounted) is an optional alternative if eviction logs show premature expiry, but it is not preferred: it adds IPC churn for a case the 2 hour idle already covers.
* Caps: max 64 entries process wide, max 256 children per entry, max 2000 playlist lines per fetch. Oldest idle evicted first. Eviction logs `opaque_id` prefix plus `extension_id`, never URLs or tokens.

Tauri commands (names illustrative, camelCase payloads per `commands.rs` convention):

```rust
#[tauri::command]
pub async fn proxy_register(
    store: State<'_, ProxyStore>,
    extension_id: String,
    stream: JsStreamInput,   // url, kind, headers, subtitles
) -> Result<ProxyUrl, String>;

#[tauri::command]
pub fn proxy_revoke(store: State<'_, ProxyStore>, opaque_id: String) -> Result<(), String>;

#[tauri::command]
pub fn proxy_status(store: State<'_, ProxyStore>) -> Result<ProxyStatus, String>;
// ProxyStatus { port: u16, entries: usize, enabled: bool }
```

JSON crossing to Svelte:

```json
{
  "proxyUrl": "http://127.0.0.1:39321/s/a3f9c1d4e5b64f8a9c0d1e2f3a4b5c6/manifest.m3u8?token=9f2e1d0c8b7a6a5b4c3d2e1f0a9b8c7d",
  "opaqueId": "a3f9c1d4e5b6...",
  "expiresInSec": 7200,
  "kind": "hls"
}
```

Svelte keeps `JsStream` but adds an optional `proxiedUrl`. Minimal shape change:

```ts
export type JsStream = {
  url: string;
  type: string;
  quality?: string | null;
  headers?: Record<string, string> | null;
  subtitles?: { lang: string; url: string; label?: string }[] | null;
  proxiedUrl?: string | null;
  proxyId?: string | null;
};
```

`PlayerView` uses `proxiedUrl` when present, else `url`. Diagnostics show `proxied: yes/no` and the upstream host, never the token.

## 5. HLS rewrite algorithm

Fetch in Rust, parse line by line, resolve every URI against the playlist URL, register each as a child entry, emit loopback URLs. hls.js then fetches only loopback. Upstream headers and cookies never leave Rust.

Inputs: playlist text (cap 2 MB), playlist absolute URL for base resolution, entry token and id, child budget.

Steps for every playlist fetch:

1. Fetch upstream with entry headers plus jar cookies, timeout 15 s, max 3 redirects each rechecked. Require content type containing `mpegurl` or `x-mpegurl`, or a body starting with `#EXTM3U` when the server lies. Reject bodies over 2 MB with 502.
2. Split on LF. Keep CR handling tolerant. Track whether master or media: presence of `#EXT-X-STREAM-INF` or `#EXT-X-I-FRAME-STREAM-INF` means master.
3. For each tag carrying a URI, resolve, register, rewrite. Order matters, quoted strings first.
4. Preserve query strings and fragments from upstream when resolving. `Url::join` behavior from the `url` crate already does this, do not strip `?token=` style params from upstream.
5. Return rewritten text with `Content-Type: application/vnd.apple.mpegurl` and `Cache-Control: no-store`. Live playlists also get `Cache-Control: no-cache`.

Tags to rewrite:

* Master: `#EXT-X-STREAM-INF:...,URI="..."` (URI may be inline in the next line for older packagers, handle both the attribute form and the next non empty non comment line after the tag), `#EXT-X-I-FRAME-STREAM-INF:URI="..."`, `#EXT-X-MEDIA:TYPE=SUBTITLES,...,URI="..."`, `#EXT-X-MEDIA:TYPE=AUDIO,...,URI="..."`.
* Media: segment lines (every non empty line not starting with `#`), `#EXT-X-KEY:METHOD=AES-128,URI="..."` (do not rewrite `METHOD=NONE` and never rewrite `IV`, it is hex not a URL), `#EXT-X-MAP:URI="..."`, `#EXT-X-PART` URI form if present.
* Session (master): `#EXT-X-SESSION-KEY` URI rewritten like `EXT-X-KEY` with the same 64 KB cap. `#EXT-X-SESSION-DATA` with a `URI` attribute rewritten as a generic file child (512 KB cap) when the URI is http or https. `VALUE` only forms pass through untouched.
* Variables: `#EXT-X-DEFINE:NAME="X",VALUE="Y"` collected in order, then `{$X}` substituted in later URIs before resolution. `IMPORT` is rejected: drop the line and log, because it pulls names from a parent scope implicitly. An undefined variable leaves the URI unresolvable, so drop that line and keep playback alive.
* Preload hint: `#EXT-X-PRELOAD-HINT:URI="..."` rewritten when http or https. The proxy fetches it as a normal segment, it never blocks.
* Subtitle playlists: fetched as child entries and rewritten recursively one level. Live subtitles share the same token.
* `#EXT-X-BYTERANGE:n[@o]`: keep the tag line verbatim, attach the range to the child entry so the segment handler can forward `Range: bytes=o-(o+n-1)`.
* Pass through untouched: `#EXT-X-ENDLIST`, `#EXT-X-PLAYLIST-TYPE`, `#EXT-X-TARGETDURATION`, `#EXT-X-MEDIA-SEQUENCE`, `#EXT-X-SERVER-CONTROL`. Absence of `ENDLIST` means live or event, do not cache, refetch each time hls.js asks.

Not honored (labeled, never silent): `CAN-BLOCK-RELOAD=YES` inside `EXT-X-SERVER-CONTROL` is passed through but the proxy answers immediately instead of blocking. `#EXT-X-SKIP`, `#EXT-X-RENDITION-REPORT`, and part holdback tags pass through without server side blocking. hls.js falls back to polling, which costs extra refreshes but keeps the handler stateless. LL-HLS with delta updates works, true blocking reload does not.

Relative resolution: use the playlist URL as base per RFC 8216 section 4.1. `segments/low/1.ts` joins against `https://cdn.example.com/hls/playlist.m3u8` to `https://cdn.example.com/hls/segments/low/1.ts`. Absolute paths, `../` hops and query strings all go through `Url::parse` plus `join`. On parse failure, drop the line and log the tag name, keep playback alive for the rest.

Encryption keys: key URIs become child entries with a 64 KB cap and `application/octet-stream` passthrough. The `IV` attribute passes through as is. Key rotation (multiple distinct key URIs in one media playlist) registers multiple children. Never log key bytes.

Init maps: `#EXT-X-MAP:URI="init.mp4"` becomes a child with `BYTERANGE` if the tag carries `EXT-X-BYTERANGE`, else full fetch.

Live playlists: every hls.js refresh re-runs rewrite, which re-registers children. To avoid unbounded child growth, reconcile by upstream URL: if a child with the same upstream URL exists, reuse its `child_id` and touch it. Only mint new ids for unseen URLs. Sliding window segments that fall off stop being touched and expire with the sweep.

Redirects during playlist fetch: reqwest follows with a `Policy::limited(3)`. After final URL, recompute the base URL for relative resolution from the final URL, not the original. If the final host leaves the allowlist, fail closed with 502.

Illustrative rewrite core (not a full implementation):

```rust
fn rewrite_media_playlist(base: &url::Url, text: &str, entry: &mut ProxyEntry) -> String {
    let mut out = String::with_capacity(text.len() + 512);
    let mut pending_key: Option<String> = None;
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() {
            out.push_str("\n");
        } else if t.starts_with("#EXT-X-KEY") {
            out.push_str(&rewrite_uri_attr(base, t, "URI", entry));
            out.push('\n');
        } else if t.starts_with("#EXT-X-MAP") {
            out.push_str(&rewrite_uri_attr(base, t, "URI", entry));
            out.push('\n');
        } else if t.starts_with("#EXT-X-MEDIA") || t.starts_with("#EXT-X-I-FRAME") {
            out.push_str(&rewrite_uri_attr(base, t, "URI", entry));
            out.push('\n');
        } else if t.starts_with('#') {
            out.push_str(t);
            out.push('\n');
        } else {
            // segment or subtitle playlist line
            let abs = base.join(t).expect("checked at register");
            let child = register_child(entry, abs.as_str());
            out.push_str(&format!("/s/{}/seg/{}?token={}", entry.opaque_id, child, entry.token));
            out.push('\n');
        }
    }
    out
}
```

Master handling is the same shape with one extra rule: after `#EXT-X-STREAM-INF`, the next URI line is the variant playlist, which itself becomes a child of type playlist. When hls.js requests it, the handler fetches and rewrites it as media. Depth cap 4 stops pathological nesting.

Test fixture sketch (local only):

```rust
// fixture serves /master.m3u8 with two variants, /low.m3u8 with
// relative segments, one EXT-X-KEY and one EXT-X-MAP.
// rewrite asserts: no https://cdn strings remain, key URI is loopback,
// IV untouched, BYTERANGE line preserved, live playlist has no ENDLIST cached.
```

## 6. Cookie jar: per session, capture, scope, expiry

Why a jar: some sources set a session cookie on the playlist fetch that segments then require. hls.js cannot see or forward those cookies cross origin. Rust must.

Design: jar lives inside `ProxyEntry`, not global. One entry, one jar. Two episodes from the same source get separate jars unless they share the entry, which they do not. This bounds leakage by construction.

Capture: on every upstream response (playlist, segment, key), read `Set-Cookie` headers. Parse `name=value` plus attributes `Domain`, `Path`, `Max-Age`, `Expires`, `Secure`, `SameSite`. Store `{ name, value, domain, path, expires_at }`. Cap 32 cookies per entry, 4 KB per cookie, drop oldest on overflow with a log.

Send: on each upstream fetch, attach cookies whose domain matches the request host (exact or leading dot suffix match) and whose path is a prefix of the request path, and which are not expired. Join with `; `. Do not send `Secure` cookies over http (upstream is almost always https, this mostly matters for local fixtures). `SameSite` has no browser meaning server side, but record it: `SameSite=None` without `Secure` is rejected per modern rules, log and drop.

Expiry: `Max-Age` wins over `Expires`. Session cookies (no expiry) live until entry expiry or revoke. Sweep removes expired cookies on each use. No persistence.

Clearing: `proxy_revoke` drops the entry and its jar. Playback close in `SourceBrowser.closePlayer` calls revoke. Source disable or remove revokes all entries for that `extensionId`. App exit drops the store, memory only.

Caveats to document in code: this is an RFC 6265 subset. No public suffix list in P0, so `Domain=.co.uk` style supercookies are blocked by a simple rule (domain must contain an embedded dot and must be a suffix of the request host, and the request host must be a suffix match, not a substring). P3 can vendor `publicsuffix`. No `HttpOnly` distinction needed server side. Cookie headers supplied by the extension in `resolveEpisode` seed the jar once at register, they are not reapplied per request except through the jar.

## 7. Range and streaming

WebView seeking needs `Range`. hls.js also issues range reads for init maps and partial segments.

Behavior:

* `HEAD`: forward upstream headers (`Content-Length`, `Content-Type`, `Accept-Ranges`) without body. Used for diagnostics and for mpv fallback checks. Cache nothing.
* `GET` without `Range`: proxy streams upstream 200 as 200 with `Accept-Ranges: bytes` if upstream supports it, else pass through. Use chunked transfer to the WebView, `Content-Length` only when upstream gave an exact length and no transcode happened (playlists are rewritten, so length changes, omit it there).
* `GET` with `Range: bytes=a-b`: forward the exact header upstream. If upstream returns 206, relay status, `Content-Range` and `Content-Length` verbatim. If upstream returns 200 despite the Range, convert only when cheap: skip `a` bytes by streaming discard up to a 16 MB limit, then stream the rest and return 206 with a synthesized `Content-Range`. If `a` exceeds 16 MB, do not read gigabytes to fake a seek. Abort the upstream read and return 502 with a short `range-unsupported` body so the player shows the mpv fallback. If upstream returns 416, relay 416.
* `Accept-Ranges`: advertise `bytes` for MP4 and segment routes. Playlist routes return `none` plus `Cache-Control: no-store`.
* Backpressure: `reqwest` with `stream` feature gives `bytes_stream`. Pipe into `axum::body::Body::from_stream` with 64 KB chunks. No buffering. If the WebView disconnects, drop the stream, which cancels the upstream read via the aborted body. Log at debug only.
* Timeouts: connect 10 s, first byte 15 s, idle 30 s per read. Live playlists get a shorter 10 s total so hls.js can retry fast. Map timeouts to 504 with a short text body.
* Max body and content types: playlists 2 MB text only. Keys 64 KB. Segments: stream through, allow `video/*`, `audio/*`, `application/octet-stream`, `video/mp2t`. Subtitles: 512 KB cap, `text/vtt` or `application/*vtt*` plus `text/plain`. Anything else gets 415 with the content type name logged, not the body.

Handler sketch:

```rust
async fn file_handler(
    State(store): State<ProxyStore>,
    Path(id): Path<String>,
    Query(q): Query<TokenQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let entry = store.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    check_token(&entry, &q.token).ok_or(StatusCode::FORBIDDEN)?;
    let range = headers.get("range").cloned();
    let upstream = fetch_upstream(&entry, range).await?;
    Ok::<_, ProxyError>(stream_response(upstream))
}
```

`ProxyError` maps to status codes: bad token 403, unknown id 404, expired 410, upstream 4xx relayed as 502 with the code name (not the body), timeout 504, oversized 502.

## 8. CSP and WebView constraints

Current CSP (from `src-tauri/tauri.conf.json`):

```
default-src 'self';
connect-src 'self' asset: http://asset.localhost ipc: https: blob:;
img-src 'self' asset: http://asset.localhost https: data:;
media-src https: blob: http://asset.localhost;
style-src 'self' 'unsafe-inline';
script-src 'self'
```

Minimal change: add the exact loopback origins for the fixed port set to `media-src` and `connect-src`. Nothing else changes. `script-src` and `style-src` stay as is. No `unsafe-eval` addition (hls.js runs as a bundled worker already).

Proposed CSP after P0 (ports 39321 through 39328):

```
connect-src 'self' asset: http://asset.localhost ipc: https: blob: http://127.0.0.1:39321 http://127.0.0.1:39322 http://127.0.0.1:39323 http://127.0.0.1:39324 http://127.0.0.1:39325 http://127.0.0.1:39326 http://127.0.0.1:39327 http://127.0.0.1:39328;
media-src https: blob: http://asset.localhost http://127.0.0.1:39321 http://127.0.0.1:39322 http://127.0.0.1:39323 http://127.0.0.1:39324 http://127.0.0.1:39325 http://127.0.0.1:39326 http://127.0.0.1:39327 http://127.0.0.1:39328;
```

Why wildcard loopback is risky: `http://127.0.0.1:*` or `http://localhost:*` in CSP lets the WebView reach any loopback listener, including a dev server, a debug port, or another app holding localhost. The blast radius is every local HTTP service, not just this proxy. Exact ports bound the reach to ports the app itself tried to bind. The token still guards the proxy even if another listener shares a port later, because the proxy owns the port while running.

Fixed versus ephemeral port: ephemeral (`bind 0`) needs a wildcard in the static CSP, which Tauri cannot rewrite at runtime. Fixed ports allow exact CSP entries. Collision risk is handled by trying 8 ports in order and publishing the winner via `proxy_status`. If all 8 are taken, playback falls back to direct plus mpv with a clear error. Measure collision frequency in diagnostics (counter only, no ports logged).

`media-src` versus `connect-src`: `<video src>` and `<track src>` need `media-src`. hls.js segment fetches use XHR and fetch, which need `connect-src`. Subtitle files loaded through hls.js need `connect-src`, native `<track>` needs `media-src`. Both directives get the same origins. `img-src` does not change. Thumbnails stay direct.

hls.js behavior: `hls.loadSource(proxiedManifestUrl)` then fetches playlists and segments from loopback URLs embedded in the rewritten manifest. No custom headers are set client side. CORS is not triggered for same origin loopback fetches from the WebView page origin, but add `Access-Control-Allow-Origin: *` on playlist responses anyway for worker contexts, and `Access-Control-Allow-Headers: Range` plus `Access-Control-Expose-Headers: Content-Range, Content-Length, Accept-Ranges` on segment responses. Handle `OPTIONS` with 204 and a short max age.

Playback from `127.0.0.1`: use the literal IP, not `localhost`, to avoid IPv6 versus IPv4 split and DNS interception. The proxy binds `127.0.0.1` only. The frontend builds URLs from the port returned by `proxy_status`, never hardcoded.

## 9. Player wiring: when to proxy, in app versus mpv, resolve flow, errors, death

Decision rule in `SourceBrowser` after `resolve()` returns:

* Proxy when any of these hold: `headers` is non empty, `type` is `hls`, URL query looks signed and short lived (contains `Expires`, `Signature`, or `token` case insensitive), or a previous direct attempt failed with a network error. Else direct.
* MP4 with no headers and a stable archive.org style URL stays direct. This keeps the public domain sample on the current path and limits proxy load.
* External mpv path is unchanged: `openExternal` still passes the original URL plus headers to `player_play`. The proxy is in app only. mpv does not need it because `build_args` already sends headers.

New resolve flow:

1. `resolve(episode)` gets `JsStream[]`.
2. For the chosen stream, call `proxy_register` with `{ extensionId, url, kind, headers }` when the rule says proxy. Store `{ proxiedUrl, proxyId }` beside the stream.
3. Mount `PlayerView` with the proxied URL. `setup()` is unchanged except it prefers `proxiedUrl`.
4. On player close, error, or episode switch, call `proxy_revoke(proxyId)`. Fire and forget, log failures at debug.
5. Subtitles: if subtitle URLs need headers, register each as a child file entry and rewrite the `<track src>` to loopback. Else leave direct. P0 proxies only the media, P1 adds subtitle proxying.

Error states in `PlayerView`:

* `403` or `410` from loopback means token expired or entry evicted. Show `The stream session expired. Retry.` Retry re-registers from the cached `JsStream` and reloads. One automatic re-register is allowed, then require a manual retry to avoid loops.
* `502` means upstream refused. Show `The source refused the request. Open it in mpv.` with the upstream host name, matching current copy tone.
* `504` means timeout. Show `The source timed out. Retry.` Keep the existing buffering and error slots, add a `proxied` bit to diagnostics.

Logging and diagnostics: Rust logs `proxy_register`, `proxy_hit`, `proxy_revoke`, `proxy_evict` at info with `extension_id` and id prefix. Upstream hosts logged at debug. Tokens, cookies, `Authorization` and key bytes never logged. Frontend diagnostics add `Proxied: yes`, `Proxy: 127.0.0.1`, `Entry: a3f9c1...`. Copy diagnostics still redacts the token.

When the proxy dies or the token expires: the video element fires `error` and hls.js fires fatal `ERROR`. The UI shows the expired message above. A liveness check `GET /healthz` on player mount detects a dead server early and skips proxying, falling back to direct with a toast. If the port changed since registration (restart), `proxy_status` port mismatch triggers re-register.

Kill path: any panic in a handler must not kill the server. Handlers return `Result`, errors map to status codes. The server task is supervised, on crash the frontend sees `healthz` fail and disables proxying for the session.

## 10. Failure modes and what could break

| Symptom | Cause | Detection | Recovery |
|---|---|---|---|
| Sample MP4 (archive.org) no longer plays | Proxy forced on for all streams, or CSP edit broke `https:` media | Manual QA on sample plus `cargo test` direct path test | Keep direct path default for headerless MP4. CSP change is additive only. Revert to direct on `healthz` fail |
| HLS plays first segments then stalls | Child eviction too aggressive, live reconcile minting new ids per refresh | Integration test with 3 playlist refreshes asserting stable child ids | Reuse child ids by upstream URL. Bump per entry child cap. Touch on use |
| Keys fail, segments 403 | Key URI not rewritten, or `Authorization` dropped on redirect that needed it | Fixture with `EXT-X-KEY` asserting loopback key fetch with jar cookie | Rewrite key URIs. Forward `Authorization` only same origin. Log key fetch status without bytes |
| External mpv arg broken | Shared header sanitizer changed `build_args` behavior | Existing `player.rs` tests (`builds_mpv_args_with_headers_and_subtitles`, `strips_newlines_from_headers`) | Sanitizer lives in one function used by both paths. Tests cover both |
| Resume and progress lost | Player close path revokes before `saveProgress` flushes | `sourceProgressSet` still fires on close in `closePlayer` | Revoke after progress write. Order: `reportProgress(true)`, await `saveProgress`, then `proxy_revoke` |
| Downloads or copy URL broken | Clipboard now copies a loopback URL with a dead token | QA on Streams Copy button | Copy always uses the upstream `url`, never `proxiedUrl`. Label stays `Copy` |
| CSP blocks loopback | Port outside the 8 listed, or typo in `tauri.conf.json` | DevTools console CSP error, `healthz` passes via Rust but video fails | Constrain bind loop to the listed 8. CI check parses CSP and asserts all 8 present |
| App startup slower | Proxy bind retries or DNS pinning on the main thread | Cold start measure in `ENGINEERING-NOTES.md` | Bind on a background task after `setup`. Frontend gates proxy on `proxy_status`, direct works meanwhile |
| Tests flaky or network dependent | Fixture hits real CDN | CI with no network | All proxy tests use a local axum fixture on `127.0.0.1:0`. Forbid `https://` in proxy tests via a test lint |
| Port collision on user machine | Dev server or another app holds all 8 ports | `proxy_status.enabled=false` plus toast | Fall back to direct plus mpv. Log port busy count, no ports in user visible text |

## 11. Test plan

No external network in tests. All fixtures are local HTTP servers on `127.0.0.1` with ephemeral ports.

Unit tests (Rust, `cargo test`):

1. Token check: wrong token gives 403, unknown id gives 404, expired entry gives 410. Assert no token bytes in the error body.
2. Header sanitizer: CR, LF, NUL rejected. `Host` and `Connection` dropped. Cap of 16 headers enforced.
3. Playlist rewrite master: variant URIs (attribute and next line forms), `I-FRAME`, `MEDIA` audio and subtitles all become loopback. No `https://` origin strings remain except in comments. Query strings preserved.
4. Playlist rewrite media: relative segments resolved, `EXT-X-KEY` URI rewritten with `IV` untouched, `EXT-X-MAP` rewritten, `BYTERANGE` preserved, `ENDLIST` passed through.
5. Live reconcile: two refreshes with overlapping windows reuse child ids. New segments mint ids. Total children bounded.
6. Range: `bytes=0-99` forwarded, 206 plus `Content-Range` relayed. Upstream 200 converted to 206 with synthesized range. 416 relayed.
7. Cookie jar: `Set-Cookie` captured, sent on next fetch, expired cookie dropped, cross extension isolation (jar A never sent to host B).
8. SSRF blocks: `169.254.169.254`, `127.0.0.1` as upstream, `file:///`, private `10/8` all refused at register with a typed error.
9. Body caps: 3 MB playlist gives 502. 100 KB key passes. Oversized subtitle gives 415.
10. Existing `player.rs` tests keep passing unchanged.
11. CSP and port set match: parse `src-tauri/tauri.conf.json`, extract `app.security.csp`, and assert the loopback origins in `media-src` and `connect-src` exactly equal the fixed bind set (`http://127.0.0.1:39321` through `http://127.0.0.1:39328`, no more, no wildcard). A port edit without a matching bind edit fails the test.
12. New tag rewrite: fixture with `EXT-X-SESSION-KEY`, `EXT-X-SESSION-DATA` with URI, `EXT-X-DEFINE` variable use, and `EXT-X-PRELOAD-HINT` asserts session URIs become loopback, variables substitute before resolution, `IMPORT` is dropped, and `SERVER-CONTROL` passes through without blocking.
13. Range skip bound: fixture ignores `Range` and returns 200. Seek at 1 MB synthesizes 206. Seek at 32 MB returns 502 and reads at most 16 MB plus headers before aborting.

```rust
// Illustrative, not the full test.
#[test]
fn csp_loopback_matches_bind_set() {
    let raw = std::fs::read_to_string("src-tauri/tauri.conf.json").unwrap();
    let conf: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let csp = conf["app"]["security"]["csp"].as_str().unwrap();
    let expected: Vec<String> =
        (39321..=39328).map(|p| format!("http://127.0.0.1:{p}")).collect();
    for origin in &expected {
        assert!(csp.contains(origin), "missing {origin} in CSP");
    }
    assert!(!csp.contains("127.0.0.1:*"), "wildcard loopback forbidden");
}
```

Integration tests (Rust, tokio):

* Full loop: fixture origin serves master plus media plus 2 segments plus key. Register entry, GET manifest through proxy, assert rewritten, GET one segment with `Range`, assert bytes equal fixture bytes. Revoke, assert 403 or 410.
* Redirect test: fixture 302s to a second local path, proxy follows once. 302 to a blocked IP fails closed.
* Death test: kill fixture mid stream, proxy returns 502, server stays up for the next request.

Frontend tests (Vitest, pure logic):

* `shouldProxy(stream)`: headers present means true, hls means true, headerless mp4 means false. Signed query means true.
* `preferProxiedUrl`: uses `proxiedUrl` when set, falls back to `url`.
* Error copy mapping: 403 maps to expired text, 502 to refused text, 504 to timeout text. Assert no token in copied diagnostics.

Manual QA matrix (real app, local fixture plus sample, no external HLS beyond what extensions already return):

| Width | Theme | Source type | Checks |
|---|---|---|---|
| 1024 | Dark, Light | Sample MP4 direct | Plays, seek works, Resume saves, diagnostics show `Proxied: no` |
| 1280 | Dark, Light | Header MP4 via proxy | Plays, seek with Range works, mpv fallback still works, no token in logs |
| 1600 | Dark, Light | HLS media playlist via proxy | Segments load, quality stable, subtitles show, retry after revoke re-registers |
| 2560 | Dark, Light | HLS master plus keys plus live | Variant switch, key fetch, live edge advances, close clears entries (`proxy_status.entries` drops) |

Light theme check matters because player chrome uses dark tokens over video in both themes, verify controls stay legible. Reduced motion has no effect on proxy, but verify no layout shift when `Proxied` chip appears.

## 12. Phased implementation

P0, minimal proxy for MP4 headers (effort: 3 to 5 days). Status: shipped.

* axum server on `127.0.0.1` with the 8 port set, `/s/:id/file` plus `/healthz`, token check, header allowlist, range forward, streaming body, `proxy_register`, `proxy_revoke`, `proxy_status`.
* Frontend: `shouldProxy` for MP4 with headers, `proxiedUrl` plumbing, revoke on close, diagnostics bit.
* Acceptance: header protected MP4 plays in app. Direct sample MP4 unchanged. No HLS rewrite yet, HLS falls back to direct plus mpv. Bundle delta recorded. All existing tests green.

P1, HLS media playlists (effort: 4 to 6 days). Status: shipped. 49 Rust tests, 25 proxy tests. Child reuse keys on URL plus byte range, and a playlist range is authoritative for that child.

* `/s/:id/manifest.m3u8` plus `/s/:id/seg/:child`, media playlist rewrite (segments, key, map, byterange), child reconcile, subtitle file proxy.
* Acceptance: single variant HLS with relative segments and init map plays. Key rotation with two keys passes. Fixture tests green. Manual QA at 1280 and 1600.

P2, master playlists and keys (effort: 3 to 5 days). Status: shipped. 55 Rust tests, 31 proxy tests. Depth cap 4, typed `playlist-depth` and `child-not-allowed` failures, and every child is checked against the extension manifest allowlist.

* Master rewrite (stream inf, iframe, media audio and subtitles), nested variant fetch, depth cap, key handler with 64 KB cap, redirect recheck with final base URL.
* Acceptance: master with two variants plus audio group plays. Variant switch works. Malicious playlist fixture (nested depth 6, `file:` URI) fails closed with player intact.

P3, cookie jar and live playlists (effort: 4 to 6 days):

* Per entry jar with capture, scope, expiry, clearing. Live no cache path with child reuse. `SameSite` and supercookie rules with the embedded dot check plus a warning comment. No public suffix list in P3 by default.
* Acceptance: fixture requiring cookie from playlist on segments plays. Live playlist advances across 3 refreshes without child explosion. Jar cleared on close verified by `proxy_status` plus a segment fetch returning 403 after revoke.

Each phase updates this plan with measured numbers (bundle size, cold start, time to first frame) before moving on. No phase widens CSP beyond the 8 ports.

## 13. Rollout and feature flag

Default: off through P1 behind a setting, on at P2 after QA passes. Rationale: P0 touches playback for protected streams only, but a bad bind or CSP typo could break all loopback playback if forced on. A flag bounds the blast radius.

Setting: `Settings, Playback, Use loopback proxy for protected streams`, boolean, default off during P0 and P1, default on after P2 QA. Stored in localStorage plus a Rust side check (frontend simply skips `proxy_register` when off). Kill switch: if `proxy_status.enabled` is false (bind failed), the frontend treats the flag as off and shows one toast per session.

Logs: info lines for register, hit counts per minute, revoke, evict. Debug for upstream hosts and status codes. Never info or debug for tokens, cookies, `Authorization`, key bytes, full URLs with query. Redaction helper `redact_url` keeps `host + path` and drops query before logging.

Diagnostics export: include `proxy_status` (port, entries count, enabled) and recent proxy error codes without URLs. The existing diagnostics copy in `PlayerView` adds `Proxied: yes/no` and `Proxy: ok/down`.

Rollback: turn the setting off, or ship a config bump that sets `enabled=false` server side. No migration needed because entries are memory only.

## 14. Decisions (defaults, user can override)

1. Port set: `39321` through `39328`. No clash with dev servers or updater ports is known. Override with another 8 port block if tooling reserves this range.
2. Default on timing: proxy off through P1, on at P2 after QA passes. P0 and P1 stay opt in behind the Settings flag.
3. `Authorization`: forwarded only same origin, dropped on any cross origin redirect. No blind forwarding.
4. Subtitles: proxied in P1 when headers are present, else left direct. Header protected subtitle files work once `/seg` exists.
5. Public suffix: deferred to P3. Until then the embedded dot rule applies plus a code comment warning that `Domain=.co.uk` style overreach is blocked crudely, not precisely.
6. External mpv: always gets the direct URL plus headers, never a loopback URL. Keeps the token out of the child process command line.
7. Diagnostics: counters only (proxy hits, 403 rate, eviction count). No URLs, tokens, or cookies in logs or exports.
