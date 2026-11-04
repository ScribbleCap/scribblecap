# Windows capture (M4-2)

`backend()` selects Windows.Graphics.Capture on Windows 10 2004+ / Windows 11.
It captures displays, covered windows and regions. Output is straight-alpha RGBA8,
without cursor; the standard Windows capture border remains enabled. Calls block:
use a worker thread. No UI dispatcher or persistent permission prompt is required.
See [decision 0010](decisions/0010-windows-capture.md).

Unsupported systems return `Unsupported`; access denied becomes `PermissionDenied`;
missing/filtered targets become `NotFound`; device failures, resize races, ten-second
frame timeouts and oversized buffers become `Os`. Lists omit minimized, cloaked,
tool, untitled and own-process windows. App names are executable stems, or empty
when the process is inaccessible. IDs are native 32-significant-bit handle IDs,
resolved against current targets; re-enumerate after topology changes.

## Coordinates

Use returned bounds/scale: global physical pixels divided by the **system** DPI
scale, primary origin `(0, 0)`, negative positions left/above primary. Every monitor
uses that scale, including secondary monitors with different UI scaling. Pixels
are never resampled. A 100-point region at scale 1.5 captures 150 pixels; fractional
regions round both edges. Cross-monitor regions clip to the greatest overlap.
Do not pass client-local/Tauri coordinates directly. Convert physical pixels by
dividing by the returned scale.

## Windows validation

On an unlocked interactive desktop with an application window open:

```sh
cargo test -p scribblecap-capture --locked interactive_capture_smoke_test -- --ignored
cargo run -p scribblecap-capture --example capture -- displays
cargo run -p scribblecap-capture --example capture -- list
cargo run -p scribblecap-capture --example capture -- screen
cargo run -p scribblecap-capture --example capture -- screen <display-id>
cargo run -p scribblecap-capture --example capture -- window <window-id>
cargo run -p scribblecap-capture --example capture -- region <x> <y> <width> <height>
```

Open `target/capture` PNGs in Photos and check:

- Primary and secondary displays, including monitors to the left/above primary.
- Mixed 100%/150%/200% scaling: crop edges and dimensions match returned bounds.
- Covered application windows capture their own content.
- Own windows are omitted from monitor/region images; original affinity is restored
  after both successful and failed captures.
- Cross-monitor clipping; invalid IDs/offscreen regions fail.
- Closing/resizing a target returns current content or an error, never padded garbage.
- Repeated captures leave no stuck border/session or changed caller DPI context.

The ignored interactive test checks enumeration, all monitors, region sizes, one
window and invalid inputs. Unattended CI cannot guarantee an unlocked GPU desktop.
Visual color/occlusion/exclusion checks remain manual.

Developed on macOS with Windows cross-compilation; **interactive Windows capture
has not been run**. HDR tone mapping and DXGI fallback are not implemented.
Protected content may be blank; locked/secure desktops may fail or time out.
