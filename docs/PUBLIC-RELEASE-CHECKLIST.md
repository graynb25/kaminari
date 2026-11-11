# Public release checklist

Status: the repository is public as of 2026-11-10. `v0.27.0` is the current published signed release. The Phase 1 and Phase 2 hardening batches are integrated on a local branch as 0.28.0 and are not yet published. Branch protection, secret scanning, push protection, Dependabot and private vulnerability reporting are enabled.

## Before public

- [x] Scan the full git history with Gitleaks. The 8.30.1 run on 2026-11-10 scanned 114 commits. Two findings were verified as fake example tokens in a test and a proxy doc. The current files use placeholders and the two historical commits are narrowly allowlisted in `.gitleaks.toml`.
- [x] Add a private security advisory link and a public takedown issue template in `.github/ISSUE_TEMPLATE/`.
- [x] Keep the update private key and password outside the repository.
- [x] Keep `VERSION`, the installer and the portable zip reproducible from the repository.
- [x] Remove the tag-triggered release workflow. It cannot sign or ship the local libmpv payload. Releases are built and uploaded from the maintainer machine.
- [ ] Have counsel review `docs/LEGAL-REVIEW-DRAFT.md` and the Terms.
- [x] Run a dependency and license audit after the final release build. Dependabot alerts were reviewed on 2026-11-10; zip, Vite and Vitest were upgraded, glib was dismissed as Linux-only, and deepmerge as a build-time low severity report. Licenses were regenerated for 0.26.6.
- [x] Confirm no source list, media, APK or user database is tracked.
- [x] Confirm the CI secret scan passes on the exact release commit.
- [x] Prepare a short takedown and security response process.

## Immediately after public

The repository is public. Current state:

1. [x] Branch protection on `main`: pull requests required, `check` and `secrets` required, linear history, no force pushes, no deletions, admins included.
2. [ ] Signed commits and tags. Not enabled; the maintainer commits are not currently signed.
3. [x] Secret scanning and push protection are enabled. [x] Dependabot alerts and security updates are enabled. Non-provider patterns and validity checks are not available on the current plan.
4. [x] Private vulnerability reporting is enabled.
5. [ ] Confirm two-factor authentication on the maintainer account. This cannot be checked from the repository API.
6. [x] The tag-triggered release workflow was removed. Releases are built locally with the libmpv payload and signing key.
7. [x] `v0.26.2` and `v0.26.3` have the signed installer, `.sig`, `latest.json` and portable zip. `v0.26.3` also has `SHA256SUMS.txt`.
8. [x] Test the in-app update from the previous signed build. 0.26.9 found 0.27.0, downloaded it, closed, let the installer replace the files, reopened 0.27.0 and reported up to date. The signed-version requirement accepted the 0.27.0 feed. The library hash survived. Host only; the clean VM pass is still open.

## Production review Phase 0

- [x] H-01: extension commands run off the Tauri main thread with per-source limits.
- [x] H-02: `html.select` node, text, HTML, attribute and output caps.
- [x] H-03: external mpv URL and subtitle validation.
- [x] M-02: AniList OAuth one-time state with expiry.
- [x] M-07: validated backup restore, safety copy and disabled restored extensions.
- [x] M-12: updater signed-version binding and key custody documentation.
- [ ] M-17 legal: replace the placeholder contact, jurisdiction and repository details, then have counsel review.
- [x] M-17 NSFW: persist and show NSFW metadata and hide NSFW sources by default.
- [x] M-18: durable AniList-to-source links and ID-based Continue Watching merge.
- [x] M-01: dialog-scoped one-use path tokens for settings, backups and package installs.
- [x] M-03, M-04, M-14: shared safe fetch for registry, Mangayomi and downloads.
- [x] M-19: bundle SQLite 3.54.1 through rusqlite 0.41.1.
- [x] M-05: proxy shared entries, constant-time child reuse and output caps.
- [x] M-06: extension update review and Rust-computed trust.
- [x] M-08: source row cleanup and orphan hiding.
- [x] M-09: episode and progress clamps.
- [x] M-10: serialized AniList sync with bounded batches and backoff.
- [x] M-11: negative watch actions no longer create unexpected AniList writes.
- [x] M-13: pinned CI actions, gitleaks checksum, read-only permissions, frontend tests and audits.
- [x] M-15: host allowlist path, port and wildcard rules.
- [x] M-16: source metadata, duration and row caps.
- [x] L-01: Incognito recent searches, paused background sync, Clear WebView data and privacy-policy disclosure.
- [x] L-02: portable AniList callback registration with explicit consent.
- [x] L-03: transactional `continue_visible` migration with a repair marker.
- [x] L-04: `create_new` download files, random part names, reserved-name and reparse-point rejection, hard concurrency cap.
- [x] L-05: shared `create_new` temporary file and atomic replace for settings and backups.
- [x] L-06: CSP `worker-src`, Svelte warnings and shared normalization helpers.
- [x] L-07: dev harnesses and unused wrappers removed; legacy registry and source commands are debug-only.
- [x] L-08: native startup recovery dialog and corrected logging documentation.
- [ ] L-09: Windows Authenticode code signing and an explicit data-at-rest decision.
- [ ] Clean Windows VM install, update and rollback pass.
- [ ] Final legal review and counsel sign-off.

## Release assets

- `Kaminari_<version>_x64-setup.exe`
- `Kaminari_<version>_x64-setup.exe.sig`
- `latest.json`
- `Kaminari_<version>_x64-portable.zip`

The portable zip is installer-free but not data-free. Settings and the SQLite library stay in the normal per-user app data folder.

## VM checks

The clean VM pass covers:

1. Install the previous signed build with the NSIS installer.
2. Create a marker in the app data folder and confirm the app starts.
3. Install the new signed build over it. Confirm the version and the marker survive.
4. Reinstall the previous build as a rollback. Confirm the version, marker and database file survive.
5. When a public signed feed exists, use the in-app Check for updates, download, install and restart path.

Local host result, 2026-11-10: 0.26.1 install, 0.26.2 upgrade and 0.26.1 rollback all exited 0 with the expected file versions. The SQLite hash was identical before, after upgrade, after rollback and after uninstall. Uninstall removed the install directory and kept the data. This is not a clean machine result.

Windows Sandbox failed to produce a guest or result file on two attempts. The clean VM pass remains open. The host result is recorded in `docs/ENGINEERING-NOTES.md`.
