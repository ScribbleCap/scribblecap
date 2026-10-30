# App shell (M2-1)

Svelte 5/TypeScript provides the shell inside Tauri 2; Canvas 2D displays images.
[Decision 0006](decisions/0006-ui-framework.md) records the framework/native boundary.

## Behavior

**Open image** uses a native single-file picker. Rust's formats crate decodes PNG,
JPEG, WebP, TIFF, BMP and GIF (first frame), including EXIF orientation. **Paste
image** reads an image from the system clipboard. Text-only or unavailable clipboard
content produces a recoverable message; the shell never changes the clipboard.

An image initially fits the viewport without enlarging small images. Fit repeats
that action. Clicking the percentage selects actual size (one image pixel per CSS
pixel). Zoom preserves its center/pointer anchor. Resize preserves zoom and shifts
the image with the viewport center. Canvas backing resolution follows device scale.

The current image remains after cancellation or failure. Only one input request
runs at a time; errors can be dismissed. This is a read-only shell: no source image
is changed, annotations added, or file saved by M2-1.

| Input | Action |
|---|---|
| Ctrl/⌘ O | Open |
| Ctrl/⌘ V / Paste button | Paste image |
| Ctrl/⌘ + or − / zoom buttons | Zoom around viewport center |
| Ctrl/⌘ 0 / Fit | Fit |
| Ctrl/⌘ 1 / percentage | Actual size |
| Arrow keys | Pan |
| Scroll / left-button drag | Pan |
| Ctrl/⌘ + scroll/pinch | Zoom around pointer |

Shortcuts avoid editable controls. Toolbar buttons expose labels, disabled/focus
states; the viewport is focusable for keyboard panning. The header icon is the same
generated PNG as the bundled app icon, not a substitute glyph.

## Native pixel boundary

`shell_open_image` accepts no JavaScript file path. It reads only the native picker
selection. `shell_paste_image` exposes image input only. Operations use a blocking
worker; cancellation is an empty response. Nonempty responses contain:

`SCIM`, version/width/height/name-length (u32 little-endian), UTF-8 basename, RGBA8.

The frontend validates the packet before replacing its state. Tauri's raw response
can arrive as ArrayBuffer or a byte array on WebKit; both are supported. This is an
in-process transport, not a saved file format.

Encoded reads are limited to 512 MiB; display pixels to 256 MiB (roughly 67 million
pixels). The formats decoder has its own limits. Source data, decoded pixels,
transport buffers and canvases can coexist: these are not total memory limits.
Frontend plugin commands receive no general filesystem, clipboard text or writing
permissions. Source files remain unchanged.

The browser-only development preview uses a file input and browser-supported codecs
and clipboard APIs. It is useful for layout testing but does not replace desktop
format/clipboard testing. Editable-document restoration, save/export and annotation
tools are later integration work.

## Validation

Run the commands in `AGENTS.md`: UI install/lint/test/build before workspace Rust
checks. The build includes Svelte diagnostics. Tests cover native transport variants,
transform anchors/bounds, cancelled/error/duplicate input requests, shortcuts and
redraws. Rust tests cover bounded reads, codec input and packet framing.

For a local macOS package: `pnpm --dir ui tauri build --debug --bundles app`.
Native smoke checks cover opening, cancelling, known-image paste, zoom/pan and
recovering from failure. Record native QA limitations and OS results in the PR;
Windows/Linux runtime behavior still needs testing on those OSes.

2026-10-30 macOS validation: a debug `.app` was built and its native window/controls
inspected. The maintainer confirmed Open image and Paste image both work after the
WebKit byte-array transport fix. Automated native UI capture became unreliable
(ScreenCaptureKit stream error), so zoom/drag/resize behavior is covered by the UI
interaction/transform tests rather than claimed as fully automated native QA.
