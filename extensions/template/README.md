# Example Template

A skeleton Kaminari source. Copy this folder, rename the id, and replace the four method bodies in `index.js`.

## The four methods

`index.js` assigns one global, `globalThis.kaminari`, with four synchronous functions. The host calls them by name and validates what comes back.

| Method | Arguments | Returns |
| --- | --- | --- |
| `search` | `query` (text the user typed), `page` (starts at 1) | `{ items, hasNext }`, each item with `id`, `title`, `cover`, `year` |
| `getShow` | `id` | one show: `id`, `title`, `cover`, `synopsis`, `year`, `genres`, `status` |
| `getEpisodes` | `showId` | an array of episodes: `id`, `showId`, `number`, `title` |
| `resolveEpisode` | `episodeId` | an array of streams: `url`, `type`, `quality`, optional `headers` and `subtitles` |

`type` on a stream is one of `hls`, `mp4`, `dash`, `embed`.

Ids stay strings everywhere. Registry source ids are decimal strings larger than JavaScript can hold as numbers, so `String(...)` on the way out and never `Number(...)`.

## The host allowlist

`permissions.hosts` in `manifest.json` is the only network access the extension gets. A fetch to a host that is not listed fails with `HOST_DENIED`.

Redirects count. If a resolved stream redirects to a CDN, a media host, or an embed host, list that host in `permissions.hosts` too. The loopback proxy rechecks every redirect hop and fails closed with a 502 when a host is missing. Widening hosts on update asks the user again.

Entry shape: `https://host[:port]` or `https://host[:port]/path`, with an optional `*` at the end. A path rule matches that path and its child paths, never a sibling prefix. An explicit port narrows the rule to that port; without one, only the default HTTPS port is allowed. `https://*.example.com/*` matches subdomains of `example.com` but not the apex. Percent-encoded dot segments and separators are rejected.

## Limits

- Every method runs for at most 5 seconds, 10 seconds for `resolveEpisode`.
- The sandbox caps memory at 64MB.
- `async` and `await` are rejected with `ASYNC_UNSUPPORTED`. Bridge calls are synchronous.
- No filesystem, no child process, no environment. The bridge is `net.fetch`, `html.select`, `html.attr`, `crypto.sha256`, `storage.get`, `storage.set`, `storage.remove`, `logger`.

One slow or broken extension greys out its own source. The app stays up.

## Package it

A `.kext` is a zip with `manifest.json`, `index.js`, and optionally `icon.png` and `README.md`. The files must sit at the zip root, not inside a subfolder. From inside this folder, on PowerShell:

```powershell
Compress-Archive -Path manifest.json, index.js, README.md -DestinationPath pack.zip -Force
Rename-Item pack.zip com.example.template-0.1.0.kext
```

The name is `<id>-<version>.kext`. Update both halves whenever you bump the manifest, and change the `id` first, since `com.example.template` is a placeholder.

Check syntax before you pack:

```powershell
node --check index.js
```

## Install and test

1. Open the Sources page in Kaminari.
2. Press the install button for a `.kext` file and pick the archive you just made. The Sources page shows it with a `sideloaded` badge.
3. Use the Sources test panel to run `search` with a query, then open a show, load its episodes, and resolve one stream.

The test panel runs the real host, so a missing method, a timeout, or a blocked host shows up there before you share the file.
