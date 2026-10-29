# 0004 — PDF annotation

_Status: **Accepted** · 2026-10-29 · scheduled after the first release (roadmap M6)_

## Context

Users want to mark up PDFs (contracts, specs, slides) with the same tools as images,
and the result must look right in Acrobat, Preview, browsers, Okular and other viewers.
Skitch stored its drawing in a private `/XXSkitch` entry plus flattened appearances
(findings §6), which other apps can't edit.

## Decision

- **Rendering:** PDFium through `pdfium-render` (same engine as Chrome; BSD-3). Pages
  are rendered to bitmaps at the current zoom; annotations are drawn on top by our UI.
- **Saving:** append an **incremental update** (original bytes untouched, so signatures
  on earlier revisions remain verifiable) containing:
  - one **standard annotation per element**, each with an appearance stream so it looks
    identical everywhere:

    | Our element | PDF annotation |
    |---|---|
    | Arrow, line | `Line` with line endings (arrow outline also in the appearance stream) |
    | Rectangle, rounded rect | `Square` |
    | Ellipse | `Circle` |
    | Marker, highlighter | `Ink` (highlighter with opacity / multiply blend) |
    | Text, bubble | `FreeText` |
    | Stamp, step counter | `Stamp` with custom appearance |
    | Pixelate / redact | Real `Redact` workflow: burn into page content on save (destructive by design) |
  - our JSON document as an embedded file, referenced from the catalog, so the app can
    re-open everything editable (grouping, labels, tool sizes).
- **Re-saving** replaces our previous update instead of stacking new ones when the file
  has not been changed by another app since.
- **Opening:** load our document if present; else read standard annotations we
  understand (so PDFs annotated in other apps can be edited); else read Skitch's
  `/XXSkitch` DOM.
- Text and page content of the PDF itself are never edited (non-goal).

## Consequences

- Redaction must actually remove the underlying content, not cover it; needs careful
  tests (text extraction after redaction must return nothing in that area).
- PDFium is a native library shipped per platform (~5 MB).
