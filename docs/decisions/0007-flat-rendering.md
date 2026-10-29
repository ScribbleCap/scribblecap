# 0007 — Flat rendering and original vector stamps

_Status: **Accepted** · 2026-10-29_

## Context

M1-4 is split into shapes/stamps (M1-4a) and text/step numbers (M1-4b). The core
must render without UI, OS APIs or fonts installed on the host. The live editor
and exported pixels should use the same paths. Decision 0005 is allocated to
raster codecs in M1-5; 0006 is reserved for the text-rendering decision.

## Decision

- Use `tiny-skia` **0.12.0**, BSD-3-Clause, as already selected in decision 0001.
  Pin the version, disable default features and enable `std` only. In particular,
  do not enable SIMD. Use the high-quality pipeline for all vector painting.
  PNG helpers are enabled only as a dev dependency for golden fixtures.
- Expose `geometry::shape_paths` as image-pixel `Path` commands plus fill/stroke
  instructions. The renderer adapts these to tiny-skia; a future Canvas painter
  can consume the same geometry. Do not reproduce the arrow or freehand algorithms.
- Paint the decoded background at its document origin, at native size with nearest
  sampling, then each annotation in document order. Use source-over blending,
  antialiased paths, nonzero fills and canvas clipping. Marker/highlighter retain
  their stored width and alpha, with each stroke painted once so self-crossings
  do not darken within an element. Separate overlapping elements do accumulate.
- Use centered strokes: round caps/joins for lines, freehand and stamp symbols;
  butt caps and miter joins (limit 4) for shape boundaries. Clamp rounded-rectangle
  radii to half the shorter side. Zero-area rectangles/ellipses and zero-length
  arrows draw nothing; one-point lines and marker strokes draw round dots.
- Create original colored circular vector stamps for `approved`, `rejected`,
  `question`, `exclaim` and `perfect`. Symbols are check, cross, question mark,
  exclamation mark and star paths, not glyphs. Unknown names use a gray diamond
  badge. Optional tails are triangles rotated using fixed polynomial arithmetic
  rather than platform trigonometry. No Skitch assets are used.
- Expose straight RGBA8 output and accept straight RGBA8 background pixels with an
  explicit resource id. Premultiply only inside the painter, then unpremultiply
  before returning pixels to encoders. Fully transparent output has zero RGB.
  Low-alpha RGB may round through the 8-bit premultiplied representation.
- Round fractional canvas extents up to whole pixels. Reject invalid documents,
  unavailable/mismatched backgrounds, unrepresentable geometry and any individual
  canvas/background buffer over **256 MiB**. Text and step numbers return explicit
  errors until M1-4b, rather than silently disappearing from an export.

## Consequences

- Core remains platform/UI-free and does not depend on image container codecs.
  Callers decode/encode through `formats` and pass pixel buffers across the boundary.
- Scalar rendering prioritizes stable results over maximum SIMD performance.
  Exact RGBA golden comparisons run in the existing three-platform CI; cross-OS
  equality must be demonstrated there, not inferred from a single local run.
- No fonts or text-layout dependencies are introduced in M1-4a. M1-4 is incomplete
  until text, bubbles, labels and step numbers have their own implementation/tests.
- The new runtime graph includes `tiny-skia-path` (BSD-3-Clause), `arrayref`
  (BSD-2-Clause), `arrayvec` (MIT OR Apache-2.0) and `strict-num` (MIT), all compatible
  with the project's MIT OR Apache-2.0 licence.
