# Releasing updates

Status: the repository is public and the signed release feed is live. `v0.26.2` was the first public signed release. `v0.26.3` fixed the Windows update flow, `v0.26.4` hid the Aniyomi registry and added the first-run checklist, `v0.26.5` added source watched marks, `v0.26.6` updated zip, Vite and Vitest, and `v0.26.7` added the player quality menu, Incognito, history polish and Coming Soon. This note records the build and publishing steps.

## Keys

The updater keypair is outside the repository:

- Private key: `%APPDATA%\kaminari-signing\update-signing.key`
- Public key: `%APPDATA%\kaminari-signing\update-signing.key.pub`
- Password: `%APPDATA%\kaminari-signing\update-signing.password`

The public key is already in `src-tauri/tauri.conf.json`. Never commit the private key or password. Losing the private key means existing installs cannot verify future updates.

## Why the tag workflow was removed

The old `.github/workflows/release.yml` ran `tauri-action` on `v*` tags. It failed on a clean runner because the pinned libmpv DLLs are intentionally local and the signing key is not in GitHub. Releases are built and uploaded from the maintainer machine with the steps below. Do not add a tag-triggered release workflow unless it also provisions the libmpv payload and the signing secrets.

## Build a signed installer

Set the private key and password in the environment, then run the Tauri build:

```powershell
$keyPath = Join-Path $env:APPDATA "kaminari-signing\update-signing.key"
$pwPath = Join-Path $env:APPDATA "kaminari-signing\update-signing.password"
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
  "version": "0.26.3",
  "notes": "Mangayomi sources, preferred playback quality and auto update.",
  "pub_date": "2026-11-10T13:42:47Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "<entire contents of Kaminari_0.26.3_x64-setup.exe.sig>",
      "url": "https://github.com/graynb25/kaminari/releases/download/v0.26.3/Kaminari_0.26.3_x64-setup.exe"
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

1. Build with the local signing key and the libmpv payload.
2. Create a release tagged `v0.26.3` (or the matching version).
3. Upload the installer, the `.sig`, `latest.json`, the portable zip and `SHA256SUMS.txt`. Mark the release latest.
4. Confirm `releases/latest/download/latest.json` returns the manifest.
5. Install an older signed build, open Settings, check for updates, download, install and confirm the app reopens.
6. Test a rollback by reinstalling the older installer with the library intact.

## Windows update flow

Tauri's Windows updater launches the NSIS installer and exits the app. In 0.26.1 and 0.26.2 the UI showed a Restart button while the installer was still replacing files. Restarting at that point could relaunch the old exe and block the installer.

0.26.3 passes `restartAfterInstall: true` on Windows and does not show a Restart button. The app closes, the installer replaces the files, and the installer launches the new version. The download must be the signed release; the signature check is unchanged.

An update from 0.26.1 or 0.26.2 to 0.26.3 can still hit the old UI race. Use the 0.26.3 installer once for that jump, then use the in-app updater for later releases.
