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

## Versioning

Edit the root `VERSION` file, never the version strings by hand. `pnpm build` and `pnpm tauri` run `scripts/sync-version.mjs` before they start, which writes `package.json` and `src-tauri/Cargo.toml`. Tauri reads `package.json`, Vite injects `__APP_VERSION__` into the frontend, and Rust reads `CARGO_PKG_VERSION`. `src-tauri/build.rs` fails a direct cargo build if `VERSION` and `Cargo.toml` disagree.

## Licenses

Run `pnpm licenses:build` after dependency changes. It reads `Cargo.lock`, `cargo metadata` and the installed node_modules, then regenerates `public/licenses.json` with package names, versions, license identifiers, sources and any full license text found on disk. Commit the regenerated file. The Settings and footer Licenses dialogs read it at runtime.

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
- [ ] `VERSION` updated once, not version strings by hand
- [ ] `pnpm licenses:build` run after dependency changes
- [ ] No em dashes (style check passes)

## Tone

Be kind. Be specific. If you disagree, say what you measured. This project has reversed its own decisions before when the numbers said so. That is welcome.

## Licensing

By contributing, you agree your code ships under the MIT License in `LICENSE`. Extensions you write for the sample folder stay MIT as well unless you state otherwise in the manifest.
