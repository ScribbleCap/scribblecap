# Drawing, privacy, styles and keyboard access

## Marker and highlighter (M2-3)

M selects marker; H selects highlighter. Pointer samples accumulate in image
coordinates, with a maximum of 8192 per stroke. On completion WASM runs the core's
point filter; shared curve geometry paints both the live editor and exports.
Highlighter uses the selected RGB with alpha 0x66 and four times the base width.
Marker uses opaque RGB and the base width. Each stroke is one undo step. Cancelled
pointer gestures discard the new stroke or restore the shape being moved/resized.
Strokes can be selected and moved; resize them with the numeric annotation controls.

## Destructive privacy (M2-8)

P selects Pixelate, X selects Redact; drag a rectangular background region to apply.
Privacy also offers numeric bounds for keyboard users. Drag regions round outwards
and clip to the canvas; numeric regions must fit entirely.

Redact writes opaque black. Pixelate averages clipped 12-pixel blocks and makes
results opaque; it is not a substitute for redaction of sensitive information.
Annotations stay editable above the changed background. Imported documents keep
their vector elements while their background is sanitized and unused resources
are removed. Sensitive text in annotation elements is not redacted by this operation.

Privacy edits and annotation edits share session undo/redo. Native snapshots retain
previous backgrounds in memory, with a 512 MiB pixel/encoded-resource history budget;
an operation exceeding it fails without changing the current state. Saves, exports,
copy and drag contain only the active background, never older snapshots. Undoing
redaction restores original pixels as the current image: redo the privacy edit
before saving or sharing the cleaned version. A visible, screen-reader-announced
status note after undoing a background edit explains this. Starting a new document
clears history. The history-limit error says to save the current image and reopen
the saved file to continue privacy editing.
Old disk files, clipboard images and previous exports are unchanged.

See decision 0017, which supersedes the flatten-and-clear policy in decision 0015.
Privacy requires the native desktop shell, as do save and clipboard output.

## Colour, size and native menus (M2-5)

Style offers eight presets, a custom RGB colour and base widths 2, 4, 6, 10 and
16 px. `[` / `]` decrease/increase the width. Arrow width is twice the base;
highlighter width is four times it; stamp/step diameter is eight times it.
Changing a style updates the selected annotation with an undo step and sets the
style for new annotations. Stamps have fixed artwork colours and accept size edits.

Every tool has a unique key: V select, A arrow, L line, R rectangle, U rounded
rectangle, O ellipse, M marker, H highlighter, P pixelate, X redact, S stamp, N step.
Native File, Edit, View and Window menus expose file actions, undo/redo and zoom;
macOS also has its application menu. Menu accelerators use the platform modifier.
Text inputs retain native text editing rather than triggering canvas operations.

## Keyboard and screen-reader access (M5-3)

The tool toolbar uses one Tab stop and Left/Right/Home/End navigation. Pixelate and
Redact are tools in the same row. The window is at least 800 × 560 px; below 960 px
the shape tools fold into one Shapes menu, and below 720 px (for example with large
text zoom) a labelled native Tool select replaces the icon row. Canvas, Annotations
and the numeric Privacy controls are under the ⋯ More menu. Controls have visible
focus outlines, accessible names and pressed states. Small windows reflow controls
instead of clipping them; popovers scroll within the viewport.

Annotations exposes an ordered list of named annotations, keyboard selection and
numeric creation/resizing. Choosing an item returns focus to the viewport, where
arrow keys move it (Shift for ten pixels), Delete removes it and Escape deselects.
Creation accepts start/end coordinates; marker/highlighter create a two-point stroke.
Stamp/step placement uses the end point. Circular annotations resize by width.
Privacy's numeric form provides the same operation without pointer dragging.
Escape in any popover closes it and returns focus to its summary. Space/Enter on
buttons keep normal activation. Live regions announce edits, selection and counts;
errors use alerts. Existing imported annotations remain flattened in their preview
and cannot be individually selected (an existing editor limitation).

Automated tests exercise these routes against the real WASM core. Manual validation
still required on native desktops: VoiceOver/NVDA/Orca announcements, native menu
accelerators in both text inputs and the viewport, OS colour dialogs, high contrast,
200% UI scaling, and Tab traversal without a pointer. This is not a WCAG certification.

## Original stamps and steps (M2-10)

The Signal set (check, alert, spark) is newly authored vector geometry in
`crates/core/src/geometry/primitives.rs`. No Skitch images, paths, fonts or other
artwork were consulted or copied. The symbols, frames, proportions and colours are
specified directly in code and covered by the `signal_stamps` rendering golden. See the
[artwork provenance record](../assets/README.md) for sources, licensing and review limits.
The product palette exposes only this new set; older stamp names remain readable
for format compatibility.

S chooses stamps, N chooses numbered steps. Click to place at the pointer, or drag
and release to place at the end point. Stamp artwork selects one of the three Signal
variants. Steps start at one and renumber in document order after deletion; undo
restores the previous numbers. Move them with Select or keyboard nudging; resize by
the circular handle or numeric width. Shared core paths, including bundled step
number glyphs, are used in both preview and saved output.

## Editor feedback fixes (macOS follow-up)

Privacy drag previews project image bounds into screen coordinates after the image
border is painted, with a two-screen-pixel outline. The mode hint is one line:
“Drag over the area to redact/pixelate. Esc to stop”. Escape exits the mode and
cancels an active privacy drag. Detailed privacy guidance remains in More → Privacy.

Transient output messages stay in the polite live region for four seconds. A new
message restarts that timer; errors still require manual dismissal. Title-bar
details menus close on outside pointer-down, Escape or opening another top-level
menu. Preset/committed custom colour, size and Share choices close their menu;
interacting with custom colour or numeric fields does not dismiss it prematurely.
When a native dialog or ARIA modal is open, its Escape handling takes precedence
over title-bar menus, preserving the dialog's dismissal and focus behavior.

Manual macOS re-check pending: run the native app, open an image, select Stamp and
inspect the title bar at 1100 px and the 800 px minimum. Confirm icons stay on one
row, no Tool select/Open/Paste buttons appear, and More opens within the window.
Also check a zoomed/panned privacy preview, Escape cancellation, colour/size and
Share dismissal, outside clicks, and message expiry. The native dev app launched
during development; computer-use's native pipe was unavailable for visual inspection.
