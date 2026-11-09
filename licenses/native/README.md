# Native libmpv license package (pre-P1)

Status: preparation only. No native DLL is bundled in this build, and no binary is distributed yet. Lawyer sign-off is still required before any P1 packaging that ships these binaries. This folder holds the license texts, the artifact hashes, and the source references needed for that review.

Scope guard: do not copy DLLs, archives, or samples into the repo. Do not add bundle.resources. The player path and its checklist belong to another session.

## Artifacts (development machine, 2026-11-09)

| Artifact | SHA-256 | Size (bytes) | Source |
| --- | --- | --- | --- |
| `src-tauri/lib/libmpv-2.dll` | `2A287ECB1FE2BC5B6AB636CFEC0CACBB12FFEC05E4EB4E5C2CADE1752E2D15E6` | 101,853,696 | zhongfly `mpv-dev-lgpl-x86_64-20261108-git-8bc468896e.7z` |
| `src-tauri/lib/libmpv-wrapper.dll` | `0D5ADEAD5F175C55E0790A80924EC0A2636F72E3675C79A6D9D9568B2ED2384A` | 391,680 | `libmpv-wrapper-windows-x86_64.zip` |
| `mpv-dev-lgpl-x86_64-20261108-git-8bc468896e.7z` | `84BD684C848591356B55555001C114FC72AF9EDCE2D4EBB8B38CC7356A3C0BF5` | 29,073,618 | https://github.com/zhongfly/mpv-winbuild/releases/tag/2026-11-08-8bc468896e |
| `libmpv-wrapper-windows-x86_64.zip` | `D2FF8B2EDCD34D2968E544ADAA915E5E5C48EB1A0995945005269C2AF119A492` | 196,183 | https://github.com/nini22P/libmpv-wrapper/releases |

Hashes above were verified on the development machine with `Get-FileHash`. The archives and DLLs live outside the repo (local temp dir and `src-tauri/lib`, which is a dev-only payload, never bundled).

## Build evidence (as observed, not re-audited here)

- mpv version string: `v0.42.0-11-g8bc468896e`, mpv commit `8bc468896ec711cc5a3ac33dae9cbc0e266bcfa4`.
- Meson configuration: `-Dgpl=false`, `-Dcplayer=false`, `-Dlibmpv=true`.
- Enabled features do not include GPL-only video outputs.
- libavcodec major: 63.2.
- zhongfly `compile-lgpl-libmpv.patch` removes `--enable-gpl` and disables x264, x265, dvdnav, dvdread, libssh, libsrt, libzvbi, rubberband, avisynth, davs2. It keeps `--enable-version3`, which is why the FFmpeg side is treated as LGPL-3.0. mpv itself uses `-Dgpl=false`.
- tauri-plugin-libmpv is locked at 0.3.2 (MPL-2.0) in `src-tauri/Cargo.lock`. The Rust crate is already listed under the Rust group in `public/licenses.json`.

## Source URLs

- mpv: https://github.com/mpv-player/mpv/commit/8bc468896ec711cc5a3ac33dae9cbc0e266bcfa4
- FFmpeg: https://github.com/FFmpeg/FFmpeg
- zhongfly winbuild: https://github.com/zhongfly/mpv-winbuild, release https://github.com/zhongfly/mpv-winbuild/releases/tag/2026-11-08-8bc468896e, build details https://github.com/zhongfly/mpv-winbuild/actions/runs/42478239987 (see release page for the canonical run link), LGPL patch `compile-lgpl-libmpv.patch` in that repo.
- libmpv-wrapper: https://github.com/nini22P/libmpv-wrapper
- tauri-plugin-libmpv: https://github.com/nini22P/tauri-plugin-libmpv (crate 0.3.2, local registry LICENSE is the MPL-2.0 text copied here).

## FFmpeg source reference (open P1 verification)

The zhongfly release for this build ships a sibling asset named `ffmpeg-lgpl-x86_64-git-cc583787b.7z` (SHA-256 `30ef003bdd8ad8588e722736138d45efefe271e130dcc3827a5e1e2348dac40a`). That name points at FFmpeg short ref `cc583787b`, which resolves on FFmpeg GitHub to commit `cc583787bbecd4914fee7c2bf7f09463113822fe` ("avcodec/vvc: correct RPR reference coordinate derivation", 2026-11-08).

That is filename evidence only. The winbuild FFmpeg recipe tracks FFmpeg master with no pinned tag, so the name does not prove the exact FFmpeg commit statically linked inside `libmpv-2.dll`. Do not treat the full SHA above as proven. Resolving the exact FFmpeg commit (build log linkage or embedded version evidence) is an open P1 verification, owned by the player-path session, and it must close before any packaging that ships the DLL. Until then the dialog and manifest say "not bundled" and carry no exact FFmpeg version claim.

## License texts in this folder

- `LGPL-2.1.txt`: byte copy of the pinned wrapper LICENSE (`C:\Users\bbush\AppData\Local\Temp\kaminari-libmpv\wrapper\LICENSE`). Covers libmpv-wrapper, and the generic LGPL-2.1 text for the mpv LGPL-mode notice.
- `LGPL-3.0.txt`: canonical LGPL-3.0 text from https://www.gnu.org/licenses/lgpl-3.0.txt. Covers FFmpeg as built here (`--enable-version3`, no `--enable-gpl`).
- `MPL-2.0.txt`: byte copy of the local registry LICENSE for tauri-plugin-libmpv 0.3.2. The crate itself is listed under the Rust group; the text here is the filed copy for the native package.
- `mpv-Copyright-8bc468896e.txt`: the mpv `Copyright` file at commit `8bc468896e`. It records the GPLv2+ default, the LGPL-2.1+ terms for `-Dgpl=false` builds, and the warning that linked libraries still affect the final license.
- `native.json`: static manifest consumed by `scripts/generate-licenses.mjs` to render the Native libraries group in the licenses dialog. Every entry is marked not bundled.

## Source offer (explicit, for future distribution)

No source distribution accompanies this build because no binary is distributed. When and if Kaminari ships the binaries above, each distribution will be accompanied by either the complete corresponding source or a written offer, valid for at least three years, to provide it. The corresponding source set is: mpv at commit `8bc468896e` (or its proven successor), the proven FFmpeg commit plus the zhongfly winbuild scripts and `compile-lgpl-libmpv.patch`, the libmpv-wrapper source matching the shipped archive, and the tauri-plugin-libmpv 0.3.2 crate source. This offer statement will move into the installer and the dialog notes at P1 packaging time, after lawyer sign-off.

## Remaining legal gate

1. Lawyer sign-off on this package (texts, artifact table, FFmpeg linkage, source offer wording).
2. Closed P1 verification of the exact FFmpeg commit inside the shipped `libmpv-2.dll`.
3. Only then: P1 packaging work (bundling, installer notes, CHANGELOG entry), which is explicitly out of scope here.
