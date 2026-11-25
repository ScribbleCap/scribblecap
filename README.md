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

You need [Rust](https://rustup.rs) (the pinned version installs itself), Node 24 and
pnpm (`corepack enable`). On Linux, also install
[Tauri's system packages](https://v2.tauri.app/start/prerequisites/#linux).

```sh
git clone https://github.com/ScribbleCap/scribblecap
cd scribblecap
pnpm --dir ui install
pnpm --dir ui tauri dev
```

The development command builds the WebAssembly core before starting the editor.

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
