# Releasing updates

Status: the repository is public and the signed release feed is live. `v0.26.2` was the first public signed release. `v0.26.3` fixed the Windows update flow, `v0.26.4` hid the Aniyomi registry and added the first-run checklist, `v0.26.5` added source watched marks, `v0.26.6` updated zip, Vite and Vitest, `v0.26.7` added the player quality menu, Incognito, history polish and Coming Soon, `v0.26.8` fixed the installer icon, `v0.26.9` deduplicated Continue Watching, and `v0.27.0` contains the production review Phase 0 hardening. This note records the build and publishing steps.

## Keys

The updater keypair is outside the repository:

- Private key: keep it offline on encrypted removable media or in a hardware-backed secret store; copy it into the build session only while signing.
- Public key: embedded in `src-tauri/tauri.conf.json`; safe to share.
- Password: keep it in a password manager or the OS credential store, separate from the key.

Never commit the private key or password, put them in CI, or sync them to cloud storage. Move any existing `%APPDATA%\kaminari-signing` key and plaintext password file to that offline store before the next release: that folder is readable by any process running as the user. If the key is temporarily on disk, restrict it to the maintainer account, keep it out of backups, and delete it after the build.

Losing the private key means existing installs cannot verify future updates. Do not generate a replacement key silently: an update signed by a new key is rejected by every installed app. A key rotation needs a transition release signed by the old key (with the new public key in a later app build) and a documented migration. Keep an offline backup of the key and its password in separate secure locations.

## Signed-version binding

`src-tauri/tauri.conf.json` sets `"requireSignedVersion": true`.

The Tauri CLI records the app version in the Minisign trusted comment when it signs the installer. On update, the plugin verifies the `.sig` against the pinned public key, then compares the signed version with the `version` announced in `latest.json`. A missing signed version or a mismatch is rejected.

This closes the replay path: an attacker who can edit `latest.json` (a compromised GitHub account or release token) cannot pair a higher version string with an older, validly signed installer. It also means:

- Rebuild and re-sign every release. Never reuse a `.sig` from another build.
- Generate `latest.json` from the same build and never edit its `version` by hand.
- The `.sig` must belong to the installer at the manifest's `url`, not to a different architecture or build.
- Releases signed before the Tauri CLI started recording the version will not satisfy `requireSignedVersion`. The pinned Tauri CLI (`2.12.4` in `pnpm-lock.yaml`) records it; build the next release with that CLI or newer and publish a fresh signature.
- The signed-version check does not replace the signature check; both run on every update.

## Why the tag workflow was removed

The old `.github/workflows/release.yml` ran `tauri-action` on `v*` tags. It failed on a clean runner because the pinned libmpv DLLs are intentionally local and the signing key is not in GitHub. Releases are built and uploaded from the maintainer machine with the steps below. Do not add a tag-triggered release workflow unless it also provisions the libmpv payload and the signing secrets in a protected environment.

## Build a signed installer

Mount the offline key store, copy the key into the build session, and read the password interactively. Do not keep the password in a file or shell history:

```powershell
$keyPath = "E:\kaminari-signing\update-signing.key"   # encrypted removable media
$env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content $keyPath -Raw).Trim()
$securePassword = Read-Host "Signing key password" -AsSecureString
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = [System.Net.NetworkCredential]::new("", $securePassword).Password
npm exec --yes pnpm@10 -- tauri build
Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY
Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD
```

Unmount the key store when the build finishes. Artifacts land in `src-tauri/target/release/bundle/nsis/`:

- `Kaminari_<version>_x64-setup.exe`
- `Kaminari_<version>_x64-setup.exe.sig`

The `.sig` file is the updater signature. The installer is also the normal NSIS installer.

## latest.json

The updater endpoint is:

`https://github.com/graynb25/kaminari/releases/latest/download/latest.json`

That URL only works when the repository is public and a release marked latest contains `latest.json`. The file shape:

```json
{
  "version": "<version>",
  "notes": "Release notes for this version.",
  "pub_date": "<ISO-8601 timestamp>",
  "platforms": {
    "windows-x86_64": {
      "signature": "<contents of Kaminari_<version>_x64-setup.exe.sig>",
      "url": "https://github.com/graynb25/kaminari/releases/download/v<version>/Kaminari_<version>_x64-setup.exe"
    }
  }
}
```

`signature` is the contents of the `.sig` file as a JSON string. Tauri writes the `.sig` file as a single base64 line; copy it verbatim and do not reformat or line-wrap it. The upload must use the exact asset name in `url`. The updater downloads the installer, verifies the signature and the signed version against the manifest, and only then installs.

A local copy is written to `src-tauri/target/release/bundle/nsis/latest.json` after a build. It is an artifact, not source.

## Verify assets before publishing

Treat the manifest and every uploaded asset as one unit. Run this after the build, before creating the release:

```powershell
$version = (Get-Content VERSION -Raw).Trim()
$nsis = "src-tauri/target/release/bundle/nsis"
$installer = Join-Path $nsis "Kaminari_${version}_x64-setup.exe"
$signature = "$installer.sig"
$manifestPath = Join-Path $nsis "latest.json"

if (-not (Test-Path $installer)) { throw "Missing installer for $version" }
if (-not (Test-Path $signature)) { throw "Missing signature for $version" }

$manifest = Get-Content $manifestPath -Raw | ConvertFrom-Json
if ($manifest.version -ne $version) {
  throw "latest.json says $($manifest.version), build is $version"
}
$platform = $manifest.platforms.'windows-x86_64'
if ($platform.signature.Trim() -ne (Get-Content $signature -Raw).Trim()) {
  throw "latest.json signature is not the signature of this build"
}
if ($platform.url -notlike "*Kaminari_${version}_x64-setup.exe") {
  throw "latest.json url does not point at this installer"
}

$signatureText = [Text.Encoding]::UTF8.GetString(
  [Convert]::FromBase64String((Get-Content $signature -Raw).Trim())
)
$signedVersion = [regex]::Match(
  $signatureText,
  '(?m)^trusted comment:.*version:(\S+)'
).Groups[1].Value
if ($signedVersion -ne $version) {
  throw "signature carries version '$signedVersion', build is $version"
}

Get-FileHash $installer, $signature, $manifestPath -Algorithm SHA256 |
  ForEach-Object { "{0}  {1}" -f $_.Hash.ToLower(), (Split-Path $_.Path -Leaf) } |
  Set-Content (Join-Path $nsis "SHA256SUMS.txt") -Encoding ascii
```

Add the portable zip to `SHA256SUMS.txt` after it is built. Do not rebuild or replace the installer after recording hashes; if anything changes, delete the hashes and repeat the whole build and verification.

To verify the installer signature cryptographically in the same PowerShell session, decode the base64 `.sig` and run minisign (if installed) with the public key line:

```powershell
$sigPath = Join-Path $env:TEMP "kaminari-update.sig"
[IO.File]::WriteAllBytes(
  $sigPath,
  [Convert]::FromBase64String((Get-Content $signature -Raw).Trim())
)
$config = Get-Content src-tauri/tauri.conf.json -Raw | ConvertFrom-Json
$pubText = [Text.Encoding]::UTF8.GetString(
  [Convert]::FromBase64String($config.plugins.updater.pubkey)
)
$pub = ($pubText -split "`n")[1].Trim()
minisign -Vm $installer -x $sigPath -P $pub
```

The updater repeats the signature and signed-version checks at install time. After publishing, download every asset back from the release and compare it with `SHA256SUMS.txt`. Confirm that `releases/latest/download/latest.json` serves the same manifest. A release whose feed and assets disagree is treated as broken: fix it with a new version, never by editing assets in place.

## Portable zip

Run the signed build first, then:

```powershell
npm exec --yes pnpm@10 -- package:portable
```

The zip lands at `src-tauri/target/release/bundle/portable/Kaminari_<version>_x64-portable.zip`. It contains `Kaminari.exe`, the libmpv `lib` folder, `LICENSE`, `README.md`, `CHANGELOG.md` and `PORTABLE.txt`. It needs the Microsoft Edge WebView2 Runtime, which Windows 10 and 11 normally provide.

The portable build is installer-free, not data-free. Settings and the SQLite library stay in the normal per-user app data folder, so replacing the folder keeps them. A true portable-data mode is a separate change.

## Publishing checklist

1. Move the signing key and password out of plaintext `%APPDATA%\kaminari-signing` storage and sign from the offline key store.
2. Build from a clean checkout of the tagged commit. Confirm `VERSION`, `package.json`, `src-tauri/Cargo.toml` and the generated `latest.json` all carry the same version.
3. Run the release checks: `pnpm check`, `pnpm test`, `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
4. Verify `latest.json` against the `.sig`, then generate `SHA256SUMS.txt` for the installer, signature, manifest and portable zip.
5. Create a release tagged with the matching version and upload the installer, `.sig`, `latest.json`, portable zip and `SHA256SUMS.txt`. Mark it latest only after all assets are present.
6. Re-download the published assets and verify them against `SHA256SUMS.txt`. Confirm `releases/latest/download/latest.json` returns the manifest.
7. On a clean Windows VM, install the previous signed build, open Settings, check for updates, download, install and confirm the app reopens and reports the new version.
8. Verify the downgrade/replay rejection with a test build pointed at a local manifest that announces a higher version but carries the previous installer's signature or version; the update must be refused.
9. Test a rollback by reinstalling the older installer with the library intact.

## Windows update flow

Tauri's Windows updater launches the NSIS installer and exits the app. In 0.26.1 and 0.26.2 the UI showed a Restart button while the installer was still replacing files. Restarting at that point could relaunch the old exe and block the installer.

0.26.3 passes `restartAfterInstall: true` on Windows and does not show a Restart button. The app closes, the installer replaces the files, and the installer launches the new version. The download must be the signed release; the signature and signed-version checks are unchanged.

An update from 0.26.1 or 0.26.2 to 0.26.3 can still hit the old UI race. Use the 0.26.3 installer once for that jump, then use the in-app updater for later releases.
