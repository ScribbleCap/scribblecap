# Contributing to ScribbleCap

ScribbleCap is an early-stage desktop capture and annotation app. Read the
[vision](../docs/vision.md), [roadmap](../docs/roadmap.md),
[architecture](../docs/architecture.md) and [AGENTS.md](../AGENTS.md) before starting.
The working rules apply to human and AI-assisted contributions alike.

## Choose a task

Pick one roadmap task with completed dependencies, or a GitHub issue. Comment on the
issue or open a [discussion](https://github.com/ScribbleCap/scribblecap/discussions)
to coordinate substantial work and resolve ambiguous requirements before coding.
Keep each PR scoped to one task; leave unrelated refactoring for another change.

Use the bug and feature forms for actionable reports. Questions and early ideas
belong in Discussions. Follow [SECURITY.md](SECURITY.md) for vulnerabilities.

## Set up and validate

Install Rust through rustup and use the pinned `rust-toolchain.toml`. Use the Node
version in `ui/.node-version` and pnpm version in `ui/package.json` (`packageManager`).
Linux also needs the [Tauri 2 system prerequisites](https://v2.tauri.app/start/prerequisites/).

Run the [commands in AGENTS.md](../AGENTS.md#commands) from the repository root.
Build the UI before Rust: the app embeds `ui/dist`. Run UI lint, tests and build,
then Rust formatting, Clippy and workspace tests. CI checks Linux, Windows and macOS;
report which platforms you actually tested locally.

Write tests first where practical and meet the task's acceptance criteria. Add
meaningful regression tests for fixes. Format changes need round-trip validation;
rendering changes need appropriate visual checks. Verify destructive redaction
does not leave recoverable source data in saved resources.

## Implementation rules

- Keep `crates/core` free of UI frameworks and operating-system APIs.
- Keep saved editable files viewable as ordinary images or PDFs.
- Update documentation with behavior, architecture or format changes, and mark a
  roadmap task done only when its acceptance criteria are met. Respect a task's
  explicit file restrictions; mention any deferred roadmap update in the PR.
- Record hard-to-reverse decisions in a new document under `docs/decisions/`.
  Never edit an accepted decision; supersede it.
- Check new dependencies for compatibility with MIT OR Apache-2.0 and list their
  licences in the PR.

## Clean-room contributions

Implement behavior independently. The documented
[reverse-engineering findings](../docs/reverse-engineering/findings.md) describe
behavior, constants and formats; they are not source code to transplant.
Never copy Evernote/Skitch code, decompiler output, images, sounds, fonts or branding
into the product. Never commit `re-work/` or a copy of `Skitch.app`. Create original
artwork and use synthetic or appropriately licensed test inputs.

You are responsible for reviewing AI-assisted work for correctness, provenance and
licensing under the same rules.

## Open a pull request

Work on a branch and fill in the PR template. Explain the problem, resulting
behavior, validation and any file-compatibility or dependency changes. Include
screenshots for visible UI changes where useful. Draft PRs are welcome for work
awaiting dependencies; identify those dependencies clearly.

Only commit files intended for the change. Do not include secrets, personal images
or private logs. Follow the [Code of Conduct](CODE_OF_CONDUCT.md).

Contributions are dual licensed under MIT OR Apache-2.0, as described in the
[README](../README.md#license).
