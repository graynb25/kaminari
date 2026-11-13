# Public release checklist

Status: the repository is public as of 2026-11-10. `v0.29.0` is the current signed release, published on 2026-11-12 from tag `v0.29.0` at `f146cca` (PR #35). It adds the airing schedule and schema version 4. The public 0.28.5 to 0.29.0 update passed. A clean Windows Sandbox pass installed 0.28.5, updated to 0.29.0, and rolled back with the marker and database intact; the sandbox image used the host portable WebView2 runtime because it could not register one. `v0.28.5` was the previous signed release; it reverted the 0.28.4 duration filter so manual watched marks returned to Continue Watching, including zero-duration mark-all rows. `v0.28.4` was the release before that. `v0.28.3` fixed source link timing, duplicate season links and manual marks in Continue Watching. `v0.28.2` added the redacted diagnostics export, history and safety-copy retention, and the accessibility pass. `v0.28.1` added DPAPI-encrypted extension storage, schema version 3 and Clear all local data. `v0.28.0` was the previous signed release, published from tag `v0.28.0` at merge commit `3b32682` (PR #29); its public assets matched `SHA256SUMS.txt`, public `latest.json` reported 0.28.0, Minisign verified the installer, CI `check` and `secrets` passed, and a real 0.27.0 install updated through the public GitHub feed to 0.28.0. Clean single-instance Windows Sandbox install and launch passes completed for 0.28.1, 0.28.2, 0.28.3 and 0.28.4. The signing key and password live in the DPAPI-encrypted vault at `D:\Data\Key Custody\kaminari-release`; the plaintext `%APPDATA%\kaminari-signing` folder is gone. Branch protection, secret scanning, push protection, Dependabot and private vulnerability reporting are enabled. Legal review and Authenticode signing are deferred by the maintainer.

## Before public

- [x] Scan the full git history with Gitleaks. The 8.30.1 run on 2026-11-10 scanned 114 commits. Two findings were verified as fake example tokens in a test and a proxy doc. The current files use placeholders and the two historical commits are narrowly allowlisted in `.gitleaks.toml`.
- [x] Add a private security advisory link and a public takedown issue template in `.github/ISSUE_TEMPLATE/`.
- [x] Keep the update private key and password outside the repository.
- [x] Move the signing key and password out of plaintext app data into a DPAPI-encrypted vault, with a loader and an export path for offline media. The vault is at `D:\Data\Key Custody\kaminari-release`; the plaintext `%APPDATA%\kaminari-signing` folder is gone.
- [x] Keep `VERSION`, the installer and the portable zip reproducible from the repository.
- [x] Generate `latest.json` from the signed installer with `pnpm release:manifest`, reject a UTF-8 BOM and verify the signed version.
- [x] Remove the tag-triggered release workflow. It cannot sign or ship the local libmpv payload. Releases are built and uploaded from the maintainer machine.
- [ ] Have counsel review `docs/LEGAL-REVIEW-DRAFT.md` and the Terms. Deferred by the maintainer; counsel review has not happened.
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
8. [x] Test the in-app update from the previous signed build. 0.26.9 found 0.27.0, downloaded it, closed, let the installer replace the files, reopened 0.27.0 and reported up to date. The signed-version requirement accepted the 0.27.0 feed. The library hash survived. Host only; the clean sandbox pass is recorded under VM checks.
9. [x] Test the public GitHub feed after 0.28.0 was published. A real 0.27.0 install found 0.28.0, downloaded it, accepted the signed version, installed it and reopened 0.28.0. The public `latest.json` reported 0.28.0.

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
- [x] L-09 data at rest: extension storage values are DPAPI-encrypted with transparent reads/writes and plaintext migration; Settings > Privacy > Data at rest explains the split and offers Clear all local data. SQLCipher was rejected because it would regress the bundled SQLite 3.54.1 fix; library and history remain plaintext and rely on BitLocker/Device Encryption. The 0.28.1 candidate uses schema version 3, so 0.28.0 and older reject its backups. Extension-storage encryption is one-way: a build without DPAPI support reads the ciphertext as an ordinary value, so release notes must also call out the live-downgrade limitation.
- [x] Source link and Continue Watching fix: opening a Watch result no longer saves it; the source row and AniList link are created on playback start or a watched mark; one source show per AniList id per extension with startup dedupe; Continue Watching requires real playback position and excludes zero-duration manual marks while keeping watched rows with a real duration.
- [x] 0.28.4: restore watched rows with a real duration to Continue Watching after the 0.28.3 filter removed them, while keeping zero-duration manual marks out.
- [x] 0.28.5 published: revert the 0.28.4 duration filter as too strict. Manual watched marks return to Continue Watching, including zero-duration mark-all rows. Source link timing and the duplicate AniList link repair from 0.28.3 stay unchanged. The public 0.28.4 to 0.28.5 update passed; no clean Windows Sandbox pass was recorded.
- [x] Production review residual risk 12: Settings > Data has a 30/90/365 days or Forever history window that prunes only AniList and source history on startup and on change; Settings > Updates and diagnostics exports a redacted local JSON report through a one-use save path; startup and a successful restore keep only the newest three `kaminari.db.safety-*` copies. The tests prove history rows exactly at the cutoff survive, watched marks, library, progress, Continue Watching and extension storage are preserved, and the report contains counts and versions without titles, URLs, paths or extension IDs. Included in 0.28.2.
- [x] Accessibility pass: one shared modal focus layer with background isolation, Tab containment, Escape and focus return; dialogs named by visible headings; keyboard paths for the palette, history range menu and quality menus; live status and error announcements; contrast and reduced-motion fixes. Settings failure statuses use alert roles and the clear-data dialog and Incognito setting are named from their headings.
- [ ] L-09 code signing: Windows Authenticode signing and published hashes. Deferred by the owner; the unsigned installer still triggers SmartScreen.
- [x] Clean Windows Sandbox install, update and rollback pass. Completed on 2026-11-11 with a 0.27.0 install, 0.28.0 upgrade, 0.27.0 rollback and final 0.28.0; the marker and database survived each step.
- [ ] Final legal review and counsel sign-off. Deferred by the maintainer; counsel review has not happened.

## 0.29.0 release

- [x] Airing schedule: Schedule rail view, next-up countdown, seven day timeline, Aired and Later states, cached AniList snapshot and detail-page next-episode line.
- [x] Schema version 4 adds `airing_schedule`. Implementation tests used temporary databases; the public update migrated the maintainer database from 3 to 4 with 45 library rows and 10 schedule rows.
- [x] 234 Rust tests and 146 frontend tests, plus fmt, clippy with warnings denied, frontend check, build, style and `git diff --check`. `cargo audit --no-yanked` exits 0 with the existing six allowed warnings.
- [x] Visual check of the schedule view in a debug build against a copied database, light and dark. The detail-page next-episode line was checked in the same debug build.
- [x] Published 2026-11-12 from tag `v0.29.0` at `f146cca` (PR #35). One PR CI run: `check` 12m23s and `secrets` 6s. The merge used `[skip ci]`; no main CI run was recorded.
- [x] Clean Windows Sandbox install, launch, update and rollback pass. The sandbox image could not register WebView2, so the pass used the host portable runtime. 0.28.5 installed and launched, 0.29.0 installed over it, and 0.28.5 rollback kept the marker and database. The 0.29.0 database was schema 4 with 17 tables and `airing_schedule`; the rollback database was schema 3.
- [x] Public update from 0.28.5 to 0.29.0 through the signed feed passed. The app reopened 0.29.0 and the schedule showed 10 shows, with Sasaki and Peeps Season 2 on Wednesday.
- [ ] Legal review, Authenticode signing and the physical offline signing-key copy remain deferred.

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

Local host result, 2026-11-11: 0.27.0 install, 0.28.0 upgrade, 0.27.0 rollback and 0.28.0 reinstall all exited 0 with the expected file versions. A marker file and the row counts for media, library, watched, history, source library, source progress, source history and extensions survived. Minisign verified the 0.28.0 installer signature against the embedded public key and the trusted comment carries `version:0.28.0`. A corrupt database produced the native recovery window with Retry, Open data folder and Quarantine and start fresh; the real database was restored afterward. The in-app update path passed against a temporary local feed built from the exact 0.27.0 release code, downloading and installing the real signed 0.28.0 installer. The public GitHub feed from 0.27.0 to 0.28.0 was tested separately after publication. This is not a clean machine result.

Clean Windows Sandbox result, 2026-11-11: `clean_before=True`; 0.27.0 install; app launched; 0.28.0 upgrade; marker and database survived; 0.27.0 rollback with marker; final 0.28.0. The first sandbox launch exited without a guest; the second completed. No sandbox processes remain. The clean Windows Sandbox pass is closed; a full Hyper-V VM pass was not run.

Clean Windows Sandbox result, 2026-11-11, 0.28.1 candidate: `clean_before=True`; 0.28.1 installed; app launched; marker and database created; the app reported version 0.28.1. The run used a single-instance guard, and no guest processes remained afterward.

Clean Windows Sandbox result, 2026-11-12, 0.29.0: `clean_before=True`; the sandbox image could not register WebView2, so the pass used the host portable WebView2 runtime; 0.28.5 installed and launched; database created; 0.29.0 installed and launched; marker and database survived; 0.28.5 rollback with marker and database intact; the 0.29.0 database copy was schema 4 with 17 tables and `airing_schedule`; the rollback copy was schema 3; no sandbox processes remained. A full Hyper-V VM pass was not run.
