# libmpv P1 readiness checklist

Status: P1 packaging is enabled by the maintainer on 2026-11-09. The two DLLs are bundled from the gitignored `src-tauri/lib` payload. The embedded player stays opt-in and `kaminari.libmpv` stays off by default. Public distribution waits for a qualified lawyer to review `docs/LEGAL-REVIEW-DRAFT.md`; local installer builds and playback checks can proceed.

Date: 2026-11-09.

## Hard gates

- [x] License package is complete: LGPL-2.1 text for the wrapper, LGPL-2.1 and LGPLv3 texts for the LGPL libmpv plus FFmpeg build, MPL-2.0 text for the plugin, mpv copyright attribution, exact versions, source URLs, archive hashes, build date, and a stable source offer.
- [x] A draft legal review packet exists in `docs/LEGAL-REVIEW-DRAFT.md`, with artifact pins, linking posture, source offer and open questions.
- [ ] A qualified lawyer reviews the package and the dynamic linking posture before public distribution.
- [x] DLLs are bundled from `src-tauri/lib`, `bundle.resources` includes `lib/**/*`, and `pnpm check:libmpv` fails the bundle if either DLL is missing or the hash differs.

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

FFmpeg source: the Build MPV (64, true) job at https://github.com/zhongfly/mpv-winbuild/actions/runs/42478239987/job/128724313594 ran `git -C mpv-winbuild-cmake/src_packages/ffmpeg rev-parse --short HEAD` at 12:12:25Z and returned `cc583787b`. The full commit is `cc583787bbecd4914fee7c2bf7f09463113822fe`. The same job produced the LGPL libmpv archive. The source offer and texts are in `licenses/native/README.md`.

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

Run for each packaging candidate. Record the native payload separately from the 15 MB core budget.

0.19.0 measurement, 2026-11-09:

| Item | Bytes | Note |
| --- | --- | --- |
| NSIS installer | 37,646,594 | 35.90 MiB |
| Release exe | 20,895,232 | 19.9 MiB |
| libmpv-2.dll | 101,853,696 | 97.1 MiB uncompressed |
| libmpv-wrapper.dll | 391,680 | 0.37 MiB uncompressed |
| Native payload | 102,245,376 | 97.5 MiB uncompressed |

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
- [x] Uninstall removes the app and both DLLs. Verified on the development machine on 2026-11-09: the install directory was removed and `%APPDATA%\app.kaminari.desktop` stayed. No stale DLL was left behind. A clean VM run remains.

## Native license group verification

- [x] The Licenses dialog shows a Native libraries group with both DLL names, versions, source URLs, license texts, and the source offer.
- [x] `public/licenses.json` contains the native entries generated from pinned metadata, with no missing or placeholder text.
- [x] The entries match the artifact hashes above and the files inside the installer. Verified in the 0.19.0 installed build on 2026-11-09.
- [x] These files are owned by the native license package session. This checklist does not edit `docs/NATIVE-LICENSES.md`, `licenses/native`, `scripts/generate-licenses.mjs`, `src/lib/LicensesDialog.svelte`, or `public/licenses.json`.

## P1 exit

- [x] Packaging changes are in their own commit. The default player is still PlayerView and the libmpv flag is still off unless a person opts in.
- [ ] Every remaining gate above is checked and dated, including the VM install, update and rollback.
- [ ] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `npm exec --yes pnpm@10 -- check`, `npm exec --yes pnpm@10 -- build`, `npm exec --yes pnpm@10 -- test`, and `scripts/check-style.sh` pass.
