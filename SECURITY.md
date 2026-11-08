# Security

Kaminari is a desktop app that loads JavaScript extensions and talks to AniList. This page says what to expect and how to report a problem.

## Expectations

- No telemetry. No analytics. No crash upload by default. The app works offline after its first run except for AniList and your chosen sources.
- OAuth tokens for AniList live in the OS keychain, not in plain text files. Diagnostics bundles redact them.
- Extensions run sandboxed. They can use `net.fetch`, `html.query`, `crypto.sha256`, `storage`, and `logger`. They cannot read the filesystem, spawn processes, or read env vars live.
- Updates are signed. The updater checks Minisign signatures against the key in the repo before it installs anything.

## Scope

In scope: the Tauri app, the Rust core, the Svelte frontend, the sample extensions, and the registry files in this repo.

Out of scope: third party extension repos you add yourself, AniList itself, and sites your extensions talk to. If a source site rejects traffic, that is not a Kaminari vulnerability.

## Report an issue

Do not open a public issue for a security problem. Email the maintainer at the address in the repo, or open a private security advisory on GitHub. Include steps to reproduce and the version you ran.

We aim to read new reports within a few days and to patch within two weeks when the fix is clear. Serious sandbox escapes get fixed first, even if other work stops.
