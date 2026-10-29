# Vision

_Status: draft, 2026-10-29. Owner: project maintainer._

## Problem

Skitch was the fastest way to grab something on screen, draw an arrow on it and send it
on. Evernote stopped developing it; the last Mac build is Intel-only and there was never
a good equivalent on Linux. Existing tools are either capture-first with weak editing
(Flameshot, Greenshot) or full image editors that are too slow for "point at this".

## What the app is

A small, fast desktop app for Linux, Windows and macOS that lets you:

1. **Open or capture** an image or PDF.
2. **Annotate** it with arrows, shapes, text, highlighter, stamps, pixelate/redact.
3. **Share** it: copy, drag out, or save — and **reopen it later still editable**, while
   the saved file stays an ordinary image/PDF that opens anywhere.

## Who it is for

People who explain things with pictures: developers filing bugs, support staff, teachers,
designers giving feedback, anyone writing documentation.

## Goals

- **Skitch-level speed:** from capture to annotated copy in a few seconds; one-key tools.
- **Works on all three desktop OSes** with native look-and-feel where it matters
  (shortcuts, clipboard, drag-and-drop, file dialogs).
- **Opens every common format** (see below) and saves back to it where possible.
- **Editable-but-normal files:** annotations are stored inside the PNG/JPEG/HEIF/PDF so
  the file is still viewable everywhere and re-editable in the app.
- **PDF annotation** that other PDF viewers can see (standard PDF annotations).
- **Re-edit later:** ScribbleCap's own annotations stay editable after saving, inside the
  normal image file (no separate project file needed).
- Scriptable: a CLI that applies annotations from a JSON file, so other tools and AI
  agents can produce annotated screenshots.

## Non-goals

- A general image editor (no layers panel, filters, brushes, colour correction).
- Editing the contents of SVG or PDF files (we annotate *on top*, we do not edit their
  own shapes or text).
- Cloud sync or accounts.
- Mobile apps (may revisit after v1).
- Video/GIF recording (may revisit after v1).

## Formats

| Format | Open | Save editable | Save flat |
|---|---|---|---|
| PNG | ✓ | ✓ (private chunks) | ✓ |
| JPEG | ✓ | ✓ (trailer after end of image) | ✓ |
| HEIF / HEIC | ✓ | ✓ (`uuid` box) | ✓ |
| AVIF | ✓ | ✓ (`uuid` box) | ✓ |
| WebP | ✓ | ✓ (RIFF chunk) | ✓ |
| TIFF | ✓ | — | ✓ |
| BMP | ✓ | — | ✓ |
| GIF | ✓ (first frame) | — | ✓ |
| SVG | ✓ (rendered as background) | — | ✓ as PNG |
| PDF | ✓ (all pages) | ✓ (standard annotations + embedded document) | ✓ |

**First release:** PNG, JPEG, WebP, TIFF, BMP, GIF. HEIF/AVIF, SVG and PDF come in the
release after (roadmap M6).

"Save editable" details: [docs/decisions/0003-save-format.md](decisions/0003-save-format.md).

## Features

### Skitch parity (v1)

- Tools: arrow (optional head at start), line, rectangle, rounded rectangle, ellipse,
  marker, highlighter, text (plain, bubble, label attached to a shape), stamps,
  pixelate, crop, resize canvas, expand canvas, rotate/flip.
- Palette of 8 colours + custom; 5 sizes.
- Select, move, resize, recolour, delete; undo/redo.
- Copy to clipboard, drag out, save as.
- Capture: region, previous region, window, full screen, timed.
- Multi-page PDF with page thumbnails (after the first release).

### New, modern features (candidates — prune before v1)

- **Redact** (solid fill, not just pixelate) and **blur**; optional auto-detect of
  emails/IP addresses/faces to redact.
- **Step counters** (numbered circles 1, 2, 3…) that renumber automatically.
- **Spotlight** (dim everything except a region) and **magnifier** callout.
- **OCR**: copy text out of a screenshot; search text in images.
- Emoji stamps and custom image stamps.
- Snap/align guides; keep shapes straight with Shift.
- Light and dark themes; HiDPI everywhere.
- Export to WebP/AVIF for small files.
- **CLI / headless mode**: `app annotate in.png --spec notes.json -o out.png` and
  `app render` for CI and agents.
- Accessibility: keyboard-only operation, screen-reader labels.

## Success measures

- Capture → annotated → copied in under 5 seconds for a practised user.
- Opens a 20 MP photo (and, once PDF lands, a 100-page PDF) without stalling the UI.
- Files saved by the app open correctly in the default viewer on each OS.
