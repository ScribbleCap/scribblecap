# AGENTS.md

Instructions for AI coding agents (and humans) working in this repository.

## What we are building

**ScribbleCap** (crate/binary name `scribblecap`): an open-source, cross-platform
(Linux, Windows, macOS) app to **capture, annotate and mark up images and PDFs**,
inspired by the discontinued Skitch for Mac. Full picture: [docs/vision.md](docs/vision.md).

## Read before working

| Need | Read |
|---|---|
| Why / what / what not | [docs/vision.md](docs/vision.md) |
| What to work on next | [docs/roadmap.md](docs/roadmap.md) |
| How the code is organized | [docs/architecture.md](docs/architecture.md) |
| Editable document format (JSON schema) | [docs/file-format.md](docs/file-format.md) |
| Why a decision was made | [docs/decisions/](docs/decisions/) |
| How Skitch behaved (formats, constants, geometry) | [docs/reverse-engineering/findings.md](docs/reverse-engineering/findings.md) |

## Workflow

1. Pick **one** task from `docs/roadmap.md` (or a GitHub issue) whose dependencies are done.
2. Read the docs that task links to. If the task is ambiguous, ask instead of guessing.
3. Write tests first where practical; every task has acceptance criteria — meet them all.
4. Keep the change scoped to the task. No drive-by refactors.
5. Update docs in the same change when behaviour, file formats or architecture change.
   Mark the task done in `docs/roadmap.md`.
6. A decision that is hard to reverse (new dependency, format change, platform approach)
   needs a new record in `docs/decisions/` (copy the format of an existing one).
   Never edit an *Accepted* decision; supersede it with a new one.

## Hard rules

- **Clean room.** `re-work/` (decompiler output) and any copy of `Skitch.app` are reference material only. Never copy
  Evernote code, decompiled code, images, sounds, fonts or the "Skitch" name/trademark
  into the product. Use `findings.md` for behaviour, constants and formats only.
- Never commit `re-work/` (it is in `.gitignore`) or any copy of `Skitch.app`.
- The core library must stay UI-free and platform-free: no Tauri, no OS APIs in `core`.
- Do not lose user data: saving an editable file must always produce an image or PDF
  that opens in ordinary viewers.
- Dependencies must have licences compatible with the project licence (see
  `docs/decisions/0001-tech-stack.md`). Note new ones in the PR description.
- Code must build and tests must pass on all three OSes (CI enforces this).

## Repository layout

| Path | What |
|---|---|
| `crates/core` | `scribblecap-core`: document model, geometry, flat rendering (no UI, no OS APIs) |
| `crates/formats` | `scribblecap-formats`: image formats, embedded editable document |
| `crates/capture` | `scribblecap-capture`: screen capture backends |
| `crates/cli` | `scribblecap-cli`: the `scribblecap` command-line binary |
| `app/` | `scribblecap-app`: Tauri 2 desktop shell (`tauri.conf.json`, capabilities, icons) |
| `ui/` | Editor front-end: Vite + TypeScript, built into `ui/dist` and embedded by `app` |
| `assets/icon/` | Icon source (`scribblecap.svg`); app icons in `app/icons/` are generated from it |

## Commands

Toolchains: Rust is pinned in `rust-toolchain.toml` (rustup installs it automatically);
Node version in `ui/.node-version`; pnpm version in `ui/package.json` (`packageManager`,
use `corepack pnpm` if pnpm is not installed). Linux needs Tauri 2's system packages.

Run from the repository root. Build the UI first: the app embeds `ui/dist`, so Rust
builds of `app` fail until it exists.

```sh
# UI
pnpm --dir ui install --frozen-lockfile
pnpm --dir ui lint
pnpm --dir ui test
pnpm --dir ui build

# Rust
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked

# Run
pnpm --dir ui tauri dev            # desktop app with hot reload
cargo run -p scribblecap-cli -- --version
cargo run -p scribblecap-core --example preview   # SVG sheet of core geometry -> target/preview/

# Golden tests: after an intended geometry change, regenerate and review the preview
SCRIBBLECAP_UPDATE_GOLDEN=1 cargo test -p scribblecap-core

# Regenerate app icons after changing assets/icon/scribblecap-1024.png
pnpm --dir ui tauri icon ../assets/icon/scribblecap-1024.png -o ../app/icons
```

All of these must pass before a change is ready (CI runs the same list).
