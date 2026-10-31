# 0008 — Editable raster container layout

_Status: **Proposed** · 2026-10-29 · implementation: M1-8_

## Context

Decision 0003 chose normal image containers with embedded editable JSON and original
resources. M1-8 specifies the bytes so readers can validate, preserve and reopen the
document. It must not write Skitch identifiers, stack stale hidden data on repeated
saves, or silently turn damaged editable files into apparently ordinary images.

## Decision

- Use the versioned, uncompressed binary envelope in [editable-containers.md](../editable-containers.md):
  `SCAPDOC\0`, envelope version 1, preview CRC32, JSON length, resource count,
  the existing `scribblecap/1` JSON, resource entries, and an envelope CRC32.
  Resource ids are the model's UTF-8 ids, not filenames or paths. No archive extraction
  or compression is involved. Resources are serialized in document inventory order.
- Store one envelope in PNG private ancillary **`scED`**, in JPEG after EOI followed
  by a big-endian u64 length and **`SCAPEDIT`**, or in WebP RIFF **`SCAP`**. PNG's
  uppercase last letter marks the chunk unsafe to copy after image modifications.
  WebP uses a `VP8X` extended header, correct alpha flag, padding and RIFF size.
- Bind the envelope to the encoded preview using CRC32 of the base image without
  our envelope/framing. This detects accidental stale metadata when another editor
  changes the image but retains the hidden document. CRC32 is an integrity check,
  not authentication and not a privacy boundary.
- Regenerate a flat preview container on every save. Embed only the supplied current
  resource inventory, never any previous file's trailer/chunks. Preserve original
  background bytes exactly for re-editing; destructive operations must replace those
  resources with sanitized bytes and prune obsolete resources before saving.
- Require an exact, duplicate-free resource inventory. Bound envelopes to 256 MiB,
  document JSON to 16 MiB, entries to 4096 and UTF-8 ids to 4096 bytes. All framing
  is checked before slicing/allocation. Reject duplicate envelopes, bad checksums,
  unsupported envelope/document versions, forged lengths and canvas/preview mismatches.
- `open` returns both the ordinary flattened pixels and optional editable data.
  Recognized corrupt or unsupported data returns an error; callers can explicitly
  show the flat image for recovery while preserving the original file.

## Consequences

- Current model/geometry and text rendering are independent: callers supply an
  up-to-date flattened preview; container code does not render annotations.
- Files grow by the original background/resources plus uncompressed JSON. Large
  embedded inputs may require substantial memory; the limits are per payload,
  not a total process memory bound.
- Foreign editors and messaging services may strip editability. Editors that retain
  metadata can retain original private pixels too: redaction must happen through
  destructive resource replacement or a flat export, not by covering the preview.
- Even harmless changes to retained preview bytes/metadata can fail the conservative
  preview binding. A flat recovery view is explicit, not an automatic overwrite path.
- New direct dependency `crc32fast` 1.5 (MIT OR Apache-2.0), already present in the
  lockfile transitively. This record adds detail to 0003 without changing its choices.
