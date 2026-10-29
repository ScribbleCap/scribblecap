# 0001 — Tech stack

_Status: **Accepted** · 2026-10-29_

Maintainer decisions: Rust for all non-UI code (speed; the maintainer can work on Rust
directly), and **Tauri 2 + TypeScript** for the UI (see "UI toolkit" below).

## Context

Must run on Linux (X11 and Wayland), Windows and macOS (Apple Silicon and Intel); must
be fast with large images and multi-page PDFs; must be easy for contributors and AI
agents to work on; should produce small installers.

## Options

| Option | For | Against |
|---|---|---|
| **Rust core + Tauri 2 (web UI)** | Small binaries; Rust core reusable by a CLI and as WASM; good crates for images (`image`, `png`, `tiny-skia`, `resvg`) and PDF (`pdfium-render`, `lopdf`); web UI is fast to build | Webview differences per OS; IPC cost for big images (avoid by keeping rendering in the UI) |
| Qt 6 (C++ or Python) | Mature, native widgets, proven for this type of app (Flameshot, ksnip) | C++ is slower to develop and riskier for agents; Qt licensing (LGPL) and deployment size |
| Flutter | One UI codebase, good canvas | Desktop is second-class; weaker image/PDF libraries |
| Electron | Easiest UI | Large downloads and memory use |

## Decision

Rust for `core`, `formats`, `pdf`, `capture`, `cli`; Tauri 2 for the desktop app with a
TypeScript UI drawing on Canvas 2D. The core is compiled to WASM for in-UI geometry.

Licence: **MIT OR Apache-2.0** for our code. HEIF/AVIF via `libheif` (LGPL, dynamically
linked) and PDF via PDFium (BSD-3) are compatible.

## UI toolkit (decided: Tauri 2)

| Option | For | Against |
|---|---|---|
| Tauri 2 + TypeScript | Fastest way to build polished UI; good text input and IME; native menus and dialogs | Two languages; the maintainer would need TypeScript to work on the UI |
| egui (eframe) | All Rust; immediate mode makes drawing tools very simple; GPU rendered | Looks less native; weaker text editing and accessibility |
| Slint / iced | All Rust; retained mode; more native-looking than egui | Smaller ecosystems; custom canvas work needed |

Tauri chosen for: the best text editing, IME and accessibility (text tools and
labels matter in this app), native menus and dialogs, a polished UI with less effort, and
`tauri-action` for releases. The cost (a TypeScript UI) is kept small by putting all
geometry and document logic in Rust (`core`, compiled to WASM for the UI).

## Consequences

- Contributors need Rust + Node toolchains.
- HEVC decoding (HEIC) may have patent constraints in some distributions; use the OS
  decoder where available and libheif elsewhere (decide in M1).
