# M4-5 Capture inside the app: design

_Status: draft for approval · 2026-11-24 · Roadmap: M4-5 · Builds on decision 0002_

## Intent

Capture is how most ScribbleCap sessions start (design review, item D4). The vision's
speed goal is "capture → annotated → copied in under 5 seconds for a practised user".
The capture backends exist for macOS, Windows, X11 and Wayland (M4-1 to M4-4), but the
app cannot use them yet. M4-5 connects them: a Capture button and menu, our own
selection overlay, previous region and a timer. The result opens in the editor.

## Goals

- **Capture button** at the top left of the title bar (design review D4). Clicking it
  captures a **region**. Its menu offers **Window**, **Full screen**, **Previous region**
  and **Timer**.
- **Our own selection overlay** on macOS, Windows and X11: drag a region or click a
  window, over a frozen picture of the screen.
- **Wayland:** the system portal dialog captures; the user crops in the editor
  (decision 0002). No overlay.
- **One editor window.** A capture replaces the current image.
- **Never lose work silently.** Before replacing an image that has unsaved changes,
  ask **Save… / Discard / Cancel**. This applies to Capture, Open, Paste and Drop.

## Non-goals

- Several editor windows (possible later; nothing here rules it out).
- Global hotkey, tray / menu-bar icon and `app capture` CLI (M4-6, designed
  separately). The maintainer wants a Windows system-tray menu, a macOS menu-bar icon
  with an option to hide the Dock icon, and a Linux tray icon.
- Scrolling capture, video and screen recording.
- Annotating on the overlay before the editor opens.
- Remembering the previous region across app restarts.
- Other settings (theme, HiDPI and so on stay in M5-2).

## Anti-goals (a failure even if it works)

- ScribbleCap's own window appears in the capture.
- What lands in the editor differs from what the user selected: shifted by a few
  pixels, the wrong scale on a HiDPI or mixed-DPI setup, or the wrong monitor.
- Capture feels slow: a visible delay between clicking Capture and seeing the overlay,
  or between releasing the mouse and seeing the editor.
- Annotations are lost without being asked.

## Constraints

- **Ready for M4-6.** The capture flow must start from a Rust entry point that does
  not depend on the editor being visible or focused, so the tray / menu-bar icon and
  a global hotkey can start a capture later without reworking it.

- `core` stays UI-free and platform-free. Overlay and window code lives in `app` and
  `ui`; OS capture stays in the `capture` crate.
- The capture trait is blocking; call it from a worker thread
  (`tauri::async_runtime::spawn_blocking`), like the existing commands.
- Pixel transfer to and from the UI uses the existing binary packets: `SCIM` from Rust
  to the UI (`shell::packet`) and `SCOP` from the UI to Rust (decision 0014). No JSON
  pixel arrays.
- 256 MiB RGBA limit per image, as in `shell::packet`.
- New dependencies need a compatible licence and a note in the PR.
- macOS needs Screen Recording permission (`Capturer::has_permission` and
  `request_permission`).
- Clean room: nothing from `re-work/` or Skitch: no code, artwork, sounds or name.

## Approaches considered

| Approach | Trade-off | Outcome |
|---|---|---|
| **A. Freeze first, then select.** Capture every display, show the frozen pictures in full-screen overlay windows, crop the chosen part from pixels already captured. | What you select is exactly what you get; open menus and tooltips are kept; no timing race; works the same on macOS, Windows and X11. Costs about 33 MB per 4K display until the selection is made. | **Chosen** for macOS, Windows and X11. |
| B. Transparent live overlay, then `capture_region`. | Less memory; live screen. But hiding the overlay before capturing races the compositor, transparent full-screen windows are unreliable on X11 and awkward across macOS Spaces, and hover menus close. | Rejected: flaky. |
| C. Capture the full screen, crop in the editor. | Least work, but adds a step to every capture and is clumsy with several monitors. | **Used on Wayland only**, where the portal leaves no alternative (decision 0002). |

The maintainer picked A, with C only on Wayland.

## Decided

### The capture flow (macOS, Windows, X11)

1. **Unsaved-work check.** If the current image has unsaved changes (annotations, a
   transform, anything since the last Save), show **Save… / Discard / Cancel** before
   anything else. Save… runs the normal Save; if that is cancelled or fails, the
   capture is cancelled too. An untouched capture or an already-saved image is
   replaced without asking.
2. **Hide the editor window** so it is not in the picture, on every platform. Only
   macOS's backend excludes our windows itself, and one rule everywhere is simpler.
   **My call:** hide our window only, not the whole app (no `NSApp hide` on macOS).
   Menus and tooltips of other apps have usually closed by the time the user clicks
   Capture; capturing an open menu is what the Timer is for.
3. **Freeze:** capture every display with `capture_screen(Some(id))`.
4. **Overlay:** one borderless, always-on-top, full-screen window per display, showing
   that display's frozen picture at its native scale.
5. **Select**, then **crop** in Rust from the frozen pixels. Close the overlays, free the
   frozen pictures, show the editor with the result.

`Escape`, or a right click, cancels at any point: close the overlays, show the editor
unchanged.

### Modes

- **Region** (button default): drag a rectangle on any one display. The overlay shows
  a crosshair and the selection's size in pixels.
- **Window:** hovering highlights the window under the pointer (outlines from
  `list_windows`); clicking captures it. **Maintainer's decision:** the result is the
  window's outline cropped from the frozen picture, exactly as it looked when clicked,
  the same on every platform. Anything overlapping the window is included.
  `capture_window` is not used here.
- **Full screen:** the display under the pointer, with no overlay.
- **Previous region:** freeze that one display and crop the same rectangle, with no
  overlay, using the same freeze-and-crop code as Region so pixels and scale match.
  **My call:** it applies only while that display has the same ID, bounds and scale as
  when the region was captured; otherwise fall back to the Region overlay. Disabled
  until a region has been captured in this session.
- **Timer:** a countdown, then the Region flow. **My call:** 3, 5 or 10 seconds, picked
  in the menu. The freeze happens when the countdown ends, so menus opened during the
  countdown are captured. **Maintainer's decision:** the editor hides when the countdown
  starts, and a small always-on-top countdown badge shows in a corner of the display
  under the pointer. The badge closes just before the freeze, so it is never in the
  picture. `Escape` while it has focus, or clicking it, cancels.

### Wayland

The Capture button sends a portal request (`capture_screen(None)`), and the portal's
own dialog decides what is captured. **My call:** the menu keeps Region and Full
screen, and both send that same portal request (the portal may itself offer an area
or window choice). Window and Previous region are shown disabled with a short reason. Timer waits, then sends the
portal request. Cropping happens in the editor with the existing Crop (M2-7). The
unsaved-work check and window hiding apply as above.

### After capture

- The image is named `Screenshot <date time>` (for example
  `Screenshot 2026-11-24 18.42.07`).

### Capture save folder and auto-save (maintainer's decisions)

- **Capture save folder.** The app remembers one folder for captures. The first time
  one is needed and none is set, a folder picker asks for it, and the answer is kept.
  If the folder later disappears or can't be written to, the app asks again.
- **Auto-save is a setting, off by default.**
  - **Off:** nothing is written until the user presses Save. The first Save of a
    capture goes straight to `<folder>/Screenshot <date time>.png` without a dialog
    (an editable PNG, the M2-9 rule); later Saves overwrite that file. Save As still
    asks, starting in the capture folder.
  - **On:** every capture is written to `<folder>/Screenshot <date time>.png` as soon
    as it lands in the editor. Save overwrites that same file. Discarding a capture
    keeps the file that was written.
  - Names carry no counters. They are unique by capture time, to the second; if that
    name already exists (two captures in the same second), the milliseconds are added,
    for example `Screenshot 2026-11-24 18.42.07.315.png`. Never overwrite a file the
    app didn't create for this capture: if even that name exists, wait a millisecond
    and retry with the new time.
  - Only captures use the folder; opened, pasted and dropped images keep the M2-9 rules.
- **Settings panel.** A small Settings panel, built now, with two rows: *Capture save
  folder* (shows the path, with a Change… button) and *Auto-save captures* (a switch,
  off). M5-2's other settings will go here later.
- **Storage.** Settings live in one small file in the app's config folder. How is left
  to the builder; a new dependency needs a decision record (AGENTS.md).
- The image is in pixels. Desktop-point scale from `CapturedImage::scale` is not used
  for editing yet. **My call:** keep it out of the document until HiDPI work in M5-2.

### Errors

- **Permission denied** (macOS Screen Recording, or the portal cancelled): show the
  editor with a short message saying where to grant access. On macOS, link to System
  Settings → Privacy & Security → Screen & System Audio Recording.
- **Backend error:** show the message; the current image is untouched.

## Left to the builder

- How the overlay windows are created and closed in Tauri 2, and how a single
  selection works across several overlay windows (only one display at a time may hold
  a selection).
- How the frozen pixels reach each overlay window (for example, the `SCIM` packet, or
  a smaller preview with the full-resolution crop done in Rust).
- The exact look of the crosshair, size label and window highlight, within
  `design-review.md`.
- How "unsaved changes" is tracked in the UI, as long as Capture, Open, Paste and Drop
  share it.
- Mapping overlay pointer positions to desktop coordinates and display pixels for each
  display's scale.
- Tests: unit tests for coordinate mapping and cropping (pure functions, tested on every
  OS), and UI tests for the menu, the unsaved-work prompt and cancellation.
- Docs to update: `docs/app-shell.md`, `docs/design-review.md` (D4), the roadmap
  (mark M4-5 `[~]` until checked on real desktops), and a decision record if a new
  dependency or a new window type needs one.
