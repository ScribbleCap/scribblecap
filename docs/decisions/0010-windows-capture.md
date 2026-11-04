# 0010 — Windows capture and desktop coordinates

_Status: **Accepted** · 2026-11-04_

## Context

M4-2 implements Windows.Graphics.Capture behind the synchronous `Capturer` trait.
Native target selection needs Windows 10 1903; excluding our windows needs 2004.
Mixed-DPI desktops need one coherent global coordinate space for region selection.

## Decision

- Require **Windows 10 2004 (build 19041)+**, including Windows 11. Check
  UniversalApiContract v10 and `GraphicsCaptureSession::IsSupported`. This
  supersedes decision 0002's generic Windows 10+ minimum. DXGI Desktop Duplication
  fallback is deferred; unavailable WGC returns `Unsupported`.
- Add Microsoft's `windows` 0.62.2 bindings as a Windows-only direct dependency
  (MIT OR Apache-2.0, already present through Tauri; no new crate versions).
  All native/unsafe code stays in `capture::windows`.
- Run operations on fresh MTA workers with per-monitor-v2 DPI awareness. Balance
  WinRT initialization without changing the caller's apartment or DPI context.
- Use **uniform system-DPI desktop points**: divide all physical monitor/window
  rectangles by `GetDpiForSystem() / 96`. Return that scale for every display/image.
  This preserves adjacency, negative coordinates and region intersections on
  mixed-DPI systems without resampling. It is not a secondary monitor's UI DIP
  scale; callers must use the returned bounds/scale.
- Windows handles have 32 significant bits. Resolve IDs against fresh enumerations;
  never reconstruct native pointers from input. IDs work between example runs while
  targets exist, but Windows can recycle handles after destruction.
- Use a free-threaded one-frame BGRA8 pool, cursor disabled, standard OS border,
  and a ten-second frame wait. D3D multithread protection guards shared device
  access. Stage to CPU memory, respect row pitch, crop, swizzle to RGBA and remove
  premultiplied alpha. Reject resize races and buffers larger than 512 MiB.
- Temporarily exclude our visible top-level windows with `WDA_EXCLUDEFROMCAPTURE`
  during monitor/region capture. Restore original affinity on success/error,
  serialize captures across instances and report exclusion/restoration failures.
  Direct window capture works when covered. List titled, visible, non-minimized,
  non-cloaked application windows, excluding tool windows and our process.

## Consequences

- No picker, tray, hotkeys or DXGI fallback in M4-2.
- Win32 capture has no persistent permission prompt. Permission methods return
  true on supported systems; capture access-denied becomes `PermissionDenied`.
  Protected content may be blank; locked/secure desktops may fail or time out.
- Output is SDR BGRA8/RGBA8. HDR tone mapping is not implemented; HDR colors may clip.
- Pure readback/coordinate tests run on all OSes. Windows CI checks native code;
  real screenshots require the interactive test and checklist in
  [windows-capture.md](../windows-capture.md).

## References

- [Microsoft capture overview](https://learn.microsoft.com/windows/uwp/audio-video-camera/screen-capture)
- [Window exclusion and OS requirements](https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-setwindowdisplayaffinity)
- [System DPI](https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-getdpiforsystem)
- [Handle interoperability](https://learn.microsoft.com/windows/win32/winprog64/interprocess-communication)
