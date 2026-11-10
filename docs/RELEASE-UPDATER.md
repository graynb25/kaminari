# Releasing updates

Status: the 0.26.2 signed installer exists locally. The update feed is not live because the repository is private. This note records the exact steps so the feed can be published later without rediscovering them.

## Keys

The updater keypair is outside the repository:

- Private key: `%APPDATA%\app.kaminari.desktop\update-signing.key`
- Public key: `%APPDATA%\app.kaminari.desktop\update-signing.key.pub`
- Password: `%APPDATA%\app.kaminari.desktop\update-signing.password`

The public key is already in `src-tauri/tauri.conf.json`. Never commit the private key or password. Losing the private key means existing installs cannot verify future updates.

## Build a signed installer

Set the private key and password in the environment, then run the Tauri build:

```powershell
$keyPath = Join-Path $env:APPDATA "app.kaminari.desktop\update-signing.key"
$pwPath = Join-Path $env:APPDATA "app.kaminari.desktop\update-signing.password"
$env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content $keyPath -Raw).Trim()
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = (Get-Content $pwPath -Raw).Trim()
npm exec --yes pnpm@10 -- tauri build
```

Artifacts land in `src-tauri/target/release/bundle/nsis/`:

- `Kaminari_<version>_x64-setup.exe`
- `Kaminari_<version>_x64-setup.exe.sig`

The `.sig` file is the updater signature. The installer is also the normal NSIS installer.

## latest.json

The updater endpoint is:

`https://github.com/graynb25/kaminari/releases/latest/download/latest.json`

That URL only works when the repository is public and a release marked latest contains `latest.json`. The file shape:

```json
{
  "version": "0.26.2",
  "notes": "Mangayomi sources, preferred playback quality and auto update.",
  "pub_date": "2026-11-10T13:42:47Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "<entire contents of Kaminari_0.26.2_x64-setup.exe.sig>",
      "url": "https://github.com/graynb25/kaminari/releases/download/v0.26.2/Kaminari_0.26.2_x64-setup.exe"
    }
  }
}
```

`signature` is a JSON string, so embedded newlines must be escaped. The upload must use the exact asset name in `url`. The updater downloads the installer, checks the signature against the public key in the app and only then installs.

A local copy is written to `src-tauri/target/release/bundle/nsis/latest.json` after a build. It is an artifact, not source.

## Portable zip

Run the signed build first, then:

```powershell
npm exec --yes pnpm@10 -- package:portable
```

The zip lands at `src-tauri/target/release/bundle/portable/Kaminari_<version>_x64-portable.zip`. It contains `Kaminari.exe`, the libmpv `lib` folder, `LICENSE`, `README.md`, `CHANGELOG.md` and `PORTABLE.txt`. It needs the Microsoft Edge WebView2 Runtime, which Windows 10 and 11 normally provide.

The portable build is installer-free, not data-free. Settings and the SQLite library stay in the normal per-user app data folder, so replacing the folder keeps them. A true portable-data mode is a separate change.

## Publishing checklist

1. Keep the repository private until the legal review is done.
2. When ready, make the repository public or move the release assets and manifest to a public host.
3. Create a release tagged `v0.26.2` (or the matching version).
4. Upload the installer, the `.sig` and `latest.json`. Mark the release latest.
5. Confirm `releases/latest/download/latest.json` returns the manifest.
6. Install an older signed build on a clean VM, open Settings, check for updates, download, install and restart.
7. Test a rollback by reinstalling the older installer with the library intact.

## Current limitation

The 0.26.2 release binary reports:

`No public release feed is published for this build. Updates stay local until a signed release exists.`

This is expected while the repository is private and no signed public release exists. The state is shown as a warning, not a crash, and the app stays open.
