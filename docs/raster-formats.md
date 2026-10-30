# Raster input and flat export

Implemented by `scribblecap-formats` (roadmap M1-5). These operations handle encoded
bytes and pixels; they do not render annotations, access the filesystem or embed
editable documents. See [decision 0005](decisions/0005-raster-codecs.md).

## API

```rust
use scribblecap_formats::{RasterFormat, RasterImage, decode, encode};

// In an application, obtain encoded bytes from a file or clipboard.
let pixels = RasterImage::new(1, 1, vec![255, 0, 0, 255])?;
let png = encode(&pixels, RasterFormat::Png)?;
let opened = decode(&png)?;
assert_eq!(opened.format, RasterFormat::Png);
assert_eq!(opened.image, pixels);
# Ok::<(), scribblecap_formats::RasterError>(())
```

`RasterImage::new` rejects zero dimensions, overflowing pixel counts and buffers
whose length differs from `width * height * 4`. Pixels are top-left-origin,
row-major RGBA8 with straight alpha. `pixels()` borrows them and `into_pixels()`
transfers ownership. This type does not depend on core geometry or the renderer.

`decode` identifies the container by signature, returns `DecodedRaster { format,
image }`, and rejects unsupported containers or corrupt input. There is no
extension-based fallback. EXIF orientation is applied to pixels and dimensions
for formats that carry it (PNG, JPEG, WebP and TIFF). An export of those pixels
has no old orientation tag, so reopening does not rotate the image a second time.

## Format behavior

| Container | Open | Flat export |
|---|---|---|
| PNG | Normalize to RGBA8 | Lossless RGBA8 |
| JPEG | Normalize to RGBA8 | Quality 90; transparency composited on white |
| WebP | Normalize to RGBA8 | Lossless RGBA8 |
| TIFF | First image; normalize to RGBA8 | Single-image RGBA8 |
| BMP | Normalize to RGBA8 | RGBA8 |
| GIF | First frame on its logical canvas | One frame, at most 256 colors, binary transparency |

JPEG compression changes pixels. GIF quantization can change colors; fully
transparent pixels remain transparent, while nonzero alpha becomes opaque.
GIF palette transparency cannot preserve hidden RGB values under transparent pixels.

Decoding retains the codec's default resource limits and rejects decoded or
normalized pixel buffers over its default 512 MiB allocation budget before creating
them. Internal codec allocations and temporary conversion buffers mean total
process memory may exceed that figure.

Input bit depths and channel layouts are normalized to RGBA8. No ICC profile
conversion is performed. Metadata, animation, extra TIFF pages and hidden payloads
are not copied into flat output. Callers that want to preserve editability must use
the M1-8 `encode_editable` API described in [editable-containers.md](editable-containers.md),
rather than treating a flat export as an editable save.

## Verification

`cargo test -p scribblecap-formats --locked` covers all six containers, independent
ordinary-image decoding of exports, exact lossless pixels and alpha, JPEG alpha
compositing, animated GIF first-frame selection, EXIF transforms, grayscale/16-bit
normalization, invalid pixel buffers, unsupported/truncated files and excessive
declared dimensions. All test inputs are generated from synthetic data; there are
no proprietary fixtures.
