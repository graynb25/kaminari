# Public release checklist

Status: the repository is private. This checklist separates what can be done now from what only becomes possible after the repository and releases are public.

## Before public

- [x] Scan the full git history with Gitleaks. The 8.30.1 run on 2026-11-10 scanned 114 commits. Two findings were verified as fake example tokens in a test and a proxy doc. The current files use placeholders and the two historical commits are narrowly allowlisted in `.gitleaks.toml`.
- [x] Add a private security advisory link and a public takedown issue template in `.github/ISSUE_TEMPLATE/`.
- [x] Keep the update private key and password outside the repository.
- [x] Keep `VERSION`, the installer and the portable zip reproducible from the repository.
- [ ] Have counsel review `docs/LEGAL-REVIEW-DRAFT.md` and the Terms.
- [ ] Run a dependency and license audit after the final release build.
- [ ] Confirm no source list, media, APK or user database is tracked.
- [ ] Confirm the CI secret scan passes on the exact release commit.
- [ ] Prepare a short takedown and security response process.

## Immediately after public

Branch protection and repository security settings are only available once the repository is public. Do these in the first minutes after the switch:

1. Enable branch protection on `main`: require pull requests, require the CI check, disallow force pushes and deletions.
2. Require signed commits and signed tags if the account supports it.
3. Enable GitHub secret scanning, push protection and Dependabot alerts.
4. Enable private vulnerability reporting.
5. Require two-factor authentication for the maintainer account.
6. Confirm the release workflow only publishes from a protected tag and a reviewed commit.
7. Confirm the GitHub release contains the signed installer, `.sig`, `latest.json` and the portable zip.
8. Test the update check from the previous signed build on a clean VM.

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
