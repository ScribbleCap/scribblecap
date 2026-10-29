# Skitch 2.9 — reverse-engineering findings

Target: `Skitch.app` 2.9 (build 265153), `com.skitch.skitch`, Mach-O x86_64 only,
built with Xcode 11.3 against macOS 10.15 SDK, minimum macOS 10.11.
Main binary SHA-256 `9a7160c71cd9cf23eb2b0cb23e05d79e71562c8895caad841b2b79448b28f7f0`.

Evidence types used:
- **observed (metadata)** — Objective-C runtime metadata (`otool -oV`, `nm`), Info.plist,
  bundled resources. Exact, but only names/types, not behaviour.
- **observed (decompiled)** — Hopper pseudocode via REA; IDs are `ev_…`. Static only;
  nothing here was observed executing.
- **inferred** — conclusions drawn from names + decompiled code. Marked as such.

Clean-room note: this ledger records *behaviour and formats* for interoperability.
The open-source rewrite should not copy Evernote's code, artwork (`Resources/*.png`,
`Assets.car`), sounds, or the "Skitch" trademark.

---

## 1. App structure (observed, metadata)

| Component | What it is |
|---|---|
| `MacOS/Skitch` | Main editor app (`SkitchApplication`, `AppDelegate_Mac`) |
| `Library/LoginItems/…SkitchHelper.app` | `LSUIElement` menu-bar helper: `NSStatusBar`, global hotkeys via Carbon `RegisterEventHotKey`, owns crosshair capture UI (`ScreensnapManager`, `ImageCaptureCrosshair*`), timed snap countdown, talks to main app over XPC (`xpc_screen_snap_message`) |
| `PlugIns/SkitchMacAction.appex` | `com.apple.ui-services` action extension ("Markup with Skitch" from share sheets) |
| `PlugIns/SkitchMacPhotoEditing.appex` | `com.apple.photo-editing` extension for Photos.app |
| `Frameworks/EDAM.framework` | Evernote Thrift API client (sync — out of scope for a rewrite) |
| `Frameworks/ENAttachmentToPDF`, `CoreNoteStrings`, `HockeySDK`, `Log4Cocoa` | Evernote helpers, crash reporting, logging |

Document types: opens `public.image`, `com.adobe.pdf`, legacy `.skitch` (Skitch 1.x);
exports PNG/JPEG/TIFF. URL scheme `skitch://`. Pasteboard type
`com.evernote.skitch.mac.pasteinfo` (plist).

Default prefs (`Resources/UserDefaults.plist`): `TimedScreenSnapDelaySeconds = 5`,
`ExpandableCanvas = true`, `TypeToAutoCreateTextNodes = true`, `exportRetina = true`,
`arrow_at_start = false`, drag-out formats PNG / JPEG(compressed) / PDF.

## 2. Feature inventory (observed, strings + class names + tool artwork)

Tools (`SkitchToolsModel` main/secondary tools; `tool_*` images; TouchBar assets):
Arrow, Text, Rectangle, Rounded Rectangle, Ellipse, Line, Marker (freehand),
Highlighter, Stamp (5 colours: blue/green/orange/pink/red; labels Approved, Rejected,
Perfect, Question, Exclaim/Attention + "funny" emoticon stamps: Happy, Angry, Cool,
Silly, Surprise, Hehe, Grrr, Wow!, What?, Kidding!, Fantastic!, Uh oh…), Pixelate,
Crop, Resize, Pan. Sub-tools can be pinned open.

Palette (`+[NSColor skitch*]`): Black, White, Yellow, Orange, Green, Blue, Pink, Red.
Five stroke sizes (`TouchBar_Size_01…05`).

Canvas operations: Expand Canvas (click edge to grow), Resize Canvas, Rotate Canvas,
Flip, Crop, Nudge, multi-page PDF (rotate/delete page, thumbnails, "Annotated only"
filter, PDF with annotation summary page).

Capture (`ScreensnapManager`, `ImageCaptureManager`): crosshair area, previous area,
timed crosshair (countdown + `pre-snap-countdown.m4a`, `snap.m4a`), full screen,
window (click-to-select window overlay), menu (with/without all menus), camera
(`AVFoundation`/`QTKit`). Window enumeration via `CGWindowListCopyWindowInfo`, pixels
via `CGWindowListCreateImage` (deprecated — rewrite must use ScreenCaptureKit).

Output: drag-out "drag me" tab (`DragMeDangleWindow`), copy, save as Skitch PNG / JPG /
PDF / PDF-with-summary, email PDF, Evernote sync/share.

Labels: text can be attached to a shape/stamp (`attachLabelToNode:`, `labelGUID`),
and follows it.

## 3. File format: annotated PNG/JPG/HEIC (observed, decompiled)

A "Skitch PNG" is a normal PNG the user can open anywhere, plus private chunks
carrying the editable document. Evidence:
`ev_62e5a448…` (`-[SkitchCompositionPNGCodec encodeBundle:imagePath:error:]`),
`ev_da9aa2a2…` (write callback), `ev_e07a4bcc…` (metadata chunk),
`ev_d47c092b…` (resource chunk), `ev_6ccbdd6c…` (`SkitchPNGWriteChunks`),
`ev_5f8b45d3…` (read callback), `ev_7b8f9608…` (decode).

Writing (`SkitchPNGWriteChunks`):
1. Write the 8-byte PNG signature.
2. Copy every chunk of the flattened preview PNG except `IEND`.
3. Append one **`skMf`** chunk: payload = raw bytes of `index.json`.
4. Append one **`skRf`** chunk per resource: payload = 16 raw UUID bytes
   followed by the resource file bytes. Resource URI is `skitch+uuid:///<UUID>`.
5. Write `IEND`.
Chunk length and CRC32 (over type + payload) follow the PNG spec. Chunk types are
lowercase-first (ancillary, private), so other readers ignore them.

**JPEG** (`SkitchJPGWriteChunks`, observed, decompiled): copy the JPEG unchanged, then
*append after the JPEG end-of-image marker*: the same chunk stream (`skMf`, `skRf`…,
PNG-style length/type/payload/CRC), then a 4-byte **big-endian** length of that chunk
stream, then the 6 ASCII bytes `skitch`. Reader (`SkitchJPGImageIsValidSkitchJPGFile`):
check the file is JPEG, read the last 6 bytes == `skitch`, read the uint32 BE at
EOF−10, seek to EOF−10−len and parse chunks. Ordinary JPEG decoders stop at EOI, so the
trailer is invisible to them.

**HEIC** (`SkitchHEICWriteChunks`, observed, decompiled): append an ISO-BMFF **`uuid`
box** at end of file. The 24-byte header template `_SkitchHEICImageFileBox` is
`00000000 'uuid' 5bb24393-8969-4d17-a78c-5c95b25b482e`; the chunk stream follows, then
the writer seeks back and patches the first 4 bytes with the big-endian box size
(header + payload). HEIF readers skip unknown `uuid` boxes.

Reading tries JPG, then HEIC, then PNG readers; `skMf` → `index.json`,
`skRf` → resource keyed by UUID.

On-disk bundle form (`SkitchCompositionBundle`): `document.skitch/` with
`index.json`, `preview.png`, `Contents/Resources/<uuid>`.

PDF: `SkitchPDF writeModifiedPDFToURL:withAppendedObjects:domJSON:error:` (observed,
decompiled names; layout partly inferred) — the original PDF bytes are kept and an
**incremental update** is appended: new objects (annotation appearance streams and the
DOM), a new xref section whose trailer has `/Prev` → the previous xref offset (error
"Could not find offset to previous xref" if missing) and `/Root` → the catalog, which
gets a private key **`/XXSkitch`** pointing to a dictionary
`<< /Type /SkitchInfo /OriginalPdfSize <bytes before the update> /SkitchDom <ref> >>`
(the `SkitchDom` stream holds the `index.json`-style DOM). `OriginalPdfSize` lets Skitch
truncate back to the untouched original when re-saving. See §6 for details.

## 4. Document model `index.json` (observed, decompiled `jsonType` / `dictionaryRepresentation`)

Every node writes `type` = its class's `jsonType`. Concrete vector shapes **don't** get
their own `type`; they are written as a generic `Vector` (path + colours, renderable by
any reader) with the shape-specific parameters in an **`extension`** dictionary whose
own `type` names the shape. `jsonTypePath` (e.g. `Vector.Arrow`) is the internal
dispatch path built as superclass path + `.Name`.

| Class | `type` | Keys written (in addition to inherited ones) |
|---|---|---|
| SkitchDOMNode (abstract) | — | `type`, `guid`, `transform`, `children` (omitted if empty; *adjustment* nodes such as pixelation are skipped), `metadata` (omitted if empty), `labelGuid` |
| SkitchDOMMultiPageDocument | `MultiPageDocument` | `children` (one `Document` per page) |
| SkitchDOMDocument | `Document` | `frame`, `title`, `pageNumber`, `deviceScale`, `contentScaleFactor` |
| SkitchDOMLayer | `Layer` | `layerId` — background, annotation, text layers |
| SkitchDOMElement | `Element` | `style` |
| SkitchDOMBitmap | `Bitmap` | `frame`, `cropRect`, `uri` (`skitch+uuid:///<UUID>`), `contentMode`, `imageProfile` |
| SkitchDOMVector | `Vector` | `path` (SVG path string), `strokeColor`, `fillColor`, `lineWidth` |
| SkitchDOMArrow | `Vector` | `extension: {type:"Arrow", startPoint, endPoint, toolArrowSize}` |
| SkitchDOMEllipse | `Vector` | `extension: {type:"Ellipse", center, xradius, yradius, theta}` |
| SkitchDOMRectangle | `Vector` | `extension: {type:"Rectangle", cornerRadius}` |
| SkitchDOMPolygon | `Vector` | `extension: {type:"Polygon", vertices}` |
| SkitchDOMLine | `Vector` | `extension: {type:"Line", startPoint, endPoint}` |
| SkitchDOMText | `Text` | `text`, `origin`, `wrapWidth`, `font`, `strokeColor`, `fillColor`, `alignment`, `direction`, `textStyle` |
| SkitchDOMStamp | `Stamp` | `frame`, `stampName`, `tailAngleInDegrees` |
| SkitchDOMColor | — | `{red, green, blue, alpha}` |
| SkitchDOMFont | — | `{name, size}` |
| SkitchDOMStyle | — | `shadowColor`, `shadowOffset`, `shadowOpacity`, `shadowRadius` |

Value encodings (observed, `NSDictionaryFromCG*` helpers):
- point → `{"x":…, "y":…}`; size → `{"width":…, "height":…}`;
  rect → `{"x", "y", "width", "height"}`;
  affine transform → `{"a","b","c","d","e","f"}` (= CGAffineTransform a,b,c,d,tx,ty).
- Text `alignment`: `left` `center` `right`. `direction`: `leftToRight`
  `rightToLeft` `topDown` `bottomUp`. `textStyle`: `paragraphText` `bubbleText`
  `labelText`.

Older-format keys still accepted on read (observed, `initWithDictionary:`): colours and
width under `inkStyle.strokeColor` / `inkStyle.fillColor` / `inkStyle.lineWidth`; line
endpoints in node `metadata` under `com.evernote.skitch.element.semantics.line.start` /
`.line.end`; arrow size under `com.evernote.skitch.element.semantics.arrow.toolArrowSize`;
label parent as `labelParent`. A reader should try the new key first, then the old one.

Debug aid (observed): in the document view, ⌘⌥ + key with user default
`DOMDumpingEnabled` dumps the current DOM JSON to the log and the pasteboard.

## 5. Drawing behaviour and constants (observed, decompiled with Ghidra; constants read from `__const`)

### 5.1 Palette (`+[NSColor(SkitchColorConvenienceMethods) skitch*]`)

`skitchColorFromRGB:alphaPercentage:` → sRGB-calibrated `r,g,b = hex bytes / 255`,
`alpha = min(pct,100)/100`.

| Name | Hex |
|---|---|
| Black | `#000000` |
| White | `#FFFFFF` |
| Yellow | `#FFF700` |
| Orange | `#FF8017` |
| Green | `#00E800` |
| Blue | `#1977FF` |
| Pink | `#FC0C59` |
| Red | `#FF0000` |

### 5.2 Tool sizes (`SkitchPropertySize`)

Five size IDs 0–4 ↔ *relative* tool size `0.2, 0.4, 0.6, 0.8, 1.0` (out-of-range → 0.6).
Reverse mapping thresholds: ≤0.2→0, ≤0.4→1, ≤0.6→2, ≤0.8→3, else 4.
Arrow absolute sizes: relative × 20 → **4, 8, 12, 16, 20 pt**
(`convertToolSizeAbsoluteToToolSizeRelative`: `abs / absoluteScale / contentScaleFactor`,
snapped with ±0.01 tolerance). Both absolute and relative sizes are stored so documents
re-scale when `contentScaleFactor` changes.

### 5.3 Arrow outline (`+[SkitchDOMArrow makeArrowPathFromPoint:toPoint:arrowSize:]`)

A single filled outline: concave tapered shaft, triangular head with slightly swept-back
barbs, rounded tail. Built along +x from the origin, then rotated by
`atan2(dy,dx)` (degrees) and translated to the start point.

```
L  = |end − start|
s  = arrowSize;  if L < 5·s then s = L / 5          // short arrows shrink
h  = (s − 1.5)·0.7 + 6
headLen   = min(max(6, 2h), 0.4·L)
tailHalf  t = max(1.5, 0.08·s)                      // half-width at the tail
shaftHalf w = max(t, min(0.5·h, 0.25·headLen))      // half-width where shaft meets head
baseX = L − headLen
barbX = L − 1.1·headLen,  barbY = 0.55·headLen
m1 = (t + w)/2,  m2 = (t + m1)/2

moveTo  (0, −t)
curveTo (baseX, −w)   cp1 (baseX/2, −m2)   cp2 ((L/2 + baseX)/2, −m1)
lineTo  (barbX, −barbY)
lineTo  (L, 0)                                       // tip
lineTo  (barbX, +barbY)
lineTo  (baseX, +w)
curveTo (0, +t)       cp1 ((L/2 + baseX)/2, +m1)   cp2 (baseX/2, +m2)
curveTo (0, −t)       round tail cap: control points pushed 1.5·t behind the start
                      along the unit vector from (baseX, −t) toward (0, −w)… (second
                      control point inferred as the mirror of the first)
closePath
```
Optional second head at the start (`arrow_at_start` default, "arrowheads at beginning
of shape"): `getStandardArrowStart:…` / `enforceMinimumLength:growFromStartPoint:`
(not yet decompiled in detail).

### 5.4 Freehand (Marker / Highlighter)

- **Low-pass filter** (`SkitchLowpassPointFilter`): first point passes through; each
  next point `p' = last + (p − last)·α`, `α = dt / (RC + dt)`. Ivars `dt`, `RC`
  Present in the binary but **not used** for strokes in 2.9 (see Rite filter below).
- **Rite filter** (`SkitchRitePointFilter`, the one `SkitchDOMVector beginAtPoint:`
  installs): works in fixed point (coordinates ×9). For each new input point it
  subdivides the segment from the previous input in thirds with a running second-order
  smoothing (each new value = previous smoothed + (target − previous)/3), converts each of
  the 3 candidates back with `(v + 4) / 9` (integer rounding), and emits only those whose
  Manhattan distance from the last emitted point is ≥ 2 px. The first point is emitted
  as-is. Effect: light smoothing plus de-duplication on an integer grid.
- While drawing, the raw filtered points are joined with straight segments; on mouse-up
  the curve fit runs on a background queue and the smoothed path replaces the polyline
  when done.
- **Curve fitting** (`SkitchPathFitter initWithPoints:nPts:smallTraceSize:`):
  Schneider-style cubic fitting with corner detection. Parameters:
  `kDencityBase = smallTraceSize`, `kCornerDetectionVectorLength = 0.5·smallTraceSize`,
  `kMinCornerAngle = 90°`, `kPrecision = sqrt(smallTraceSize / 5)`, `kIterations = 4`,
  where **`smallTraceSize` = the stroke's `lineWidth`**. Result stored as an SVG path
  string in `path`.
- `lineWidth` is clamped to **1…256**.
- **Highlighter** is not a separate class: it is a `Vector` whose stroke colour starts at
  **40 % alpha** (Marker 100 %). Changing colour keeps the existing alpha.
  `absoluteToolSizeScale` is **2.0 when stroke alpha < 1** (highlighter is twice as
  thick as the marker at the same size step), else 1.0. Readers detect a highlighter by
  stroke alpha < 1.
- Marker strokes drawn in quick succession are grouped as "wet ink" (animated together,
  one undo step).

### 5.5 Text

- Default font size **24 pt**.
- Bubble/outline stroke width = **0.28 × font size** (bubble style uses the default
  size × content scale).
- Outline colour: **white if the text colour is dark, otherwise black**
  (`strokeColorForTextColor:` via `isDarkColor:`).
- Block styles `paragraphText`, `bubbleText`, `labelText` (label = text attached to a
  shape/stamp via `labelGuid`).

### 5.6 Stamps

`stampName` identifiers — standard: `com.evernote.skitch.rejected`, `.exclaim`,
`.question`, `.approved`, `.perfect`; "funny": `.happy`, `.cool`, `.silly`, `.surprise`,
`.angry`. A stamp is a circular badge with a rotatable tail (`tailAngleInDegrees`) and an
optional attached label. A rewrite should use its own identifiers on write and map
these on read.

### 5.7 Pixelate

`SkitchPixelBlurWidgetView loadPixelationImage`: render a copy of the background layer,
then `skitch_pixelatedImageByScaling: 1/6` — vImage high-quality downscale to 1/6 and
back up to full size (a soft pre-blur so blocks don't alias). That intermediate is
cached; then `skitch_pixelatedImageWithBlockSize:` downscales it by `1/blockSize`
(plain vImage scale) and draws it back up without interpolation, giving hard square
blocks.

Block size (observed, `updatePathWithPoint:`, `setImage:`, `setBlockSize:`):
```
m         = min(selection.width, selection.height)        // in image pixels
blockSize = 6 + floor(floor(m / 10) * 10 / 6)
blockSize = clamp(blockSize, 6, maxBlockSize)
maxBlockSize = 16 if max(image.width, image.height) <= 1600 else 28
```
So larger selections get coarser blocks; default block size is 6. Applied through a free-form mask (marching
ants) and baked into the bitmap as an *adjustment* node, which is **not** serialized
— pixelation is destructive in saved files (privacy-correct).

### 5.8 Tool IDs and creation (observed, `SkitchDOMDocumentViewDelegate documentView:didBeginObjectAtPoint:`)

| ID | Tool | Node created | Notes |
|---|---|---|---|
| 0 | Marker | `Vector` (Rite filter + fit) | ⇧ held → straight `Line` instead |
| 1 | Highlighter | `Vector`, stroke alpha 40 % | ⇧ held → straight `Line` |
| 2 | Arrow | `Arrow` | ⌥ held toggles the arrowhead-at-start preference for this arrow. Minimum length **40 pt × contentScale**; a click with no drag places a standard-length arrow |
| 3 | Line | `Line` | |
| 4 | Rectangle | `Rectangle` | |
| 5 | Rounded rectangle | `Rectangle`, `cornerRadius = 20 × contentScale` | |
| 6 | Ellipse | `Ellipse` | |
| 7 | Stamp | `Stamp` (current `stampName`) | in PDFs kept on the page vertically |

After creation every node gets the current tool colour and tool size (relative → absolute
via the class's `absoluteToolSizeScale` and the document `contentScaleFactor`).

Stamp geometry: badge **100 × 100 pt** (× contentScale), inner circle **56 pt**, both
centred on the stamp's start point; the tail points toward `tailAngleInDegrees`.

### 5.9 Keyboard (observed, `keyDown:` handlers)

- Typing / Space (keycode 0x31) with `TypeToAutoCreateTextNodes` → creates a text node.
- Delete (0x33) / Forward-delete (0x75) → delete selection.

## 6. PDF annotation storage (observed, decompiled `writeModifiedPDFToURL:…`)

1. Write the original PDF bytes to the destination; record its size.
2. Pick the next object number N = max(original's next free number, appended objects + 1).
3. Object **N** = stream with the DOM JSON (UTF-8), Flate-compressed.
4. Object **N+1** = `<< /Type /SkitchInfo /OriginalPdfSize <size from step 1> /SkitchDom N 0 R >>`.
5. Re-emit the catalog (`/Root` of the previous trailer) with an added
   `/XXSkitch N+1 0 R`, plus the annotation objects (flattened appearance of the
   drawing on each page).
6. Append those objects with a new xref section and trailer copied from the previous
   trailer (or xref-stream dictionary) with `/Prev` = previous xref offset.

Reading: follow `/Root` → `/XXSkitch` → `/SkitchDom` to get the editable DOM. To re-save,
truncate the file to `/OriginalPdfSize` and append a fresh update, so edits never
accumulate. Other PDF viewers just see an ordinary incremental update.

## 7. Legacy Skitch 1.x `.skitch` import (observed, `SkitchComposition(SVGImport) convertSkitchSVGAtURL:error:`)

A 1.x `.skitch` file is **SVG XML**. Accepted when the extension is `skitch`, the XML
document has exactly two top-level nodes, the first one's text is `Skitch1.0` (probably
a comment or processing instruction; inferred), and the root `<svg>` has
`version="1.1"`. Mapping:
- `<svg width height style>` → canvas size and background colour.
- `<image x y xlink:href="data:image/png;base64,…">` → background bitmap.
- `<rect>` / `<path d=…>` / `<g>` → vector nodes; attributes `fill`, `stroke`,
  `opacity`, custom `skitchHasShadow` (adds the standard drop shadow).
- `<text>` → text node: lines (child elements) joined with `\n`, `font-family`,
  `font-size`, `fill`, position from custom `skitchTextX` / `skitchTextY`; outline
  colour from the fill (same dark/light rule as §5.5).

## 8. Notes for an Apple Silicon rewrite

- Screen capture: `CGWindowListCreateImage` is obsolete → **ScreenCaptureKit**
  (`SCScreenshotManager`), window picking via `SCShareableContent`.
- Global hotkeys: Carbon `RegisterEventHotKey` still works on macOS 14/15; or use a
  small wrapper package.
- Menu-bar helper + login item: `SMAppService` replaces `LoginItems` helper bundles.
- Share extension / Photos editing extension: same extension points still exist.
- Evernote sync (`EDAM.framework`), HockeySDK, Log4Cocoa: drop.
- Pixelation: Core Image `CIPixellate` matches the look closely; vImage is also fine.
- File compatibility: a reader for §3 (PNG `skMf`/`skRf` chunks, JPEG `skitch` trailer,
  HEIC `uuid` box), §4 (JSON incl. old keys), §6 (PDF `/XXSkitch`) and §7 (1.x SVG)
  opens every Skitch file users have; writing the same format keeps round-tripping
  with Skitch 2.9 possible.

## Open questions

- [x] Remaining `dictionaryRepresentation` keys — all done (§4)
- [x] Arrow outline geometry (§5.3)
- [x] Path-fitter constants (§5.4)
- [x] Pixelation block-size formula (§5.7)
- [x] JPEG/HEIC chunk embedding (§3)
- [x] PDF incremental-update object layout (§6)
- [x] Legacy Skitch 1.x import (§7)
- [ ] Exact Bézier internals of `SkitchPathFitter` (standard Schneider algorithm
      assumed; only constants verified)
- [ ] Text bubble shape (rounded-rect path, padding) and drop-shadow default values
- [ ] Stamp badge artwork drawing (rendered from images in the bundle; a rewrite needs
      new artwork anyway)
- [ ] Capture helper details (crosshair magnifier, timed snap UI) — behaviour is clear
      from the UI; not decompiled
- [ ] Nothing here was checked against a real Skitch-saved file. **Next step:** save a
      PNG/JPEG/PDF with Skitch 2.9 (on an Intel Mac or Rosetta) and confirm the parser
      against it.
