# Native libmpv payload

This folder is for the local-only libmpv DLLs used by the embedded player:

- `libmpv-2.dll`
- `libmpv-wrapper.dll`

They are intentionally gitignored. The pinned hashes and download steps are in `licenses/native/README.md` and `scripts/check-libmpv-artifacts.mjs`. `pnpm check:libmpv` verifies both files before a Tauri bundle.

This README is tracked so the Tauri resource glob `lib/**/*` has a match in a clean checkout. CI runs clippy and tests without the DLLs; a real installer or portable build still requires the two files.
