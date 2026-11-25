# 0017 — Background privacy edits with session undo

_Status: Accepted · 2026-11-25_

## Context

Decision 0015 flattened unrelated annotations and discarded undo on every privacy
edit. The roadmap requires replacing stored original pixels, not destroying
annotation editability. Review requests background-only edits and session undo.
Decision number 0016 is reserved by the M4-5 work.

## Decision

Supersede decision 0015. Apply redact/pixelate only to the background. Keep the
current annotation document and annotation history unchanged. For imported editable
files, render only their background onto the canvas, sanitize it, replace the
background resource and discard unused resources. Preserve imported vector elements
and rerender their preview on the sanitized background. No new annotations are
flattened; selecting imported elements remains an existing editor limitation.

Track a session-only background revision in the WASM editor's normal document undo
history. The document returned for output strips these runtime resource identities.
Native memory retains revision snapshots and switches the active session when undo
or redo restores a revision. Save, export, copy and drag encode only the active
background and document, never the snapshot collection. Undoing a privacy edit
intentionally makes the previous pixels current again; saving that state saves
those pixels. Redo or reapply the privacy edit before sharing a sanitized state.

Snapshots have a 512 MiB combined pixel/encoded-resource budget. If another snapshot
would exceed it, reject the operation without changing the image or history; save
and reopen to start a fresh session. This is a history resource budget, not a
process-memory cap. New documents and raster canvas transforms clear the snapshots.
Normal annotation history retains its existing 500-step limit.

## Consequences

Annotations remain editable after background privacy edits. Background edits and
annotation edits undo/redo in one sequence. Old pixels exist only in session history
and previously created files/clipboard contents, not as hidden saved resources.
Privacy operations never write the opened file; changing background revision clears
the in-place save target so subsequent Save asks for a destination.

Privacy edits do not remove text or sensitive content stored in annotation elements;
those remain on top. Pixelation obscures detail; solid redaction is preferable for
sensitive background data. This does not promise secure erasure of process memory
or prior copies. Invalid operations and failed restores preserve the current state.
