# 0013 — Native file output and drag-out

_Status: Accepted · 2026-11-24_

## Context

Sharing needs real OS file drags on macOS, Windows and Linux. Browser drag events
cannot reliably deliver generated image files to other desktop applications. Saves
must preserve editable data and leave existing files intact when encoding or writing
fails.

## Decision

Use the `drag` 2.1.1 Rust crate (MIT OR Apache-2.0) directly from the Tauri shell,
on macOS and Windows, with a copy operation and native main-thread dispatch.
Linux uses a small GTK 0.18 adapter (MIT) and `url` 2 (MIT OR Apache-2.0) for file
URIs. It uses button index 1 and retains URI delivery until drag-end, avoiding
upstream drag 2.1.1 GTK gesture and early-cleanup issues. No generic path-based drag command or plugin
filesystem permission is exposed to JavaScript. The application generates a PNG
from bounded RGBA input and the current WASM document, using the core renderer.
It retains imported editable documents and merges new annotations with collision-free
IDs; fresh documents embed the original normalized PNG background. Only the
generated file is supplied to the OS.

Use `tempfile` 3 (MIT OR Apache-2.0) for private sharing directories and same-folder
atomic save replacement. Encode completely before touching the destination, write
and sync a temporary file, then persist it over the chosen destination. Save and
Save As always write an editable PNG, and Save replaces the opened file only when it
is already an editable PNG, so an opened photo is never re-encoded in place. Export
flat is explicit, lets the user choose the format, and refuses to replace an
existing editable or damaged destination. Saves keep a replaced file's permissions.

## Consequences

Native drag begins from pointer-down while the primary button is held. If released
before encoding finishes, the generation is cancelled before dispatch. Keyboard
activation copies the image instead. Generated files remain alive until application
shutdown because destination applications may read after reporting a completed
drop; repeated shares therefore consume temporary disk space during the session.

OS clipboard and native drag behavior need interactive validation on all three
platforms. In particular, the GTK adapter needs a real Linux
session to verify gesture initiation and destination transfer; a successful macOS
build does not validate Linux drag delivery. Dependency source/API reviewed at
[drag-rs](https://github.com/crabnebula-dev/drag-rs) and
[tempfile](https://github.com/Stebalien/tempfile).

Imported annotations remain in the preview and preserved document; this editor
session adds new editable annotations but cannot yet select imported elements. Pixel IPC currently uses
JSON arrays on output operations; the 256 MiB pixel limit is not a total-memory
limit, and a binary output transport should precede raising it.
