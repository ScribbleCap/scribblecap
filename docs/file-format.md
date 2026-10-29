# ScribbleCap document format (`scribblecap/1`)

_Status: version 1, 2026-10-29. Implemented in `crates/core` (`Document::from_json`,
`Document::to_json`)._

The editable document is one JSON object. Container formats (PNG, JPEG, WebP, later
HEIF/AVIF and PDF) store it next to the flattened image, together with the binary
resources it lists ([decision 0003](decisions/0003-save-format.md)). This page
describes the JSON only; how each container embeds it is specified in roadmap M1-8.

This is ScribbleCap's own format. It is **not** Skitch's `index.json`; reading Skitch
files is a separate, later task.

## Conventions

| Thing | Encoding |
|---|---|
| Coordinates, sizes, widths | Numbers in **image pixels** of the canvas, origin top-left, y down. Fractions allowed. Never points, never screen pixels. |
| Point | `[x, y]` |
| Size | `{"width": w, "height": h}` |
| Rect | `{"x": left, "y": top, "width": w, "height": h}` (width and height ≥ 0) |
| Colour | `"#RRGGBBAA"`, sRGB, straight alpha, written upper-case. `"#RRGGBB"` is accepted on read (alpha `FF`). |
| Angle | Degrees; 0 points right, 90 points down (clockwise on screen). |
| Keys | `camelCase`. Optional keys are left out when they hold their default. |
| Ids | Non-empty strings, unique within their list. The app writes UUIDs. |

All numbers must be finite. Widths, diameters and font sizes must be greater than 0.

## Versioning

- The top-level key `"format"` must be `"scribblecap/1"`.
- A document without it is not a ScribbleCap document.
- `"scribblecap/<n>"` with any other `<n>` is rejected with a "newer format" error
  (never guessed at). A breaking change bumps `<n>`.
- Readers **ignore unknown keys**, so a later version may add optional keys without a
  bump. Adding an element `kind` is a breaking change (old readers can't draw it).

## Document

| Key | Type | Req. | Meaning |
|---|---|---|---|
| `format` | string | ✓ | `"scribblecap/1"` |
| `canvas` | Size | ✓ | Canvas size; the flat export is exactly this size. Both > 0. |
| `background` | object | | The un-annotated image under the annotations. Absent = blank canvas. |
| `background.resource` | string | ✓ | Id of an entry in `resources`. |
| `background.origin` | Point | | Where the image's top-left sits on the canvas. Default `[0, 0]`; non-zero after "expand canvas". |
| `resources` | array | | Binary resources stored by the container: `{"id", "mediaType"}` (`mediaType` is a MIME type such as `image/png`). Default empty. |
| `elements` | array | | Annotations, painted in order (first = bottom-most). Default empty. |

Pixelate and redact are **not** elements: they are burned into the background resource
when applied, so the hidden pixels are not recoverable from the file.

## Elements

Every element has `id` (string) and `kind` (one of the names below), plus the keys of
its kind.

### `arrow`

Filled, tapered arrow. The outline is computed from these keys by
`geometry::arrow_outline` in `crates/core` (findings §5.3); arrows shorter than
5 × `width` are drawn with a proportionally smaller size.

| Key | Type | Req. | Meaning |
|---|---|---|---|
| `start` | Point | ✓ | Tail. |
| `end` | Point | ✓ | Tip. |
| `color` | Colour | ✓ | Fill colour. |
| `width` | number | ✓ | Arrow size (head and shaft scale with it). |
| `headAtStart` | bool | | Also draw a head at `start`. Default `false`. |

### `line`

| Key | Type | Req. | Meaning |
|---|---|---|---|
| `start`, `end` | Point | ✓ | End points. |
| `color` | Colour | ✓ | Stroke colour. |
| `width` | number | ✓ | Stroke width. |

### `rectangle`

| Key | Type | Req. | Meaning |
|---|---|---|---|
| `rect` | Rect | ✓ | Bounds (the stroke is centred on them). |
| `stroke` | Colour | ✓ | Outline colour. |
| `fill` | Colour | | Fill colour. Absent = not filled. |
| `width` | number | ✓ | Stroke width. |
| `cornerRadius` | number | | ≥ 0. Default 0; > 0 makes a rounded rectangle. |

### `ellipse`

Ellipse inscribed in `rect`. Keys: `rect`, `stroke`, `fill`, `width` as for
`rectangle`.

### `freehand`

Marker or highlighter stroke.

| Key | Type | Req. | Meaning |
|---|---|---|---|
| `tool` | string | ✓ | `"marker"` or `"highlighter"`. |
| `color` | Colour | ✓ | Stroke colour, drawn as stored (the app gives highlighters a translucent colour). |
| `width` | number | ✓ | Stroke width. |
| `points` | Point[] | ✓ | Input points after filtering, at least one. The smooth curve is computed from them when drawing (findings §5.4, roadmap M1-3); it is not stored. |

### `text`

| Key | Type | Req. | Meaning |
|---|---|---|---|
| `origin` | Point | ✓ | Top-left of the text box. |
| `text` | string | ✓ | The text; `\n` separates lines. |
| `style` | string | | `"plain"` (default, outlined text), `"bubble"` (speech bubble) or `"label"` (attached to another element). |
| `color` | Colour | ✓ | Text colour. |
| `fontSize` | number | ✓ | Font size. |
| `font` | string | | Font family. Absent = the app's default sans-serif. |
| `wrapWidth` | number | | Wrap lines at this width. Absent = no wrapping. |
| `align` | string | | `"left"` (default), `"center"` or `"right"`. |
| `attachedTo` | string | | Id of the element this label belongs to. Only with `style: "label"`; must name another element that is not text. |

### `stamp`

Round badge from the app's stamp set.

| Key | Type | Req. | Meaning |
|---|---|---|---|
| `center` | Point | ✓ | Centre of the badge. |
| `stamp` | string | ✓ | Stamp name, e.g. `"approved"`, `"rejected"`, `"question"`. Unknown names draw a generic stamp so newer stamp sets don't break old readers. |
| `diameter` | number | ✓ | Badge diameter. |
| `tailAngle` | number | | Direction of the pointer tail. Absent = no tail. |

### `step`

Numbered circle for step-by-step instructions.

| Key | Type | Req. | Meaning |
|---|---|---|---|
| `center` | Point | ✓ | Centre. |
| `number` | integer | ✓ | ≥ 1. The editor renumbers; the file stores the number shown. |
| `color` | Colour | ✓ | Circle colour (the digit is drawn in a contrasting colour). |
| `diameter` | number | ✓ | Circle diameter. |

## Example

The test suite parses this example, so it stays valid.

```json
{
  "format": "scribblecap/1",
  "canvas": { "width": 1280, "height": 800 },
  "background": { "resource": "6f1c0e2a-3b0d-4c5e-9a57-2d8f4b1e7c90" },
  "resources": [
    { "id": "6f1c0e2a-3b0d-4c5e-9a57-2d8f4b1e7c90", "mediaType": "image/png" }
  ],
  "elements": [
    {
      "id": "a1", "kind": "rectangle",
      "rect": { "x": 100, "y": 120, "width": 400, "height": 160 },
      "stroke": "#FF0000FF", "width": 6, "cornerRadius": 20
    },
    {
      "id": "a2", "kind": "arrow",
      "start": [700, 500], "end": [520, 260], "color": "#FF0000FF", "width": 12
    },
    {
      "id": "a3", "kind": "text", "style": "label", "attachedTo": "a1",
      "origin": [110, 290], "text": "Click here first", "color": "#FFFFFFFF",
      "fontSize": 24
    },
    {
      "id": "a4", "kind": "freehand", "tool": "highlighter", "color": "#FFF70066",
      "width": 24, "points": [[80, 600], [140, 604.5], [260, 602]]
    },
    {
      "id": "a5", "kind": "step",
      "center": [90, 110], "number": 1, "color": "#1977FFFF", "diameter": 36
    }
  ]
}
```
