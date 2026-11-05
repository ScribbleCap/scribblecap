# 0011 — Core library in the editor UI as WebAssembly

_Status: **Proposed** · 2026-11-04 · implementation: M2-2_

## Context

The editor must paint annotations with exactly the geometry the flat export uses
("one geometry, two painters", [architecture.md](../architecture.md)), and decision
0006 keeps annotation geometry in Rust. The UI therefore needs the core library's
paths for every shape, on every pointer move while drawing. The architecture already
foresaw "core, also compiled to WASM for the UI"; the maintainer chose that over
Tauri commands (2026-11-03).

## Options

| Option | Benefits | Costs |
|---|---|---|
| Tauri commands to the app process | No new build target | IPC round trip per pointer move; no core in the browser preview or UI tests |
| WASM with `wasm-bindgen` | Generated typed bindings | New dependency, plus `wasm-bindgen-cli` pinned to the exact same version on every machine and CI |
| WASM with a hand-written JSON ABI | No new dependency or tool; only the `wasm32-unknown-unknown` target | A small `unsafe` boundary to maintain by hand |

## Decision

- New crate `crates/core-wasm` (`scribblecap-core-wasm`), built for
  `wasm32-unknown-unknown`. The document lives inside the module; the UI sends JSON
  requests (`reset`, `add`, `update`, `remove`, `paint`, `document`) and paints the
  SVG path data it gets back with Canvas 2D. Protocol: [editor.md](../editor.md).
- The ABI is three exports: `sc_alloc`, `sc_call`, `sc_free`. Every buffer crossing
  it is a `Box<[u8]>`. `unsafe` is confined to those exports in
  `crates/core-wasm/src/lib.rs`, each with a `SAFETY` note; all editing logic is in a
  safe module tested natively.
- No new crates or tools. `rust-toolchain.toml` lists the `wasm32-unknown-unknown`
  target, so rustup installs it with the pinned toolchain.
- `pnpm --dir ui dev|build|test` run `ui/scripts/build-wasm.mjs` first (a release
  build, copied to the git-ignored `ui/src/generated/core.wasm`). CI installs the
  Rust toolchain before the UI steps.
- The app's CSP allows `'wasm-unsafe-eval'` in `script-src`, which permits compiling
  the bundled module and nothing else.

## Consequences

- UI tests run the real core: drawing tests exercise production geometry.
- `pnpm` UI commands now need the Rust toolchain installed.
- The module is about 1.9 MB (750 kB gzipped), mostly the bundled font; it is
  loaded locally, once, when the first image opens.
- The protocol is JSON, so each pointer move serialises one shape and its paths;
  negligible for annotation-sized shapes. Revisit only if profiling shows a cost.
- Undo/redo (M2-6) extends the same protocol with commands kept in the module.
