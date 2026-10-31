# 0006 — Svelte for the desktop UI

_Status: **Proposed** · 2026-10-30 · implementation: M2-1_

## Context

Decision 0001 selected Tauri 2, TypeScript and Canvas 2D, leaving the component
framework open. M2 needs reactive toolbars, accessible controls, asynchronous input
state and an image viewport. The maintainer proposed Svelte for M2-1.

0006 was previously reserved for text rendering in the roadmap/0007. This record
allocates it to the UI framework at the maintainer's request; export fonts/layout
will get a separate decision. Accepted records remain unchanged.

## Options

| Option | Benefits | Costs |
|---|---|---|
| Plain TypeScript | No framework; matches scaffold | Manual state/DOM synchronization grows with editor interactions |
| Svelte 5 | Small compiled components, TypeScript, reactive state and accessibility diagnostics | New syntax/compiler tooling |
| React | Large ecosystem and contributor familiarity | More runtime surface for a small local editor |

## Decision

- Use **Svelte 5 + TypeScript** through the Svelte Vite plugin inside Tauri 2.
  No SvelteKit, server, router or network service is needed.
- Keep native dialogs, clipboard access and image decoding in Rust. Use the official
  Tauri dialog/clipboard plugins; run blocking input/decoding on a worker thread.
- Pass a versioned raw pixel packet: dimensions, UTF-8 basename and straight RGBA8.
  Accept ArrayBuffer and WebKit's byte-array response transport. Absolute file paths
  are not exposed to the UI.
- Paint image pixels with Canvas 2D. UI code owns viewport transforms only; annotation
  geometry stays in Rust. Core remains free of Svelte, TypeScript and OS APIs.
- Add `svelte-check` to the existing UI build command and lint components with
  `eslint-plugin-svelte`. Use Vitest/jsdom/Testing Library for interaction tests.
- Reuse the app's generated icon in the header. UI system fonts are independent of
  the later deterministic export-font choice.

## Consequences

- Runtime frontend dependencies: `svelte` (MIT) and `@tauri-apps/api` (MIT OR
  Apache-2.0). Rust plugins: `tauri-plugin-dialog` and
  `tauri-plugin-clipboard-manager` (MIT OR Apache-2.0).
- Development dependencies: Svelte Vite plugin/checker/ESLint plugin, Testing Library
  and jsdom (MIT). Lockfiles record their transitive graphs.
- No filesystem/clipboard write/text permissions are granted to frontend plugin
  commands. Application commands expose only the selected file and clipboard image.
- The static `ui/dist` build and CI command contract stay unchanged. Frontend tests
  now exercise real shell behavior instead of passing an empty suite.
- M2-1 is read-only: open/paste and inspect images with fit/zoom/pan. Annotation
  tools, save/export, undo and capture integration remain separate tasks. Editable
  containers display their normal flattened preview until M1-8 is integrated; the
  shell never writes or modifies source files.
