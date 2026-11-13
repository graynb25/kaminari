# Architecture

Status: active. Read `docs/ENGINEERING-NOTES.md` for measured numbers. This file explains the system. Design explains the look. Roadmap says the order.

## System

```
Svelte UI (WebView) --Tauri IPC invoke/listen--> Rust core
                                                    |
                    AniList client | Extension host | Source router
                    SQLite WAL   | Image cache    | Player bridge
                    Updater      | Logger         | Keychain
```

Rust owns network, database, image cache, extension host, player lifecycle, updater, and keychain. Svelte owns layout, animation, optimistic UI, and virtualized lists. Anything that needs trust or low latency goes in Rust. Anything that needs 60fps goes in Svelte.

Why Tauri 2 and not Electron: installer near 10MB instead of 90MB plus, startup in hundreds of ms, Rust sandbox plus signed updater built in, WebView2 evergreen on Windows. Cost is a strict IPC line and a WebView2 floor. We pay it with a typed bridge and a bootstrapper check.

Why Svelte 5 Runes and not React: fine grained state fits 500 node grids without VDOM churn, smaller runtime JS, fast Vite HMR. Cost is a smaller hiring pool. We pay it with strict TS and component docs.

Why libmpv and not VLC or web video: gpu-next path, real ASS subs, clean HLS and DASH resolve, one embedded handle through `tauri-plugin-libmpv`. VLC has weaker streaming and sub styling. Web video caps HDR and 4K. Fallback is external mpv or VLC only when bundled libs fail, with a clear error.

Why QuickJS through rquickjs and not a Node sidecar: in process, small, sync timeout plus memory cap per runtime, easy kill on 5s timeout, no child IPC flakiness. Node would add process per extension plus JSON RPC plus supply chain weight. Limit is no async IO inside JS except through bridged `net.fetch`.

Source backends: native `.kext` through QuickJS is the bundled path. A 2026-11-08 review found Suwayomi is a manga server and cannot run Aniyomi/yuzono anime extensions; anime support was removed in its v0.6.0 release in 2021, and the request to restore it has stayed open. No mature equivalent exists. Miru is now Hayase, a BUSL-1.1 torrent client with no source API. Mangayomi is an Apache-2.0 Flutter client with no source API, but its QuickJS anime extensions are the most realistic optional adapter because they return direct URLs with headers and subtitles. The app stays native-first and adds a loopback stream proxy plus embedded libmpv before considering any adapter. An optional user-installed anime server adapter can be explored later behind a flag, with the server on loopback, a per-launch token and a narrow proxy. No APK runtime is bundled.

## IPC contract

Commands use snake case in Rust, camel case in TS through serde rename. Prefix events with `kaminari/v1/`. Payloads derive Serialize and Deserialize. Schemas check in CI. No `any` crosses the bridge.

Invoke, frontend to Rust:

```ts
anilistSearch(query: string, page: number, perPage: number, filters?: MediaFilter): Promise<Page<Media>>
anilistDetail(id: number): Promise<MediaDetail> // stale while revalidate
anilistTrending(page: number): Promise<Page<Media>>
libraryList(status?: ListStatus): Promise<LibraryEntry[]>
libraryUpsert(entry: LibraryUpsert): Promise<LibraryEntry>
libraryRemove(anilistId: number): Promise<void>
progressSet(anilistId: number, episode: number, positionSec: number, durationSec: number): Promise<void>
historyPush(anilistId: number, episode: number): Promise<void>
airingSchedule(force: boolean): Promise<AiringSchedule> // cached snapshot, local day grouping in Svelte
sourcesList(): Promise<SourceInfo[]>
sourceToggle(id: string, enabled: boolean): Promise<void>
extensionInstall(pathOrUrl: string): Promise<ExtensionMeta>
extensionRun(sourceId: string, op: ExtOp, args: unknown): Promise<unknown> // 5s timeout
playerOpen(stream: ResolvedStream, resumeSec?: number): Promise<void>
playerCommand(cmd: string, value?: number): Promise<void>
diagnosticsExport(token: string): Promise<{ bytes: number }> // redacted local JSON report
updaterCheck(): Promise<UpdateStatus>
authLogin(): Promise<void> // OS browser, PKCE
authLogout(): Promise<void>
```

Events, Rust to frontend: `kaminari/v1/progress`, `kaminari/v1/library_changed`, `kaminari/v1/player_event`, `kaminari/v1/rate_limit`, `kaminari/v1/ext_log`, `kaminari/v1/update_available`, `kaminari/v1/online_changed`.

Error envelope: `{ code, message, diagnosticsId, retryAfterSec? }`. Codes include `ANILIST_429`, `DB_LOCKED`, `EXT_TIMEOUT`, `MPV_MISSING`. Frontend maps code to panel or toast. Raw stacks never reach the user. Every catch logs with context and a short diagnostics id in form `kdig_<ulid>`.

Capabilities in `capabilities/` allowlist invoke names only. Asset scope covers the cache dir only. Shell open allows `https://anilist.co/*` and release URLs only.

## AniList client (Rust)

GraphQL at `https://graphql.anilist.co`. reqwest with rustls, no openssl, 10s timeout, 5s connect. UA carries `Kaminari/<version>` plus repo URL.

Rate limit: public ceiling near 90 per minute. Token bucket holds 90, refills 1 per second for 60 per minute sustained. Queue plus `Retry-After` honor. Sleep, emit `rate_limit`, serve stale cache. Idempotent queries retry 3 times with 1s to 8s backoff. Mutations retry zero times. Every 429 logs with a span. UI shows seconds left, not a blank page.

Queries cover search, detail with relations and next airing, trending and airing with cursor. Cache table holds query hash, body, fetched time, TTL. Trending TTL 1 hour. Detail 24 hours. Airing 5 minutes. Stale serves on 429 or offline with a `stale: true` flag.

OAuth uses the AniList implicit grant through `kaminari://auth/callback`. The token is parsed and validated in Rust, stored in the OS keychain, and never reaches JS. AniList tokens last one year and there is no refresh token, so expiry means reconnecting.

### Airing schedule

Rust collects distinct AniList ids from `library` except dropped rows and from `source_library` rows with an `anilist_id`, then calls `Page.media(id_in:)` in batches of 50. Each media request asks for `nextAiringEpisode { airingAt episode }` and a one-node `airingSchedule(notYetAired: true)` fallback. The command returns `items`, `fetchedAt`, `stale` and `warning`.

`airing_schedule(anilist_id, episode, airing_at)` is a snapshot with primary key `(anilist_id, airing_at)` and an index on `airing_at`. `app_meta` stores `airing_schedule_fetched_at` and `airing_schedule_id_set`. A successful refresh deletes ids that left the library, drops rows older than 48 hours, replaces future rows and upserts the new entries in one transaction. Past rows stay for 48 hours so the selected day still reads as a timeline after an episode airs. A matching snapshot returns immediately even when older than 15 minutes; the view forces one background refresh. A failed refresh returns the cached rows with a warning. The cache is local and included in database backups.

The frontend groups Unix seconds by local calendar day with `Intl.DateTimeFormat` parts, formats times in the local zone, and ticks countdowns once per second while the view is visible. The schedule does not require an AniList token, does not request source pages and never writes progress or library state. Schema version 4 adds the table.

## Database

SQLite in WAL mode, one file, rusqlite, checked in migrations. Busy timeout 5000ms. Foreign keys on.

```sql
PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;
CREATE TABLE media(
  anilist_id INTEGER PRIMARY KEY, title_romaji TEXT NOT NULL, title_english TEXT,
  title_native TEXT, cover TEXT NOT NULL, banner TEXT, blurhash TEXT,
  year INTEGER, format TEXT, status TEXT, episodes INTEGER, score REAL,
  genres JSON, synopsis TEXT, updated_at INTEGER NOT NULL
);
CREATE TABLE episodes(
  anilist_id INTEGER NOT NULL REFERENCES media(anilist_id) ON DELETE CASCADE,
  number INTEGER NOT NULL, title TEXT, thumb TEXT, duration_sec INTEGER,
  PRIMARY KEY(anilist_id, number)
);
CREATE TABLE library(
  anilist_id INTEGER PRIMARY KEY REFERENCES media(anilist_id) ON DELETE CASCADE,
  status TEXT NOT NULL CHECK(status IN ('watching','planning','completed','paused','dropped')),
  added_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
);
CREATE TABLE progress(
  anilist_id INTEGER PRIMARY KEY REFERENCES media(anilist_id) ON DELETE CASCADE,
  episode INTEGER NOT NULL DEFAULT 1, position_sec INTEGER NOT NULL DEFAULT 0,
  duration_sec INTEGER NOT NULL DEFAULT 0, updated_at INTEGER NOT NULL
);
CREATE TABLE history(
  id INTEGER PRIMARY KEY AUTOINCREMENT, anilist_id INTEGER NOT NULL,
  episode INTEGER NOT NULL, watched_at INTEGER NOT NULL
);
CREATE TABLE sources(
  id TEXT PRIMARY KEY, name TEXT NOT NULL, version TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1, manifest JSON NOT NULL, installed_at INTEGER NOT NULL
);
CREATE TABLE http_cache(key TEXT PRIMARY KEY, body BLOB NOT NULL, fetched_at INTEGER NOT NULL, ttl_sec INTEGER NOT NULL);
```

Schema version 4 adds `airing_schedule(anilist_id, episode, airing_at, PRIMARY KEY(anilist_id, airing_at))` and an index on `airing_at`. The snapshot rules are under Airing schedule.

Complete past 90 percent marks complete and advances. Backup uses `VACUUM INTO`. If the database cannot be opened at startup, a native recovery dialog offers Retry, Open data folder and Quarantine; quarantine moves `kaminari.db` and its `-wal`/`-shm` sidecars into a `kaminari.db.quarantine-<ts>` folder and starts a fresh database. Nothing is deleted quietly. Source ids that can exceed 2 to the 53 stay TEXT on every boundary. See registry notes in `docs/EXTENSION-API.md` and the measured sample in `docs/ENGINEERING-NOTES.md`.

### Data at rest

The database is not encrypted as a whole. Library, watched marks, playback progress, history, source library, source progress and source history are plaintext SQLite rows. They rely on the Windows account and Windows disk encryption (BitLocker or Device Encryption). Full SQLite encryption with SQLCipher was rejected: it would replace the bundled SQLite 3.54.1 build that carries the RUSTSEC-2026-0459 / CVE-2026-119375 fix (M-19), so the data-at-rest change would regress a known security fix. It would also complicate backups, portable use and the migration path for no measurable gain over disk encryption.

Extension storage is the exception. `js_storage.value` is protected with Windows DPAPI in the current-user scope before it reaches SQLite and stored as `dpapi:v1:<base64>`. `ExtStorage::get` decrypts, `ExtStorage::set` encrypts, and the extension API stays plaintext. Storage key names and timestamps remain plaintext. The database opens with a best-effort pass that encrypts legacy plaintext rows; a read also migrates a plaintext row in place. A malformed or foreign ciphertext is logged and treated as missing, never returned as ciphertext and never re-encrypted. The 1 MB extension quota is still measured in plaintext bytes. DPAPI binds values to the Windows user and machine, so a backup restored under a different account can contain unreadable extension values; the rest of the backup stays readable. A build without DPAPI support reads `dpapi:v1:` as an ordinary value, so downgrading can make extension storage appear missing. Schema version 3 marks the format, so 0.28.0 and older reject a 0.28.1 backup. The format is one-way until a migration exists.

Settings > Privacy > Data at rest explains the split and offers **Clear all local data**. It writes a marker, closes the live database, deletes `kaminari.db` plus `-wal`/`-shm`, removes Kaminari's `kaminari.db.safety-*` and `kaminari.db.quarantine-*` copies, removes a local `settings.json` if present, clears the WebView profile, and asks for a restart. The process keeps a fresh in-memory database until then. If Windows still holds a file open, the marker makes the next start finish the deletion before opening a new database. Downloads, exported backups and the AniList token in Windows Credential Manager are not touched.

### History retention and safety copies

Settings > Data has **Keep history for**: 30 days, 90 days, 365 days or Forever. The choice is stored with the other local settings, included in settings export and import, and applied through `history_retention_apply`. Rust computes the cutoff and deletes only `history` rows with `watched_at < cutoff` and `source_history` rows with `updated_at < cutoff`. Watched marks, library rows, `progress`, `source_progress`, Continue Watching and `js_storage` are not touched. The app applies the window after startup and whenever the choice changes. The existing `idx_history_watched` index covers the AniList history delete; `source_history` is bounded per show and uses its primary key, so no new index is added.

A restore writes a safety copy with `VACUUM INTO` before replacing the live database. Restore and startup call `cleanup_safety_copies`, which keeps the newest three `kaminari.db.safety-<nanos>.db` files in the data folder. The matcher accepts only regular files with a numeric stamp and the exact safety prefix. The live database, `-wal`/`-shm` sidecars, `.restore-*` scratch files and `kaminari.db.quarantine-*` folders are not matched and are never deleted by this cleanup.

### Diagnostics export

Settings > Updates and diagnostics exports `kaminari-diagnostics.json` through a native save dialog. The path is granted by the same one-use `PathAuthority` as settings and backups, and the file is written through `atomic_file::write_atomic`. Rust generates the report from fixed queries: app version from `CARGO_PKG_VERSION`, OS version and architecture, `PRAGMA user_version` and row counts for the tables named in `DIAGNOSTICS_TABLES`. Feature flags are coarse booleans. The report does not read row contents, titles, URLs, hosts, paths, cookies, extension IDs, storage keys or settings values. It is local only; nothing is uploaded.

Windows Authenticode code signing remains deferred by the owner. The installer is unsigned and may trigger SmartScreen; the updater's Minisign signature is separate and does not sign the executable.

## Extension host

One rquickjs Runtime per extension. Fresh Context per call. Timeout 5 seconds wall, 10 seconds for resolve. Memory cap 64MB. Stack 1MB. Calls run on `spawn_blocking` since the runtime is not Send.

Bridge globals, frozen: `net.fetch` through Rust reqwest with host allowlist, 10s timeout, 5MB body cap. `html.query` through a Rust CSS select helper, no JS DOM. `crypto.sha256`, plus hmac and random. `storage.get` and `set` namespaced per extension, 1MB each, SQLite backed. `storage` values are DPAPI-encrypted at rest in the current-user scope; the bridge still passes plaintext. Denied: fs, child process, env, raw sockets, remote eval, non allowlisted hosts. Denied calls throw `HOST_DENIED` and log.

Failure maps to codes: timeout kills context and toasts `Source X timed out`, OOM disables and suggests update, throw sanitizes and links diagnostics, bad signature refuses install. Cold runtime under 50ms. Warm call overhead under 5ms.

## Networking, images, logging, updater

Net uses one reqwest Client, rustls, h2, gzip and brotli, 30s idle pool. Frontend fetch is blocked by CSP except `asset:` and `ipc:`. All outbound AniList and source HTTP runs in Rust.

Images: URL to sha256 filename, LRU 512MB, 30 day TTL, resize to display width, blurhash stored at ingest. Serve through the asset protocol. Miss shows placeholder color, one retry, then an error chip. Grid decode never blocks the UI thread.

Logging: `tracing` writes to stderr, with the `RUST_LOG` environment filter and info as the default. Release builds use the Windows subsystem, so there is no console and logs are not persisted; Kaminari does not write a rolling log file, ship a log tail or upload diagnostics. The in-app Copy diagnostics action builds a one-off redacted message (error code, version, time and message). Settings > Updates and diagnostics can export a separate redacted JSON report with version, platform, schema version, row counts and feature flags. Tokens, cookies and Authorization values are redacted before display or copy, and neither path reads row contents or settings values.

Updater: `tauri-plugin-updater` plus GitHub Releases plus Minisign. Pubkey pinned in `tauri.conf.json`. Channels stable and beta in Settings. Silent download, toast `Restart to update`, relaunch. Windows ships NSIS plus WebView2 bootstrapper. Unsigned nightly builds warn on SmartScreen. That warning is documented, never bypassed quietly. macOS and Linux land in P3.

## Failure modes

AniList 429: backoff, stale serve, countdown banner, wiremock harness. Offline: cached mode banners, player errors fast with no 3 second spinner. Missing mpv: blocking player error plus install link. Missing WebView2: native dialog plus download link. DB locked or corrupt: busy retry then quarantine path. Extension timeout or OOM: kill plus disable plus toast. Bad signature: refuse plus `EXT_UNTRUSTED`. Each path has a Rust unit test or a fixture pair.
