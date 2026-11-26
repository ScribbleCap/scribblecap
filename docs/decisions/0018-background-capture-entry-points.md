# 0018 — Tray, global shortcuts and single-instance capture requests

_Status: Accepted · 2026-11-26_

## Context

M4-6 adds capture while the editor is hidden or unfocused. The M4-5 capture flow
must remain the only capture implementation and must keep its unsaved-work guard.
Different desktops offer different tray/shortcut integration.

## Decision

Use Tauri's built-in tray-icon feature, official tauri-plugin-global-shortcut 2.4.1
and tauri-plugin-single-instance 2.5.5 (both Apache-2.0 OR MIT). No separate tray
crate is added. On Linux, use ashpd 0.14.3 (MIT, already used by capture) with its
GlobalShortcuts feature and futures-util 0.3 (MIT OR Apache-2.0, already locked).
New lockfile packages and their licence expressions are listed in
[dependency notes](../m4-6-dependencies.md).

One Rust dispatcher forwards typed requests from the tray, global shortcut and CLI
to the running editor page, with a bounded startup queue and a listener-ready
handshake. The page need not be visible or focused. It uses the existing unsaved
prompt before invoking the shared shell_capture/countdown implementation; dirty
work brings the editor forward first. This avoids maintaining a second document
or bypassing the guard in a native shortcut handler.

The default region shortcut is CmdOrCtrl+Shift+Digit2: Command on macOS and Control
on Windows/Linux. Registration failure is nonfatal and appears in Settings. A new
native binding is registered before removing the old one. Wayland uses the
GlobalShortcuts portal rather than initializing the X11-oriented native plugin;
portal approval and its reported trigger are shown in Settings. Without a usable
portal, shortcut editing is disabled with a reason.

Use a new monochrome reduction of our own capture-frame/loop-arrow artwork, recorded
in assets/icon/tray-template.svg and rasterized from the matching line geometry.
It is a template on macOS, and changes foreground for light/dark themes elsewhere.
Linux creates a tray only when a StatusNotifier watcher is present; failed tray
creation is nonfatal. No tray means close-to-tray is disabled, preventing a hidden
editor with no tray entry on stock GNOME. The startup availability check is not a
promise that the desktop's tray will remain present throughout the session.

Persist the shortcut, keep-running and hide-Dock options in the existing defaulted
settings JSON. Older files load with defaults. Close hides the main window only
when a tray exists and keep-running is enabled; it retains the document unchanged.
A real Quit (tray, menu or OS exit request) goes through the unsaved prompt, cancels
capture and waits for in-flight UI work before exiting. Hide Dock uses macOS's
Accessory activation policy only with an available menu-bar icon.

The app executable accepts capture [region|window|screen|previous] [--delay N].
Choose a bounded 0–60 second CLI delay; the existing menu remains 3, 5 and 10 seconds.
The shared countdown can precede any capture mode. Single-instance forwarding sends
the same typed request as a first launch. The separate scribblecap CLI is unchanged.

## Consequences

The background editor webview stays alive when hidden. Renderer availability is
required for the document/unsaved guard, while neither focus nor visibility is
required. Custom commands are declared and granted only to the main capability;
overlays receive no new permissions. Tray/shortcut/portal behavior still needs real
desktop checks on each OS. No source, icon font or artwork from other apps is used.
