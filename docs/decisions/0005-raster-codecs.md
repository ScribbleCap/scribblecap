# 0005 — Raster codecs and pixel interchange

_Status: **Accepted** · 2026-10-29_

## Context

Roadmap M1-5 needs portable PNG, JPEG, WebP, TIFF, BMP and GIF input and flat
output. The formats crate must remain independent of UI and operating-system APIs,
and must interoperate with the later renderer without depending on its geometry.

## Decision

- Use `image` 0.25.10 (MIT OR Apache-2.0), with default features disabled and only
  `png`, `jpeg`, `webp`, `tiff`, `bmp` and `gif` enabled. Its codecs are implemented
  in Rust; no native codec installation is needed. HEIF/AVIF and SVG remain M6 work.
- Expose owned RGBA8 pixels with straight alpha through `RasterImage`. The API
  accepts and returns bytes; callers handle filesystem, clipboard and dialog I/O.
- Detect input by its signature. Apply the decoder's EXIF orientation before
  converting to RGBA8. Read the first GIF frame and first TIFF image only.
- Use the library's default decoder limits and check its default 512 MiB budget
  against both decoded and normalized buffer sizes before allocating pixels.
  This is not a bound on total process memory or every codec's internal allocation.
- Encode PNG, lossless WebP, TIFF and BMP as RGBA8. JPEG uses quality 90 and
  composites alpha against white. GIF uses the codec's palette quantization and
  binary transparency. All exports are single, flat images.
- Do not copy source metadata, extra pages/frames or hidden editable payloads.
  M1-8 will add editable container handling as a separate operation.

## Consequences

- The renderer and UI adapters must supply straight RGBA8; a premultiplied renderer
  must unpremultiply before passing pixels to the formats crate.
- High-bit-depth input is reduced to 8 bits per channel. Color profiles and other
  source metadata are not preserved; ICC color conversion is not implemented here.
- JPEG is lossy; GIF reduces colors and alpha. They cannot be tested with general
  exact-pixel equality. PNG, WebP, TIFF and BMP round trips preserve RGBA8 pixels.
- The direct dependency and enabled codec dependencies have compatible licences.
  The new graph includes `tiff` (MIT), `moxcms`/`pxfm` (BSD-3-Clause OR Apache-2.0)
  and image-rs codecs (MIT OR Apache-2.0); the lockfile records exact versions.
