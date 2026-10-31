# 0009 — Deterministic text and step rendering

_Status: **Accepted** · 2026-10-31_

## Context

M1-4b needs text, bubbles, labels and numbered circles in headless exports and
shared geometry for the editor. Host fonts and platform rasterizers cannot provide
identical output across macOS, Windows and Linux.

## Decision

- Bundle the unmodified Noto Sans Regular font (SIL Open Font License 1.1), with
  its licence and provenance in `crates/core/assets/fonts`. This is the default
  sans-serif and the deterministic fallback for every unknown `font` family.
  Never discover fonts through OS APIs. Latin, Greek and Cyrillic are supported;
  glyphs outside the bundled font return an explicit error, not missing-glyph boxes.
- Pin `rustybuzz` 0.20.1 (MIT) for shaping, its `ttf-parser` for glyph outlines,
  `unicode-bidi` 0.3.18 (MIT OR Apache-2.0) for visual runs,
  `unicode-linebreak` 0.1.5 (Apache-2.0) for word breaks and
  `unicode-segmentation` 1.13.3 (MIT OR Apache-2.0) for grapheme-safe long-word breaks.
  Shaping supplies kerning, ligatures and combining mark placement. Convert
  quadratic glyph curves to cubic paths; tiny-skia paints the same core paths
  that the editor can consume. No text rasterizer or system font API is used.
- Treat origin as the top-left of the ascender box. Line height is the font's
  ascender minus descender plus line gap, scaled by font size / units per em.
  Preserve explicit empty lines; normalize CRLF/CR to LF and tabs to four spaces.
  Wrap at Unicode opportunities; break overlong words at graphemes, allowing one
  grapheme to exceed an impossibly narrow width. Alignment uses wrapWidth when
  present, otherwise the widest line. Wrapping constrains advances, not ink bounds.
- Plain text and labels use a contrasting rounded stroke (0.28 × font size),
  then colored glyphs. Contrast uses weighted sRGB brightness (299R+587G+114B
  below 128000 means white; otherwise black), retaining the annotation alpha.
  A label's absolute origin controls rendering; attachment controls future editor
  movement/deletion, without duplicating position calculations during export.
- Bubbles use an original rounded box and bottom-left triangular pointer,
  padding/radius 0.3 × font size, contrasting fill and colored glyphs. This is
  original geometry, not copied Skitch artwork. Decorations use source-over;
  translucent overlaps can darken as with other separate paint operations.
- Steps use the stored circle color and contrasting digits. Fit the glyph path's
  control-point bounds to 65% of diameter horizontally and 55% vertically, then
  center it. All positive u32 numbers fit, including ten-digit values.
- Limit each text block to 16 KiB before and after whitespace normalization.
  Limit wrapping measurements to one million input bytes per paragraph. Excessive
  work, unavailable glyphs and unrepresentable metrics fail explicitly.

## Consequences

- Saved document JSON is unchanged; font names remain stored, but only bundled
  Noto Sans is rendered in this version. More script fonts and family selection
  need a subsequent decision and fixtures. Bidirectional ordering does not imply
  coverage for scripts absent from the bundled font.
- The binary grows by about 556 KiB plus pure Rust shaping code. New transitive
  dependencies use MIT or MIT/Apache-2.0: ttf-parser, core_maths, libm, unicode-script,
  unicode-properties, unicode-ccc and unicode-bidi-mirroring.
- Exact PNG goldens cover plain, bubble, label and multi-digit steps. Three-platform
  CI establishes cross-OS equality; a local macOS run alone cannot establish it.
