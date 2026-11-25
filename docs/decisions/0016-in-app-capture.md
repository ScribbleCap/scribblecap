# 0016 — In-app capture: freeze, then select

_Status: Accepted · 2026-11-25_

## Context

M4-1 to M4-4 gave `scribblecap-capture` backends for macOS, Windows, X11 and Wayland,
but the app could not use them. M4-5 connects them (design:
`docs/superpowers/specs/2026-11-24-m4-5-capture-design.md`). The choices below are hard
to reverse: new window types, a settings file, a dependency and a file-naming rule.

## Decision

**Freeze, then select (approach A).** On macOS, Windows and X11 the app hides its
editor window, captures every display with `capture_screen(Some(id))`, and shows one
borderless, always-on-top overlay window per display containing that display's frozen
picture. The selection is cropped in Rust from the frozen pixels, so what the user
selects is what they get; menus open at the time stay in the picture. Window mode crops
the window outline from the frozen picture; `capture_window` is not used. Rejected: a
transparent live overlay followed by `capture_region` (races the compositor when
hiding the overlay; unreliable transparent full-screen windows on X11), and
capture-then-crop in the editor everywhere (an extra step on every capture).

**Wayland (approach C).** There the app sends one portal request
(`capture_screen(None)`, decision 0012) and the user crops in the editor with Crop.
Window and Previous region are disabled with a reason. No overlay.

**New window types.** The overlay (one per display, label `capture-overlay-<n>`) and a
small countdown badge (`capture-countdown`) are extra Tauri webview windows that load
the same UI bundle, selected by `?overlay=` and `?countdown=` query parameters. They
get a narrow capability file (`app/capabilities/capture.json`). A session in Rust owns
a busy flag, a 10 second watchdog that closes overlays if the page never reports
ready, and a guard that restores the editor window on every exit path. Overlay and
badge are drawn as our own SVG/CSS.

**Settings file.** Settings (capture folder, auto-save, off by default) are JSON in
`settings.json` in the app config folder (`app_config_dir`). A missing or corrupt file
yields defaults. Writes are atomic. No new dependency is needed for this.

**Dependency.** `chrono` 0.4 as a direct dependency of `scribblecap-app`, with
`default-features = false` and `features = ["clock"]`, for local capture times. Licence
MIT OR Apache-2.0, compatible with decision 0001; it was already in the lockfile.

**File naming and no-overwrite.** A capture is named `Screenshot YYYY-MM-DD HH.MM.SS`
(local time), saved as `<capture folder>/Screenshot YYYY-MM-DD HH.MM.SS.png`; if taken,
`Screenshot YYYY-MM-DD HH.MM.SS.mmm.png`; if that is taken too, wait 1 ms and retry
with a new time. New captures are written with `output::save_new`, which never
overwrites an existing file (it creates a temporary file and links it into place with
`persist_noclobber`). Later Saves of the same capture overwrite that file; Save As
starts in the capture folder. Auto-saved and first-saved captures are editable PNGs
(decision 0003).

## Consequences

- Each capture holds every display's pixels in memory until it ends (bounded by
  the existing 256 MiB per-image limit per display).
- `save_new` relies on a hard link on Unix, so it can fail on filesystems without
  them (some exFAT or network folders). The error is shown; the user can pick another
  folder in Settings.
- A permission error on macOS is shown as text naming System Settings. There is no
  link, because opening System Settings needs an opener plugin (a new dependency and
  its own decision record). Follow-up.
- An untouched capture counts as clean, so Capture followed by Capture replaces it
  without asking, even with auto-save off. This follows the design spec; whether it
  should ask is an open maintainer question.
- Overlay placement on mixed-DPI setups, and on Windows and X11, is not verified on
  real desktops; see the checklist in `docs/app-shell.md`.
- Adds a direct dependency on `chrono`. M4-6 (hotkey, tray, CLI) can start the same
  Rust capture flow without the editor being visible.
