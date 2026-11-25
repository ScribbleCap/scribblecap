# Wayland capture

Wayland sessions use the xdg-desktop-portal Screenshot interface. Install
`xdg-desktop-portal` and a desktop backend that implements Screenshot (for
example, the GNOME or KDE backend for that desktop). A running user D-Bus
session is required. A missing or misconfigured backend produces an error.

```sh
cargo run -p scribblecap-capture --example capture -- screen
```

Approve the portal's screenshot request. The image is written to
`target/capture/screen.png`. Cancelling the request returns `PermissionDenied`
and writes no image. Each capture may prompt; there is no promise of silent
capture or a permanent permission grant.

The portal controls which screen/window/area its dialog offers. Only
`capture_screen(None)` is supported: listing displays/windows, passing their
IDs and requesting desktop-coordinate regions return explicit `Unsupported`
errors. Capture then crop in the editor when needed. Portal images use pixel
coordinates with scale 1, because no desktop geometry or DPI metadata is
provided. The portal may include ScribbleCap's window; callers should hide it
before capture when appropriate. No parent identifier is supplied, so the
dialog is nonmodal and desktop placement is controlled by the portal.

PNG data is decoded through the normal raster decoder. Only local `file:` URIs
are accepted; percent-encoded filenames are supported. Portal files are left
in place because the portal controls their lifetime. File reads stop at 512 MiB;
`scribblecap_formats::decode` uses the codecs' default allocation limits and,
before decoding pixel data, reserves the larger of the codec output size and
normalized RGBA size against `image::Limits::default()` (currently 512 MiB).
The formats regression test `rejects_oversized_output_before_allocating_pixels`
covers this check using an oversized image header without a pixel payload.

The capture backend's separate 256 MiB RGBA budget is checked after decoding;
it is an acceptance limit, not a promise that the decoder allocates at most
256 MiB. Neither limit caps total process memory or the sum of encoded data,
codec working memory and output buffers.

## Validation

Automated tests on every OS cover URI handling, rejected remote/ambiguous URIs,
RGBA decode, missing/corrupt screenshots, file preservation and session
detection. Linux cross-compilation checks the real portal code. These checks do
not replace interactive testing.

Real Linux desktop validation is **not yet recorded**. A Linux tester should:

1. On GNOME Wayland and KDE Wayland, run `screen` and approve. Open the output
   in an ordinary viewer; check dimensions, colours and the chosen target.
2. Cancel the dialog; confirm a permission error and no new output file.
3. Repeat capture with fractional scaling and multiple monitors. Confirm the
   output is usable while reported scale stays 1; check compositor behaviour
   when choosing a region/window if the portal offers those options.
4. Check unsupported `list`, `displays`, `screen 1`, `window 1` and `region`
   operations report their limitations without opening a screenshot dialog.
5. Test a wlroots desktop with a Screenshot-capable portal backend. Record the
   compositor/backend versions; missing Screenshot support must fail clearly.
6. Test a missing portal backend/session bus and record the error. Recheck an
   X11 desktop to confirm it still uses the existing X11 backend.

Architecture and dependencies: [decision 0012](decisions/0012-wayland-screenshot.md).
