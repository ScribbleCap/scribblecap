# Editor (M2-2)

The editor draws annotations on the open image. Shapes are edited through the core
library running as WebAssembly ([decision 0011](decisions/0011-core-in-the-ui.md));
the UI never computes painted geometry itself.

## Layout

The layout from the [design review](design-review.md):

- **Title bar:** Open and Paste on the left (Capture replaces them in M4-5), tools on
  the right. On macOS the title bar is an overlay beside the window buttons and can
  be dragged to move the window. Windows and Linux keep their native title bar above
  it for now.
- **Status bar:** file name and size on the left, Fit and zoom on the right.

## Tools

| Tool | Key | Drag does |
|---|---|---|
| Select | V | On a shape: move it. On empty space: pan. |
| Arrow | A | Draw an arrow from the press point to the tip. This is the starting tool. |
| Line | L | Draw a line. |
| Rectangle | R | Draw a rectangle (any drag direction). |
| Rounded rectangle | U | Draw a rectangle with rounded corners. |
| Ellipse | O | Draw an ellipse inside the dragged rectangle. |

- A click without a drag (under 3 screen pixels) draws nothing.
- A newly drawn shape is selected. Its handles are in the system accent colour:
  two endpoints for arrows and lines, eight for boxes. Dragging a handle resizes the
  shape with any tool; boxes flip cleanly when dragged past the opposite edge.
- **Delete** or **Backspace** removes the selected shape; **Escape** deselects.
- **Arrow keys** move the selected shape by 1 pixel (10 with Shift); with nothing
  selected they pan.
- **Pan:** hold Space and drag, drag with the middle button, or scroll.
  ⌘/Ctrl + scroll zooms.
- Cursors follow the tool: crosshair for drawing, move over a shape with Select,
  grab/grabbing while panning.
- Colour and size are fixed defaults until M2-5 (pink `#e5194a`, 6 px outlines and
  12 px arrows). Undo is M2-6.

Hit testing and handle dragging work on shape parameters (endpoints and rectangles)
in `ui/src/lib/shapes.ts`. Painted outlines, arrowheads and rounded corners come only
from the core.

## Core protocol

`ui/src/lib/core.ts` wraps the module. Each call sends one JSON request and receives
`{"ok": …}` or `{"error": "…"}`; a request that would make the document invalid
changes nothing.

| Request | Response `ok` |
|---|---|
| `{"op":"reset","canvas":[w,h]}` | `null`; starts an empty document |
| `{"op":"add","shape":{…}}` | `{"id","paths"}`; the shape goes on top |
| `{"op":"update","id","shape":{…}}` | `{"paths"}`; keeps id and stacking order |
| `{"op":"remove","id"}` | `null` |
| `{"op":"paint"}` | `[{"id","paths"}]`, bottom-most first |
| `{"op":"document"}` | the document JSON ([file-format.md](file-format.md)) |

Shapes use the element JSON from `file-format.md` without `id`. Ids are `el-1`,
`el-2`, … and are not reused within a document. A path is
`{"d": SVG path data, "fill": colour}` or
`{"d", "stroke": colour, "width", "round"}`, in image pixels; `round` selects round
caps and joins, otherwise butt caps and miter joins (limit 4), matching the flat
renderer.

## Validation

`pnpm --dir ui test` builds the module and runs the UI tests against it: tool
switching, drawing, ignoring a click, selecting, moving, resizing, nudging, deleting
and Space-panning. `cargo test -p scribblecap-core-wasm` covers the protocol
natively. Manual check: run `pnpm --dir ui tauri dev`, open an image and draw each
shape.
