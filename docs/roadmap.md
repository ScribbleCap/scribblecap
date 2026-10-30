# Roadmap

_Each task is sized for one agent session / one pull request. Pick a task whose
dependencies are done; meet every acceptance criterion; tick it here in the same PR.
Larger work gets split into new tasks here first._

Legend: `[ ]` todo · `[~]` in progress (put your branch name) · `[x]` done

## M0 — Foundations

- [x] **M0-1 Accept decisions 0001–0004.** _Owner: maintainer._
  All four accepted 2026-10-29; name **ScribbleCap**; GitHub org `ScribbleCap` and crate
  `scribblecap` claimed. No formal trademark search done (web search found no conflict;
  no plan to register a trademark).
- [x] **M0-2 Repository scaffold.** Deps: M0-1. Must provide the commands listed in M0-3.
  Cargo workspace with `core`, `formats`, `capture`, `cli`, `app` (Tauri 2), `ui`
  (`pdf` is added in M6); `AGENTS.md` "Commands" section filled in.
  Done when: `build`, `test`, `lint` commands documented and passing locally.
- [ ] **M0-3 CI on Linux, Windows, macOS.** Can run **in parallel with M0-2**; merges
  after it (rebase onto M0-2, then confirm a green run).
  Scope: only files under `.github/` (workflows, dependabot). Do not edit other files.
  Contract with M0-2 (the scaffold provides exactly these):
  - Rust toolchain pinned in `rust-toolchain.toml`; Node version in `ui/.node-version`;
    pnpm version in `ui/package.json` `packageManager`.
  - Order: UI first (the Tauri app embeds `ui/dist`), then Rust.
    1. `pnpm --dir ui install --frozen-lockfile`
    2. `pnpm --dir ui lint` · `pnpm --dir ui test` · `pnpm --dir ui build`
    3. `cargo fmt --all -- --check`
    4. `cargo clippy --workspace --all-targets --locked -- -D warnings`
    5. `cargo test --workspace --locked`
  - Linux runner needs Tauri 2's system packages (WebKitGTK 4.1 etc.; see Tauri's
    "Prerequisites" page for Linux/Debian).
  - Workflow `.github/workflows/ci.yml`, runs on pull requests and pushes to `main`;
    jobs on `ubuntu-latest`, `windows-latest`, `macos-latest` with check names
    **`linux`**, **`windows`**, **`macos`** (the branch ruleset will require these
    names); Rust and pnpm caching.
  Done when: all three checks pass on the M0-2 scaffold; the maintainer adds the three
  checks + "require pull request" to the `main` ruleset.

## First release scope

v1 handles **raster images only**: PNG, JPEG, WebP, TIFF, BMP, GIF (first frame).
HEIF/AVIF, SVG and PDF follow after the first release (M6).

## M1 — Document model and file formats (no UI)

- [x] **M1-1 Write `docs/file-format.md` and implement the document model in `core`.**
  Elements: arrow, line, rect, rounded rect, ellipse, freehand (marker/highlighter),
  text (plain/bubble/label), stamp, step counter. JSON round-trip.
  Done when: schema documented; serialise → parse → equal for every element type.
- [x] **M1-2 Geometry: arrow outline.** Deps: M1-1. Use findings §5.3.
  Done when: golden tests for short, normal and long arrows at all 5 sizes.
- [x] **M1-3 Geometry: freehand smoothing + curve fitting.** Deps: M1-1. Findings §5.4.
  Done when: noisy input strokes produce smooth paths within tolerance; tests.
- [ ] **M1-4 Flat renderer (tiny-skia).** Deps: M1-2, M1-3.
  Done when: golden-image tests for every element type pass on all OSes.
  Split into:
  - [x] **M1-4a Shapes and vector stamps**: arrow, line,
    rectangle/rounded rectangle, ellipse, marker/highlighter and original vector
    stamps; background compositing, clipping, alpha conversion and golden images.
    API and verification: [rendering.md](rendering.md).
  - [ ] **M1-4b Text and step numbers**: deterministic bundled fonts and text layout,
    plain/bubble/label text and numbered circles, with golden images. Requires a
    text-rendering decision; M1-4 remains incomplete until both land. Decision 0006
    is allocated to the UI framework.
- [x] **M1-5 Read/write raster formats** (PNG, JPEG, WebP, TIFF, BMP, GIF first frame).
  Done when: each opens; flat save of each verified by re-opening; EXIF orientation
  applied.
  Implemented in `crates/formats`; API and export behavior: [raster-formats.md](raster-formats.md).
- _M1-6 (HEIF/AVIF) and M1-7 (SVG) moved to M6._
- [ ] **M1-8 Embed/extract editable document** in PNG, JPEG, WebP
  (decision 0003). Deps: M1-1, M1-5.
  Done when: round-trip tests; files still open in the default viewer of each OS
  (manual check listed in PR).
- _M1-9 / M1-10 (re-editing files saved by Skitch) moved to Later._

## M2 — Editor MVP (images)

- [x] **M2-1 App shell**: open file, paste image from clipboard,
  window, zoom/pan. Framework: [decision 0006](decisions/0006-ui-framework.md).
  Behavior and validation: [app-shell.md](app-shell.md).
- [ ] **M2-2 Tools**: arrow, line, rect, rounded rect, ellipse. Selection, move, resize,
  delete. Deps: M1-4.
- [ ] **M2-3 Marker and highlighter.** Deps: M1-3.
- [ ] **M2-4 Text**: plain, bubble, label attached to a shape.
- [ ] **M2-5 Colour and size pickers**, keyboard shortcuts for every tool.
- [ ] **M2-6 Undo/redo** for every operation.
- [ ] **M2-7 Crop, resize, expand canvas, rotate/flip.**
- [ ] **M2-8 Pixelate and redact** (destructive; original replaced in stored resource).
- [ ] **M2-9 Copy, drag-out, save / save as** in every supported format.
- [ ] **M2-10 Stamps and step counters.** New artwork (no Skitch assets).

## M3 — _moved to M6 (PDF annotation after the first release)_

## M4 — Capture (decision 0002)

- [x] **M4-1 `capture` trait + macOS backend** (ScreenCaptureKit, macOS 14+).
- [ ] **M4-2 Windows backend** (Windows.Graphics.Capture).
- [ ] **M4-3 Linux X11 backend.**
- [ ] **M4-4 Linux Wayland backend** (xdg-desktop-portal).
- [ ] **M4-5 Region/window picker overlay**, previous region, timed capture.
- [ ] **M4-6 Global hotkey / tray icon**, `app capture` CLI command.

## M5 — v1 polish

- [ ] **M5-1 Packaging and releases** via GitHub Actions: pushing a `v*` tag builds on
  macOS (arm64 + x86_64), Windows and Linux runners and uploads `.dmg`, `.msi`,
  AppImage and `.deb` to a draft GitHub Release (`tauri-action` if Tauri, else
  `cargo-dist`). Signing: Apple Developer ID + notarisation (paid account) and Windows
  signing via SignPath Foundation (free for OSS); unsigned builds are allowed until then,
  with the warnings documented. Flathub submission as a follow-up.
- [ ] **M5-2 Settings**, light/dark theme, HiDPI checks.
- [ ] **M5-3 Accessibility pass** (keyboard-only, screen-reader labels).
- [ ] **M5-4 CLI**: `annotate`, `render`, `convert` with docs and examples.

## M6 — After first release: HEIF/AVIF, SVG, PDF

- [ ] **M6-1 HEIF/AVIF read/write** (was M1-6), including embedding the editable
  document in a `uuid` box (decision 0003). Decide OS decoder vs
  libheif per platform (write a decision record). Done when: open + save works on all
  three OSes or the gap is documented.
- [ ] **M6-2 SVG as background** (was M1-7, resvg). Done when: SVG opens at chosen DPI,
  annotates, exports PNG.

PDF annotation (was M3, decision 0004). Adds the `pdf` crate.

- [ ] **M6-3 Open and render PDFs** (PDFium), page thumbnails, page navigation.
- [ ] **M6-4 Annotate pages** with all M2 tools.
- [ ] **M6-5 Save as standard annotations** with appearance streams + embedded document,
  as an incremental update. Done when: output checked in Acrobat Reader, macOS Preview,
  Chrome, Firefox, Okular (screenshots in PR).
- [ ] **M6-6 Real redaction** removes underlying text/images. Done when: text
  extraction of the redacted area returns nothing.
- [ ] **M6-7 Open PDFs annotated by other apps** (editable standard annotations).

## Later (not scheduled)

- **Re-edit files saved by Skitch.** Skitch 2.x saved ordinary PNG/JPEG/HEIC/PDF files
  with its editable drawing hidden inside (findings §3–4, §6); reading that data would
  let those annotations be edited again. Plus Skitch 1.x `.skitch` (SVG) import
  (findings §7). Needs sample files saved by Skitch 2.9. Low priority: few such files
  are likely still around.

OCR copy-text, auto-redact suggestions, spotlight and magnifier, emoji/custom stamps,
snap guides, GIF/video capture, mobile.
