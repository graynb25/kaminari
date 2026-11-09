# libmpv P1 readiness checklist

Status: pre-P1 hardening. P0 is a default-off dev spike. PlayerView stays the default player and `kaminari.libmpv` stays opt-in. Do not bundle DLLs until every gate below passes.

Date: 2026-11-09.

## Hard gates

- [ ] License package is complete: LGPL-2.1 text for the wrapper, LGPL-2.1 and LGPLv3 texts for the LGPL libmpv plus FFmpeg build, MPL-2.0 text for the plugin, mpv copyright attribution, exact versions, source URLs, archive hashes, build date, and a stable source offer.
- [ ] A qualified lawyer signs off on the package and the dynamic linking posture.
- [ ] No DLLs until sign-off. `src-tauri/lib/` stays gitignored, `bundle.resources` gains no `lib/**/*` entry, and no installer ships `libmpv-2.dll` or `libmpv-wrapper.dll` before the gates pass.

## Artifact hashes

Verify both the archive and the extracted DLL before packaging. These are the P0 pins and must match the files that actually ship.

| Component | Version | Archive SHA-256 | DLL SHA-256 |
|---|---|---|---|
| libmpv-wrapper | v0.1.1, `libmpv-wrapper-windows-x86_64.zip` | `D2FF8B2EDCD34D2968E544ADAA915E5E5C48EB1A0995945005269C2AF119A492` | `0D5ADEAD5F175C55E0790A80924EC0A2636F72E3675C79A6D9D9568B2ED2384A` |
| libmpv | 2026-11-08, mpv git `8bc468896e`, `mpv-dev-lgpl-x86_64-20261108-git-8bc468896e.7z` | `84BD684C848591356B55555001C114FC72AF9EDCE2D4EBB8B38CC7356A3C0BF5` | `2A287ECB1FE2BC5B6AB636CFEC0CACBB12FFEC05E4EB4E5C2CADE1752E2D15E6` |

```powershell
Get-FileHash "libmpv-wrapper-windows-x86_64.zip", "mpv-dev-lgpl-x86_64-20261108-git-8bc468896e.7z" -Algorithm SHA256
Get-FileHash "src-tauri/lib/libmpv-wrapper.dll", "src-tauri/lib/libmpv-2.dll" -Algorithm SHA256
```

## Header safety rule

- `mpv_embed_load` rejects any stream with headers unless the URL is a loopback proxy URL on ports 39321 to 39328.
- It also rejects any header value containing a comma. The wrapper rejects JSON arrays, so fields are joined into one comma separated string and a comma inside a value moves the field boundary.
- `libmpvNeedsProxy` in `src/lib/libmpv.ts` keeps streams with headers and no proxied URL on PlayerView. SourceBrowser shows the loopback proxy notice for MP4 and HLS, and points DASH and embed users to external mpv.
- `shouldProxy` keeps the proxy scope to header protected MP4 and HLS. Headerless media stays direct.
- The loopback proxy supports MP4 and HLS only. DASH and embed streams with headers stay on the external mpv path.
- When the wrapper supports header lists, update the Rust rule, the helper, and their tests in one change. Do not relax the comma or proxy checks first.

## Transparency decision

P0 proved route-scoped transparency on the main window, the app chrome stays opaque, and a dedicated watch window is deferred unless P1 visual/GPU checks fail.

- Config: `transparent: true` on the main window in `src-tauri/tauri.conf.json`.
- CSS marker: `body[data-player-open]` in `src/app.css`. While the player is open, `body` and `#app` become transparent and the app shell is hidden so no opaque page sits between the video and the glass controls.
- The marker is removed on player close. Every other route, the title bar, navigation, dialogs, and toasts paint the normal opaque theme tokens.
- Keep the single window. Do not add a dedicated watch window unless P1 checks fail: controls unreadable over bright HDR frames, video hidden in windowed or fullscreen mode, black video after a monitor move, DPI changes at 125, 150, or 200 percent, sleep and resume, or first frame timeout. If any check fails, record the result and decide on a dedicated watch window before packaging.

## Installer size measurement

Run only after license sign-off. Do not run these commands as part of this hardening change.

```powershell
npm exec --yes pnpm@10 -- tauri build
Get-Item "src-tauri/target/release/bundle/nsis/*.exe" | Select-Object Name, Length
Get-Item "src-tauri/target/release/kaminari.exe" | Select-Object Name, Length
Get-Item "src-tauri/lib/libmpv-2.dll", "src-tauri/lib/libmpv-wrapper.dll" | Select-Object Name, Length
```

Record exe size, installer size, native payload delta, cold start, and time to first frame in `docs/ENGINEERING-NOTES.md`. Keep the 15 MB core installer budget and report the native payload as a separate budget line.

## Clean install, update, and rollback

- [ ] Clean Windows 11 VM with WebView2 present. Install the candidate, play an HEVC file with ASS subtitles, play a header protected stream through the loopback proxy, and confirm no missing DLL errors.
- [ ] Update from the previous signed build in place. Settings, library, and progress survive. Diagnostics show the DLL versions that match the shipped manifest.
- [ ] Roll back by reinstalling the prior build. The SQLite library file stays intact and playback works on PlayerView if libmpv init fails.
- [ ] Uninstall removes the app. No stale DLL is left behind that a later build could load by accident.

## Native license group verification

- [ ] The Licenses dialog shows a Native libraries group with both DLL names, versions, source URLs, license texts, and the source offer.
- [ ] `public/licenses.json` contains the native entries generated from pinned metadata, with no missing or placeholder text.
- [ ] The entries match the artifact hashes above and the files inside the installer.
- [ ] These files are owned by the native license package session. This checklist does not edit `docs/NATIVE-LICENSES.md`, `licenses/native`, `scripts/generate-licenses.mjs`, `src/lib/LicensesDialog.svelte`, or `public/licenses.json`.

## P1 exit

- [ ] Every gate above is checked and dated.
- [ ] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `npm exec --yes pnpm@10 -- check`, `npm exec --yes pnpm@10 -- build`, `npm exec --yes pnpm@10 -- test`, and `scripts/check-style.sh` pass.
- [ ] Packaging changes land in their own commit after sign-off. The default player is still PlayerView and the libmpv flag is still off unless a person opts in.
