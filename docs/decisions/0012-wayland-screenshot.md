# 0012 — Wayland screenshots through the desktop portal

_Status: **Accepted** · 2026-11-24_

## Context

Decision 0002 chooses xdg-desktop-portal for Wayland. XWayland cannot capture
native Wayland windows, and the existing capture trait uses OS display/window
IDs and desktop coordinates that the Screenshot portal does not expose.
The first implementation needs a single image, rather than a PipeWire video
stream and its additional native dependencies.

## Decision

Use `ashpd` 0.13's Screenshot client on Linux, with only its `tokio` and
`screenshot` features. Run asynchronous portal I/O on a private current-thread
Tokio runtime inside the blocking capture call. Use `url` to decode local file
URIs, including percent escapes, and the existing `scribblecap-formats` decoder
to obtain straight-alpha RGBA8. The new direct dependencies are `ashpd` (MIT),
`tokio` (MIT), and `url` (MIT OR Apache-2.0); the latter two already occur in the
workspace dependency graph. None require PipeWire or a GTK dependency here.

Select this backend when either Wayland session indicator is present.
`capture_screen(None)` sends an interactive, nonmodal Screenshot request without
a parent window identifier. The portal controls confirmation and target
selection. Cancellation/not-allowed maps to `PermissionDenied`. Unsupported
portal interfaces produce an actionable error; other transport failures remain
OS errors. No fallback to XWayland or external screenshot executables occurs.

## Consequences

Display/window enumeration, explicit display/window IDs and desktop-coordinate
regions return `Unsupported`. Portal output has neither desktop origin nor
scale, so the image uses pixel coordinates and scale 1. Own-window exclusion
and primary-display selection cannot be promised. The editor can crop the
result later; the desktop portal may offer its own target selection.

`has_permission` stays false: no portable persistent permission probe exists.
`request_permission` allows the operation to proceed without making an extra
screenshot; confirmation happens inside the actual capture request. The trait
documents this per-request exception. Callers must run capture on a worker
thread, as for the existing blocking backends.

Portal-returned files are read but never deleted: ownership and lifetime may
belong to the portal or document portal. Manual desktop tests remain necessary
on GNOME, KDE and wlroots; a cross-compilation check cannot validate prompts.

References: [Screenshot portal specification](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Screenshot.html),
[ashpd screenshot module](https://docs.rs/ashpd/0.13.13/ashpd/desktop/screenshot/index.html).
