# Architecture

_Status: accepted stack ([0001-tech-stack](decisions/0001-tech-stack.md)); module layout proposed._

## Shape of the system

```
┌───────────────────────────── desktop app (Tauri 2) ─────────────────────────────┐
│  UI (TypeScript, Canvas 2D)                                                       │
│   tools · selection · toolbar · page thumbnails · live rendering of paths         │
│        │  commands / document snapshots (JSON)          ▲ path geometry           │
│        ▼                                                 │                        │
│  app shell (Rust)  ── clipboard · drag-out · dialogs · settings · hotkeys         │
│        │                                                                          │
│        ├── capture (Rust, per-OS backends)                                        │
│        └── core (Rust library, also compiled to WASM for the UI)                  │
└───────────────────────────────────────────────────────────────────────────────────┘
                     ▲
     CLI (Rust) ─────┘   uses the same core: annotate / render / convert headless
```

## Crates / packages

| Unit | Responsibility | Must not depend on |
|---|---|---|
| `core` | Document model, JSON (de)serialisation, undo history, **geometry** (arrow outline, freehand smoothing + curve fit, shapes, text layout boxes), flat rendering for export (tiny-skia) | UI, OS APIs |
| `formats` | Read/write images (PNG, JPEG, HEIF/AVIF, WebP, TIFF, BMP, GIF, SVG→raster), embed/extract the editable document | UI, OS APIs |
| `pdf` (added in M6) | Render PDF pages, write standard annotations + embedded document as an incremental update | UI, OS APIs |
| `capture` | Screen/window/region capture, one backend per OS | UI |
| `app` | Tauri shell: windows, menus, clipboard, drag-and-drop, settings, hotkeys | — |
| `ui` | Editor front-end | OS APIs (goes through `app`) |
| `cli` | Headless commands for scripts and agents | UI |

Layout on disk: `crates/{core,formats,capture,cli}`, `app/` (Tauri), `ui/`; see the
table in `AGENTS.md`.

## Key ideas

- **One geometry, two painters.** The core turns every element into plain path
  commands. The UI paints those paths with Canvas 2D for live editing; export paints the
  same paths with tiny-skia. Because both draw identical paths, on-screen and exported
  output match without two implementations of each tool.
- **Document = JSON tree** (layers → elements), close to Skitch's model (findings §4),
  which keeps a later Skitch re-edit feature simple. Every element also stores a ready-to-draw `path`, so
  older or simpler readers can render elements they don't understand.
- **Commands for undo.** Every edit is a command applied to the document in `core`;
  undo/redo is a command stack, not UI state.
- **Background is immutable** except through explicit destructive operations (crop,
  pixelate/redact, resize), each of which is an undoable command.

## Data flow: open → edit → save

1. `formats`/`pdf` detects the file type and returns a background (bitmap or PDF pages)
   plus our embedded document if one exists.
2. `core` builds the document; the UI renders it.
3. On save: `core` renders a flat preview, `formats`/`pdf` writes the target format and
   embeds the JSON document (when the format supports it).

## Testing

- `core`, `formats`, `pdf`: unit tests + golden-image tests (render → compare PNG).
- Round-trip tests: open → save → open gives an identical document, for every format.
- CI on Linux, Windows, macOS for every change.
