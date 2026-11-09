# Draft legal review: bundled libmpv

Status: draft for a qualified lawyer. This is not legal advice and it is not a sign-off. Kaminari's maintainer prepared this packet with AI assistance on 2026-11-09 for version 0.19.0. Public distribution should wait for counsel review.

## 1. Question for counsel

Can Kaminari distribute a Windows NSIS installer that bundles the LGPL components below, while Kaminari's own code stays under the MIT license, given the separation, notices, source offer and replacement policy described here? Please identify any changes required before public distribution.

## 2. Distribution model

- App: Kaminari 0.19.0, Windows x64, NSIS installer, MIT licensed own code. The installer is 35.90 MiB and installs per-user to `%LOCALAPPDATA%\Kaminari`.
- UI and backend: Tauri 2, Rust, Svelte 5, SQLite, QuickJS.
- Native video: two separate DLLs installed in a `lib` folder next to `kaminari.exe`.
- `libmpv-wrapper.dll`: C bridge, LGPL-2.1, loaded at runtime with `libloading`.
- `libmpv-2.dll`: LGPL-mode mpv build with FFmpeg statically linked into it, loaded by the wrapper.
- `tauri-plugin-libmpv` 0.3.2: MPL-2.0 Rust crate, statically linked into `kaminari.exe`.
- No static linking of libmpv or the wrapper into `kaminari.exe`.
- The embedded player is opt-in through the `kaminari.libmpv` flag. The default player is the WebView player with external mpv fallback. The DLLs are still distributed and loadable.
- The app does not check DLL signatures or refuse to start when the DLLs are replaced. Users can replace either DLL.
- The installer ships no media, source lists, APKs or stream URLs.

## 3. Components and licenses

| Component | Version | License | Form | Source |
| --- | --- | --- | --- | --- |
| Kaminari | 0.19.0 | MIT | App code | Project repository |
| tauri-plugin-libmpv | 0.3.2 | MPL-2.0 | Rust crate linked into the exe | https://github.com/nini22P/tauri-plugin-libmpv |
| libmpv-wrapper | v0.1.1 | LGPL-2.1 | Separate DLL | https://github.com/nini22P/libmpv-wrapper |
| libmpv (mpv) | v0.42.0-11-g8bc468896e | LGPL-2.1 or later for the `-Dgpl=false` build | Separate DLL | https://github.com/mpv-player/mpv/commit/8bc468896ec711cc5a3ac33dae9cbc0e266bcfa4 |
| FFmpeg inside libmpv-2.dll | commit `cc583787b` | LGPL-3.0 for this build (`--enable-version3`, no `--enable-gpl`) | Static inside `libmpv-2.dll` | https://github.com/FFmpeg/FFmpeg/commit/cc583787bbecd4914fee7c2bf7f09463113822fe |
| Sora, Inter, JetBrains Mono | bundled subsets | SIL OFL 1.1 | Font files | Font project releases |

The wrapper and libmpv are LGPL-2.1, while FFmpeg in this build is treated as LGPL-3.0. Counsel should confirm how those licenses combine and whether the "or later" option in mpv's notice matters.

## 4. Artifact pins

| Artifact | Archive SHA-256 | DLL SHA-256 | Size |
| --- | --- | --- | --- |
| `libmpv-wrapper-windows-x86_64.zip` | `D2FF8B2EDCD34D2968E544ADAA915E5E5C48EB1A0995945005269C2AF119A492` | `0D5ADEAD5F175C55E0790A80924EC0A2636F72E3675C79A6D9D9568B2ED2384A` | 391,680 bytes |
| `mpv-dev-lgpl-x86_64-20261108-git-8bc468896e.7z` | `84BD684C848591356B55555001C114FC72AF9EDCE2D4EBB8B38CC7356A3C0BF5` | `2A287ECB1FE2BC5B6AB636CFEC0CACBB12FFEC05E4EB4E5C2CADE1752E2D15E6` | 101,853,696 bytes |

`pnpm check:libmpv` verifies both DLL hashes before a Tauri bundle. The exact files are also listed in the in-app Licenses dialog under Native libraries.

## 5. Compliance measures in this build

- The GPL `mpv-dev` artifact is not used. Only `mpv-dev-lgpl` is shipped.
- The FFmpeg commit is verified from the Build MPV (64, true) log for the same job that produced the libmpv archive.
- DLLs are separate, dynamically loaded and replaceable. No signature gate blocks replacement.
- The app includes the LGPL-2.1 text, LGPL-3.0 text, MPL-2.0 text and the mpv `Copyright` notice.
- The in-app Native libraries group lists names, versions, source URLs, DLL hashes, license texts and the source offer.
- The source offer states that Kaminari will provide the complete corresponding source on request for at least three years, at no charge beyond media or transmission cost.
- The source set is mpv at the pinned commit, the verified FFmpeg commit, the zhongfly winbuild scripts and `compile-lgpl-libmpv.patch`, the matching wrapper source, and the `tauri-plugin-libmpv` 0.3.2 crate source.
- Kaminari's own code stays in separate files. No mpv or FFmpeg headers are copied into MIT source files without their license notices.
- The Terms of Service contain no reverse-engineering prohibition and no DLL replacement restriction.
- The uninstaller removes the installed DLLs. Updates ship a full installer so the DLLs move with the app.

## 6. Open questions for counsel

1. Does separate DLL loading through `libloading` and the wrapper satisfy the LGPL relinking requirement, including the user's practical ability to replace the libraries?
2. Is the mixed LGPL-2.1 wrapper, LGPL-2.1 mpv and LGPL-3.0 FFmpeg combination compatible, and what notices or source duties follow?
3. Is the written source offer in the README and the in-app dialog sufficient? Does it need to appear in the installer UI, a `NOTICE` file, or a separate web page, and who should be named as the offeror?
4. Must the installer display the LGPL or MPL texts during installation, or is the in-app Licenses dialog plus the shipped texts enough?
5. Does the FFmpeg `--enable-version3` build require additional notices, and should the source offer include the exact configure line, patch and toolchain?
6. Do H.264, HEVC or other codec patent licenses attach to distributing libmpv and FFmpeg, separate from copyright? If yes, what notice or license is needed?
7. Is the mpv, FFmpeg, MPL and wrapper attribution wording adequate, and are any trademark licenses or disclaimers needed?
8. Does the MPL-2.0 plugin impose any file-level source duty beyond keeping its notice and making its source available?
9. Does the MIT license on Kaminari plus the Terms of Service create any conflict with LGPL rights, and should the Terms add an explicit LGPL carve-out?
10. Does bundling the DLLs while defaulting to the WebView player change the analysis, given that the DLLs are still installed and loadable?
11. Are there export control or cryptography issues in the bundled FFmpeg build that need a notice or filing?
12. Does the uninstall and update behavior leave any stale DLL that could be loaded by a later version, and is that a legal as well as a security concern?

## 7. Proposed review block

Reviewer name:

Firm:

Jurisdiction:

Date:

Files and versions reviewed:

- `docs/LEGAL-REVIEW-DRAFT.md`
- `licenses/native/README.md`
- `licenses/native/native.json`
- `licenses/native/LGPL-2.1.txt`
- `licenses/native/LGPL-3.0.txt`
- `licenses/native/MPL-2.0.txt`
- `licenses/native/mpv-Copyright-8bc468896e.txt`
- `src-tauri/tauri.conf.json` and the installer at `src-tauri/target/release/bundle/nsis/`

Conclusion:

- [ ] Approved without conditions
- [ ] Approved with conditions
- [ ] Not approved

Conditions or required changes:

Signature:

Review trigger: a new libmpv or FFmpeg version, a change from dynamic to static linking, a new distribution channel, a change to the source offer, or a change to the Terms of Service.

## 8. Sources

- mpv source: https://github.com/mpv-player/mpv/commit/8bc468896ec711cc5a3ac33dae9cbc0e266bcfa4
- FFmpeg source: https://github.com/FFmpeg/FFmpeg/commit/cc583787bbecd4914fee7c2bf7f09463113822fe
- winbuild release: https://github.com/zhongfly/mpv-winbuild/releases/tag/2026-11-08-8bc468896e
- winbuild job: https://github.com/zhongfly/mpv-winbuild/actions/runs/42478239987/job/128724313594
- wrapper source: https://github.com/nini22P/libmpv-wrapper
- plugin source: https://github.com/nini22P/tauri-plugin-libmpv
- license package: `licenses/native/README.md`
- in-app notices: `public/licenses.json`, generated by `scripts/generate-licenses.mjs`

Maintainer note: local installer builds and playback checks are allowed while counsel reviews. Public distribution should wait for the review block above to be completed by a qualified lawyer.
