<p align="center">
  <img src="assets/icon/scribblecap-256.png" alt="ScribbleCap icon" width="128" height="128">
</p>

# ScribbleCap

**Capture, scribble, share.** A fast, open-source app to annotate screenshots, images
and PDFs on Linux, Windows and macOS.

> 🚧 **Early development.** No packaged download yet; [try it from source](#try-it-from-source).
> Follow the [roadmap](docs/roadmap.md) for progress.

## What it will do

- **Capture** a region, window or the whole screen, or open an existing image.
- **Annotate** with arrows, lines, shapes, text and speech bubbles, marker,
  highlighter, stamps, step numbers, pixelate and redact.
- **Share** by copying, dragging out or saving. Saved files stay ordinary images that
  open anywhere, and they reopen in ScribbleCap still editable.
- **Opens** PNG, JPEG, WebP, TIFF, BMP and GIF.
- **Coming after the first release:** HEIF/HEIC, AVIF, SVG, and PDF markup with standard
  annotations that show up in Acrobat, Preview and browsers.
- **Scriptable**: a command-line mode for scripts, CI and AI agents.

See [docs/vision.md](docs/vision.md) for goals and non-goals.

## Try it from source

### Prerequisites

| Tool | Version | Check with | Notes |
|---|---|---|---|
| Rust (via [rustup](https://rustup.rs)) | 1.99.1, plus the `wasm32-unknown-unknown` target | `cargo --version` | Pinned in `rust-toolchain.toml`; rustup installs the right version and target on the first build. Install rustup, not a distro or Homebrew `rust` package. |
| Node.js | 24 | `node --version` | Pinned in `ui/.node-version`. |
| pnpm | 12.16.1 | `pnpm --version` | Pinned in `ui/package.json`; `corepack enable` provides it. |
| Git | any recent | `git --version` | |

Plus the platform tools Tauri 2 needs to build a desktop app:

**macOS**

```sh
xcode-select --install          # Xcode Command Line Tools (compiler, linker)
```

**Windows**

- [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
  with the "Desktop development with C++" workload.
- WebView2 (already installed on Windows 10 and 11).
- Rust's default `msvc` toolchain (rustup picks it).

**Linux (Debian / Ubuntu)**

```sh
sudo apt-get install build-essential curl wget file libwebkit2gtk-4.1-dev \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

Other distributions: see [Tauri's Linux prerequisites](https://v2.tauri.app/start/prerequisites/#linux).

### Install the tools

```sh
# Rust (skip if `cargo --version` already works)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"       # or open a new terminal

# pnpm, using Node 24's corepack
corepack enable
```

### Build and run

```sh
git clone https://github.com/ScribbleCap/scribblecap
cd scribblecap
pnpm --dir ui install
pnpm --dir ui tauri dev
```

The first run downloads the pinned Rust toolchain and builds the WebAssembly core,
so it takes a few minutes.

### Capturing on macOS

Capture needs Screen Recording permission. The first capture asks for it; grant it in
System Settings → Privacy & Security → Screen & System Audio Recording, then quit and
restart the app. When you run from a terminal, macOS may list the terminal app rather
than ScribbleCap.

### Troubleshooting

- **`failed to run 'cargo metadata'`**: the terminal can't find `cargo`. Run
  `source "$HOME/.cargo/env"` or open a new terminal, then check `cargo --version`.
  If it still fails, `cargo metadata --format-version 1 > /dev/null` prints the real
  error.
- **`corepack` or `pnpm` not found**: check that `node --version` shows 24.
- **Linker or `cc` errors on macOS**: install the Command Line Tools (above).

**Works today:** open, paste or drop an image; annotate; undo; copy, save and export;
drag out; crop, resize and rotate; capture (macOS, Windows, X11; Wayland in progress).
**Not yet:** PDFs, installers, signed builds. Please report problems as
[issues](https://github.com/ScribbleCap/scribblecap/issues).

## Why

Skitch made "draw an arrow on a screenshot and send it" effortless, but it's no longer
developed and never ran on Linux or Windows. ScribbleCap aims to be that tool for every
desktop, open source.

ScribbleCap is an independent project. It is not affiliated with Evernote or Skitch,
and contains no Skitch code or artwork.

## Built with

Rust for the core (document model, file formats, PDF, capture) and
[Tauri 2](https://tauri.app) for the desktop app. Details in
[docs/architecture.md](docs/architecture.md) and [docs/decisions/](docs/decisions/).

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](.github/CONTRIBUTING.md) for setup
and the full validation checklist, and [AGENTS.md](AGENTS.md) for the working guide
for humans and AI coding agents alike.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for
inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual
licensed as above, without any additional terms or conditions.
