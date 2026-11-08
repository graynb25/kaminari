# Contributing to Kaminari

Thanks for being here. This project is small and hand made. That is a feature, not a flaw. Please help keep it that way.

## Before you start

Read `docs/ARCHITECTURE.md` and `docs/ENGINEERING-NOTES.md`. They have the numbers and the reasons behind the stack. If you plan to write an extension, read `docs/EXTENSION-API.md` first.

Open an issue early if the change is big or unclear. A short note is enough. Say what you want to build, why, and what you tried.

## Setup

You need Node 20 or newer, pnpm, and a Rust toolchain.

```bash
git clone https://github.com/graynb25/kaminari
cd kaminari
pnpm install
pnpm tauri dev
```

The app runs a local dev window that talks to a local SQLite file. No account needed. AniList public queries work without a key. Tracking needs your own OAuth client, see `docs/ARCHITECTURE.md`.

## How to work

- Small pull requests. One idea per PR.
- Add tests when you add logic. Rust unit tests live next to the code. Frontend tests live in `src/` next to the module.
- Update docs and `CHANGELOG.md` in the same PR.
- Keep the UI honest. Every screen needs empty, loading, error, and offline states.
- Do not use em dashes in docs, code, or PR text. Use commas or periods.

## Style

Rust: run `cargo fmt` and `cargo clippy` before you push. CI will fail on warnings.

TypeScript: strict mode, small typed modules, no `any` without a comment that says why.

UI: use tokens from `src/app.css`. Add a token before you add a one off value. Hover and motion come from `docs/DESIGN-SPEC.md`. If you break the spec, say so in the PR and show a short video or screenshot.

## Pull request checklist

- [ ] Builds locally (`pnpm tauri dev`)
- [ ] Tests pass (`cargo test`, `pnpm test`)
- [ ] Fx, Clippy, lint pass
- [ ] Docs updated
- [ ] CHANGELOG entry added
- [ ] No em dashes (style check passes)

## Tone

Be kind. Be specific. If you disagree, say what you measured. This project has reversed its own decisions before when the numbers said so. That is welcome.

## Licensing

By contributing, you agree your code ships under the MIT License in `LICENSE`. Extensions you write for the sample folder stay MIT as well unless you state otherwise in the manifest.
