# 0003 — Editable save format

_Status: **Proposed** · 2026-10-29_

## Context

Users want one file that is (a) a normal image/PDF anyone can open and (b) still
editable in the app. Skitch solved this by hiding its document inside the image
(findings §3, §6). We also want to open Skitch's files.

## Decision (proposed)

- Our document is JSON (`document.json`, schema in `docs/file-format.md`, roadmap
  M1-1), versioned with a top-level `"format": "<app-id>/1"`. Resources (original
  background, embedded images) are stored separately and referenced by UUID.
- Embedding per container, always alongside the flattened preview:

| Container | Where the document goes |
|---|---|
| PNG | Private ancillary chunks after the image data (our own chunk names, not Skitch's `skMf`/`skRf`) |
| JPEG | Trailer after the end-of-image marker + length + magic (same layout idea as Skitch, our own magic) |
| HEIF / AVIF | A `uuid` box with our own UUID |
| WebP | A custom RIFF chunk |
| PDF | Standard annotations (visible in every viewer) + our document in an embedded file stream referenced from the catalog, written as an incremental update ([0004](0004-pdf-annotation.md)) |
| Others (TIFF, BMP, GIF, SVG) | Flat export only; offer to save an editable PNG copy |

- The **original, un-annotated background** is stored as a resource so annotations can
  be moved or removed later (pixelate/redact intentionally replace the stored original,
  so redacted data can't be recovered).
- **Reading Skitch files**: detect Skitch markers and convert Skitch's JSON to ours on
  load. We never write Skitch's format.

## Consequences

- Files grow by roughly the size of the original image (stored twice). Offer a "flatten"
  save option for small files.
- Some tools (image optimisers, chat apps) strip unknown chunks; the image survives, the
  editability doesn't. Document this.
