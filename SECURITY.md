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

## Report a security issue

Use the private GitHub Security Advisory form:

https://github.com/graynb25/kaminari/security/advisories/new

Do not put exploit details, tokens, or personal data in a public issue. If the advisory form is unavailable, open an issue titled `[Security]` with only a request for a private channel. Include the version, platform, and a short impact summary.

We aim to read new reports within a few days and to patch within two weeks when the fix is clear. Serious sandbox escapes get fixed first, even if other work stops.

## Takedown or legal request

Kaminari ships no content, no source list, and no APKs. If something in this repository is a problem, open a takedown issue:

https://github.com/graynb25/kaminari/issues/new?template=takedown.yml

Include the exact URL or path, why the material should be removed, and a way to contact you. Do not include government identity documents, passwords, private keys, or other sensitive personal data in a public issue. Ask for a private channel and we will move the conversation there. We aim to acknowledge a complete request within a few days.

The issue tracker is public when the repository is public. Security advisories stay private.

## Secret scanning

The full git history is scanned with Gitleaks 8.30.1. The current configuration is `.gitleaks.toml`. The only historical findings were fake example tokens in an auth parser test and a proxy documentation sample. Both current files use placeholders, and the two historical commits are narrowly allowlisted. CI runs the same scan on every push and pull request.
