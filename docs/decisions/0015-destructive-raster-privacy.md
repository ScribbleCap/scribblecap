# 0015 — Destructive raster privacy commits

_Status: Accepted · 2026-11-25_

## Context

Pixelate and redact must change stored resources, rather than cover sensitive pixels
with removable annotations. Imported editable images retain original resources in
the native session, while the UI's annotation history retains whole documents.

## Decision

A privacy commit first flattens the visible preview and current annotations through
the core renderer. The core replaces every RGBA channel inside the integer region:
solid redaction writes opaque black; pixelation averages each clipped 12-pixel block
over white and writes opaque results. Bounds and input lengths are checked first.

After successful processing and response allocation, Rust clears the imported editable
session, preview and save destination. The UI loads the sanitized image as a fresh
document and resets annotation history. Subsequent editable saves can only embed the
sanitized background. Privacy commits are deliberately irreversible within the
session; visible controls explain this before applying. Existing disk files are
unchanged, and the next save asks for a destination.

Use the existing raw pixel request envelope. No new dependency or editable file
format is needed. Invalid requests preserve the previous session and image.

## Consequences

Privacy commits flatten all annotations and clear undo/redo. They are exceptions to
the ordinary annotation undo contract. A later annotation can still be edited and
undone over the sanitized background. This does not promise secure erasure of
previous process allocations, existing files, clipboard contents or prior exports.
Pixelation is an obscuring effect; use solid redaction for sensitive information.
