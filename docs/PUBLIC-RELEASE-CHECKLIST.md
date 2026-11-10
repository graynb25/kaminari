# Public release checklist

Status: the repository is public as of 2026-11-10. `v0.26.7` is the current signed release. Branch protection, secret scanning, push protection, Dependabot and private vulnerability reporting are enabled.

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
8. [x] Test the in-app update from the previous signed build. 0.26.4 found 0.26.5, downloaded it, closed, let the installer replace the files, reopened 0.26.5 and reported up to date. The library hash survived. Host only; the clean VM pass is still open.

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
