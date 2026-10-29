# 0002 — Screen capture per platform

_Status: **Accepted** · 2026-10-29_

## Context

Capture is the only part that cannot be shared code. Each OS restricts it differently.

| OS | API | Notes |
|---|---|---|
| macOS 12.3+ | ScreenCaptureKit (`SCScreenshotManager`, `SCShareableContent`) | Needs Screen Recording permission; `CGWindowListCreateImage` (used by Skitch) is obsolete |
| Windows 10+ | Windows.Graphics.Capture; DXGI Desktop Duplication as fallback | Window picker available from the OS |
| Linux X11 | XGetImage / XComposite | Full access |
| Linux Wayland | `xdg-desktop-portal` Screenshot / ScreenCast | The user confirms in a system dialog; **no silent capture, no global hotkeys by default** (use the portal GlobalShortcuts where available, else tell users to bind a desktop shortcut to `app capture`) |

## Decision

- A `capture` crate with one trait (`capture_region`, `capture_window`,
  `capture_screen`, `list_windows`) and one backend per OS, picked at runtime.
- The region/window selection overlay is our own UI on all platforms except Wayland,
  where we capture the full screen through the portal and then let the user pick the
  region in our editor.
- A `app capture --region|--window|--screen` CLI entry point so desktop shortcuts and
  scripts can trigger capture on every OS.

## Consequences

- Wayland users get a slightly different flow (one portal dialog). Documented, not hidden.
- Capture ships after the editor (roadmap M4); until then, open files and paste from the
  clipboard.
