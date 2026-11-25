# App shell and file output

Svelte 5/TypeScript provides the shell inside Tauri 2; Canvas 2D displays images.
[Decision 0006](decisions/0006-ui-framework.md) records the framework/native boundary.

## Behavior

**Open image** uses a native single-file picker. Rust's formats crate decodes PNG,
JPEG, WebP, TIFF, BMP and GIF (first frame), including EXIF orientation. **Paste
image** reads an image from the system clipboard. Text-only or unavailable clipboard
content produces a recoverable message. Copy image writes pixels only when explicitly
requested.

An image initially fits the viewport without enlarging small images. Fit repeats
that action. Clicking the percentage selects actual size (one image pixel per CSS
pixel). Zoom preserves its center/pointer anchor. While the image is fitted (after
opening, pasting or Fit), resizing the window re-fits it, as native image viewers
do; once the user zooms or pans, resize preserves zoom and shifts the image with
the viewport center. Canvas backing resolution follows device scale.

Colours follow the system light/dark setting (`prefers-color-scheme`) through
CSS tokens in `ui/src/style.css`; focus uses the system accent colour where the
webview supports `AccentColor`. The image has a soft shadow and a hairline edge so
dark images stay distinct from the dark canvas.

The current image remains after cancellation or failure. Only one input request
runs at a time; errors can be dismissed. Opening does not write source files.
Explicit Save actions write the destination.

| Input | Action |
|---|---|
| Ctrl/⌘ O | Open |
| Ctrl/⌘ V / Paste button | Paste image |
| Ctrl/⌘ C | Copy the annotated image |
| Ctrl/⌘ S / Ctrl/⌘ Shift S | Save / Save As |
| Ctrl/⌘ + or − / zoom buttons | Zoom around viewport center |
| Ctrl/⌘ 0 / Fit | Fit |
| Ctrl/⌘ 1 / percentage | Actual size |
| Arrow keys | Pan (move the selection when one exists, see [editor.md](editor.md)) |
| Scroll / Space + drag / middle-button drag | Pan (left-button drag draws or selects since M2-2) |
| Ctrl/⌘ + scroll/pinch | Zoom around pointer |

Shortcuts avoid editable controls. Toolbar buttons expose labels, disabled/focus
states; the viewport is focusable for keyboard editing and panning.

## Copy, save and drag-out

Copy image (Ctrl/⌘ C), Save (Ctrl/⌘ S) and Save As (Ctrl/⌘ Shift S) use the
current WASM core document and source RGBA pixels. Rust's core renderer produces
the complete image; selection handles and viewport zoom are excluded. The Share
menu also offers Export flat. Output operations serialize with input and editing,
and cancellation or failure leaves the current image and annotations intact.

Save always writes an editable PNG. It goes straight back to the opened file only
when that file is already an editable PNG; for any other file (a photo, a plain
PNG) or a pasted image, the first Save opens a dialog suggesting `<name>.png`, so
the original is never replaced. Later Saves go to that PNG. Save As always asks.
Export flat offers PNG, JPEG, WebP, TIFF, BMP and GIF as separate file types and
writes a flat image in the chosen one; a name typed without an extension becomes
PNG. Encoding completes before a same-folder temporary file is written, synced and
atomically persisted. A replaced file keeps its permissions; a new file gets 0644
on macOS and Linux. Saved PNGs include the original normalized PNG background.

Imported editable documents and their binary resources are preserved. This session
adds annotations above the imported preview; new element IDs are remapped before
merging them into the original document. Imported elements cannot yet be selected
in the UI. Damaged imported metadata is an error, rather than silently treating the
file as flat. Export flat refuses to replace existing editable or damaged files,
including when reached through an alias, and keeps the Save destination unchanged.

The thumbnail tab at the image corner starts a native PNG file drag while the
primary button is held. It includes current annotations and editable data. Releasing
before preparation finishes cancels dispatch. Keyboard activation copies instead.
Rust allocates gesture IDs so cancellation continues to work after a webview reload.
The newest five share directories remain available for asynchronous recipients;
older directories are removed once their native drag has also finished.
See [decision 0013](decisions/0013-native-file-output.md) for native dependencies.

Runtime smoke checks remain required on macOS, Windows and Linux: paste an image,
draw an arrow, copy into another application; save/reopen PNG, JPEG and WebP;
export flat TIFF, BMP and GIF; drag into a file manager and a chat/email application;
cancel Save As; verify a failed save preserves the original. GTK delivery requires
a real Linux session, in addition to passing Linux CI.

## Native pixel boundary

`shell_open_image` accepts no JavaScript file path. It reads only the native picker
selection. `shell_paste_image` exposes image input only. Operations use a blocking
worker; cancellation is an empty response. Nonempty responses contain:

`SCIM`, version/width/height/name-length (u32 little-endian), UTF-8 basename, RGBA8.
Version 1 has a 20-byte header. Version 2 adds a flags word at byte 20 (24-byte
header), whose bit 0 identifies an imported editable document held by the native
session. Unknown flags and versions are rejected.

The frontend validates the packet before replacing its state. Tauri's raw response
can arrive as ArrayBuffer or a byte array on WebKit; both are supported. This is an
in-process transport, not a saved file format.

Copy, save, drag, privacy edits and canvas transforms send pixels through Tauri's raw binary
request body. The `SCOP` version 1 envelope contains a 12-byte header (magic,
u32 little-endian version and metadata byte length), UTF-8 JSON metadata, then
straight RGBA8 bytes. Dimensions and command-specific options live in metadata;
pixels are never expanded into a JSON number array. Both sides enforce the
256 MiB pixel limit, exact dimensions and a 16 MiB metadata limit. Rust rejects
JSON request bodies and unsupported packet versions. See
[decision 0014](decisions/0014-bounded-sharing-and-binary-requests.md) and the
[4K preparation benchmark](pixel-ipc-benchmark.md).

Encoded reads are limited to 512 MiB; display pixels to 256 MiB (roughly 67 million
pixels). The formats decoder has its own limits. Source data, decoded pixels,
transport buffers and canvases can coexist: these are not total memory limits.
Frontend plugin commands receive no general filesystem or clipboard-text
permissions. Purpose-built output commands perform native dialog-selected writes.

The browser-only development preview uses a file input and browser-supported codecs
and clipboard APIs. It is useful for layout testing but does not replace desktop
format/clipboard testing. Native saving and drag-out require the desktop app;
imported annotation selection is later integration work.

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

## File drops (D6a)

Drop one image anywhere on the window to open it. Native OS paths enter the same
bounded decoder as Open; JavaScript cannot supply a filesystem path. A failed or
multiple-file drop leaves the current image and annotations intact. Browser-only
preview supports one dropped Blob. Native drop delivery still needs desktop smoke tests.

## Raster canvas transforms (M2-7 milestone)

The Canvas menu provides crop, resize, transparent expansion, quarter-turn rotations
and flips through the shared core implementation. Dimensions use source pixels,
independent of zoom. Transforms fit the new raster and never write files by themselves.

Transforms are disabled while the editor contains new annotations or the imported
image has editable metadata, so this milestone cannot silently discard annotations.
Document-wide transforms and undo history remain pending; see [canvas-transforms.md](canvas-transforms.md).

## Privacy session replacement

`shell_privacy_image` sanitizes the background while preserving annotation elements.
It returns a u32 little-endian session revision followed by a SCIM image packet.
`shell_restore_privacy` switches native background snapshots for session undo/redo.
Only the active session's resources enter output; old snapshots are not serialized.
Both commands allocate their response before changing the current native session.
See [editor-tools.md](editor-tools.md) and decision 0017 for limits and behavior.

## Capture (M4-5)

The Capture button (top left of the title bar) captures a region. Its menu offers
**Window**, **Full screen**, **Previous region** and **Timer** (3, 5 or 10 s), and
**Settings**. Design: `docs/superpowers/specs/2026-11-24-m4-5-capture-design.md`;
decision: [0016](decisions/0016-in-app-capture.md).

- **Region / Window (macOS, Windows, X11).** The editor window hides, every display is
  captured, and one full-screen overlay per display shows the frozen picture. Drag to
  select a region, or hover and click a window; the result is cropped from the frozen
  pixels. Escape or a right click cancels and restores the editor. If an overlay page
  does not report ready within 10 s the capture is cancelled.
- **Full screen** captures the display under the pointer without an overlay.
  **Previous region** re-captures the last region while that display keeps the same
  ID, bounds and scale; otherwise it opens the Region overlay. It is disabled until a
  region has been captured in this session.
- **Timer.** The editor hides and a small countdown badge shows in a corner of the
  display under the pointer. The freeze happens when it ends, so menus opened
  meanwhile are captured. Escape or a click on the badge cancels.
- **Wayland.** Region and Full screen send one portal request; the system dialog
  chooses what to capture, and the user crops in the editor. Window and Previous
  region are disabled. Timer waits, then sends the request.
- **Capture folder and auto-save.** Captures use one remembered folder, asked for the
  first time it is needed and again if it disappears or is not writable. Settings
  (`settings.json` in the app config folder) hold the folder and *Auto-save captures*
  (off by default). Off: the first Save writes `<folder>/Screenshot <date time>.png`
  (editable PNG) without a dialog; later Saves overwrite it; Save As starts in the
  folder. On: each capture is written on arrival. Existing files are never overwritten
  by a new capture (milliseconds are added to the name).
- **Unsaved work.** Capture, Open, Paste and Drop first ask **Save… / Discard / Cancel**
  when the image has unsaved changes. The prompt is modal. An untouched capture is
  replaced without asking, even with auto-save off.
- **Errors.** A permission error (macOS Screen Recording, or a cancelled portal)
  shows text naming System Settings > Privacy & Security > Screen & System Audio
  Recording; there is no link yet. A folder that does not support new-file creation
  by hard link (some exFAT or network shares) shows the save error.

### Manual desktop checklist (not yet run)

- [ ] macOS: permission prompt on first capture; Retina and non-Retina displays side
  by side, selection lands on the right pixels.
- [ ] Windows: mixed-DPI displays; overlay covers each display exactly.
- [ ] X11 without a compositor: overlays appear and cover the displays.
- [ ] Wayland: portal dialog, approval and cancellation; crop in the editor.
- [ ] Timer with a menu opened during the countdown: menu is in the picture, badge is not.
- [ ] Unplug a monitor, then Previous region: falls back to the Region overlay.
- [ ] Read-only capture folder: asks for another folder; no loop.
- [ ] Auto-save on and off: files and names as above, no overwrites; Alt+F4 on an
  overlay restores the editor.
