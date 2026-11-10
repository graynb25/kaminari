# Plan: AniList authentication and sync

Status: planned. AniList client ID `56484`, redirect URL `kaminari://auth/callback`, implicit grant. No client secret is used. No token is stored in the WebView, localStorage or the database.

## Flow

1. Settings shows the AniList account state. If disconnected, a Connect button calls `anilist_auth_url`.
2. Rust builds `https://anilist.co/api/v2/oauth/authorize?client_id=56484&response_type=token`. No `redirect_uri` parameter is sent; AniList uses the client's registered redirect, which is exactly `kaminari://auth/callback`.
3. The frontend opens the URL in the system browser through the opener plugin.
4. AniList approves the client and redirects to `kaminari://auth/callback#access_token=...`.
5. `tauri-plugin-single-instance` forwards the URL to the running app. `tauri-plugin-deep-link` calls a Rust handler in the capture path, not the WebView.
6. Rust parses only the exact callback, rejects any other deep link, validates the token with a Viewer query, stores it in the OS keychain under service `app.kaminari.desktop` and user `anilist`, then emits an auth-changed event.
7. Settings refreshes `anilist_auth_status` and shows the viewer name and avatar.

AniList tokens last one year. There is no refresh token. An expired token clears the keychain entry and shows Reconnect.

## Commands

- `anilist_auth_url() -> String`
- `anilist_auth_status() -> { connected, viewerName, viewerAvatar, expiresAt }`
- `anilist_auth_logout()`
- `anilist_sync_pull() -> { titles, updatedAt }`
- Later: `anilist_sync_push()`, `anilist_queue_status()`

## Read sync

- Query `Viewer { id name avatar { large } }`.
- Query `MediaListCollection(userId, type: ANIME)` with `lists { name entries { mediaId status progress score updatedAt media { id title coverImage bannerImage description genres averageScore seasonYear format status episodes } } }`.
- Map AniList statuses: CURRENT -> watching, PLANNING -> planning, COMPLETED -> completed, PAUSED -> paused, DROPPED -> dropped, REPEATING -> watching.
- Upsert media, library status and watched rows 1..progress in one SQLite transaction.
- Keep local-only rows. Server entries update matching local rows.
- Show a toast with the number of titles updated. A failed request keeps the current data and shows a retry, not a blank library.

## Write sync and offline queue

- Add `anilist_sync_queue` with `anilist_id`, `status`, `progress`, `updated_at`, `attempts`, `last_error`.
- Local status and watched changes enqueue one row per media, replacing the previous queued operation for that media.
- Replay on reconnect and every 30 seconds while connected. One SaveMediaListEntry mutation per media.
- On success, remove the queue row. On a newer server `updatedAt`, keep the server value, toast the conflict and log a redacted entry.
- Never sync source-library or source-progress rows. AniList only receives AniList media.

## Security

- The client secret is not embedded and not used.
- The token never reaches Svelte or localStorage. It is read from the keychain in Rust and added as a Bearer header by the AniList client.
- Deep link handling accepts only `kaminari://auth/callback` with a fragment. Do not log the fragment or token.
- Diagnostics redact query secrets and authorization values through `redactSecrets`.
- Logout deletes the keychain entry and stops future sync.

## Acceptance

- Connect with the client ID, approve in the browser, return to the app, and see the viewer name.
- Pull a list of at least one title and match its status and progress locally.
- Toggle an episode and see the queued write replay within 30 seconds on good net.
- Go offline, toggle progress, reconnect, and replay exactly once.
- Expired or revoked token shows Reconnect and never a blank screen.
