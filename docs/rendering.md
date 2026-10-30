# Flat rendering (M1-4a)

`scribblecap_core::render::render` paints a validated document into straight RGBA8
pixels. It handles arrows, lines, rectangles/rounded rectangles, ellipses,
marker/highlighter and original vector stamps. It does not decode files, write
files or use operating-system APIs. See [decision 0007](decisions/0007-flat-rendering.md).

## API and pixel interchange

```rust
use scribblecap_core::{Document, Size};
use scribblecap_core::render::render;

let document = Document::new(Size::new(640.0, 480.0));
let image = render(&document, None).unwrap();
assert_eq!((image.width(), image.height()), (640, 480));
// image.pixels() borrows row-major straight RGBA8; into_pixels() takes ownership.
```

For a document with a background, decode its resource outside core and construct
`BackgroundImage::new(&resource_id, width, height, rgba_bytes)`. Pass that view as
the second argument. Its resource id must match `document.background.resource`;
missing pixels are an error. The image is drawn at `background.origin`, at native
pixel size with nearest sampling, including fractional or negative offsets.

The canvas starts transparent. Extents are rounded up to whole pixels; rendering
is clipped at those bounds. Background and canvas buffers are each limited to
256 MiB. This is not a total-process memory bound: decoded input, temporary paths
and both pixel buffers may coexist. f64 document coordinates must be representable
by the painter's f32 geometry; unrepresentable paths return an error.

tiny-skia stores premultiplied RGBA internally. `RenderedImage` always returns
straight alpha for file encoders, matching the M1-5 `RasterImage` contract. Pass
`width()`, `height()` and `into_pixels()` into that type; no reverse dependency
from core to formats is needed. Low-alpha colors can round through premultiplication;
fully transparent RGB is cleared. Source metadata and editable payloads are the
responsibility of formats, not the painter.

## Geometry and appearance

`geometry::shape_paths(&shape)` returns a list of `PaintedPath { path, style }`.
These are plain core paths suitable for both Canvas and export. Fill operations
use the nonzero rule. Strokes are centered; `round: true` requests round caps and
joins, otherwise butt caps/miter joins with limit 4. All operations use source-over.

Shape fill is painted before its boundary. Alpha is applied to each operation,
so translucent fill/stroke intersections and distinct overlapping annotations can
darken. A freehand stroke is painted in one operation, so its own crossings do not
accumulate alpha. Highlighter width/alpha come directly from the stored element;
the renderer does not apply a second opacity or size multiplier.

Stamps have original green check, red cross, blue question, orange exclamation
and purple star badges. Unknown names produce a gray diamond badge. Their optional
tail points in `tailAngle` degrees (0 right, 90 down), and may extend beyond the
circle diameter. Icons and tails are paths, with no font or external artwork.

Text of every style and step numbers return `UnsupportedElement` with the element
id. M1-4b will add their shared layout/glyph paths after the font decision. The
renderer never returns a successful image with those annotations silently removed.

## Golden tests

`crates/core/tests/golden/render/` contains five PNG fixtures: arrows, shapes,
freehand, stamps and background/alpha compositing. Tests decode them and compare
every straight RGBA byte, with no tolerance. These PNGs are original synthetic
scenes and are safe to commit.

After an intentional rendering change:

```sh
SCRIBBLECAP_UPDATE_GOLDEN=1 cargo test -p scribblecap-core --test flat_renderer
cargo test -p scribblecap-core --locked
```

Review every regenerated PNG before committing it. CI does not set the update
variable, so it can only compare against committed fixtures. tiny-skia is pinned,
SIMD is disabled and vector painting uses its high-quality pipeline. All three OS
checks must pass to establish that those fixtures are pixel-identical there.
