# 0019 — Native system sharing with a flat temporary PNG

_Status: Accepted · 2026-11-26_

## Context

M5-5 needs the desktop share panel. Tauri has no desktop sharing API. Sharing must
use the current rendered image without embedding the editable document or session
privacy history. Linux has no standard share sheet.

## Decision

Add one command, `shell_share_image`, available only to the main editor capability.
It accepts the existing bounded raw SCOP image packet and document, plus the Share
button's logical viewport rectangle. Reuse `output::prepare` exactly as Copy does,
then encode a flat PNG in a private `tempfile` directory. The fixed filename is
`ScribbleCap.png`; neither the opened file nor a user-supplied path is written.

On macOS, show `NSSharingServicePicker` on the main thread with a file URL, anchored
to the Share button after converting the WebView's top-left coordinates to NSView
coordinates. A retained delegate owns the picker and temporary directory through
selection, service completion or cancellation. Release the file at that point.
Completed delegate objects are pruned on the next share and at normal app exit.

On Windows, obtain `DataTransferManager` for the main HWND using
`IDataTransferManagerInterop`, then call `ShowShareUIForWindow` on the UI thread.
Use a data-request deferral to open the PNG as a `StorageFile` on an initialized
WinRT worker thread. Supply it as read-only storage data. Retain its directory
until `ShareCompleted` or `ShareCanceled`, and unregister the data-request handler
on the next share or normal exit. Windows positions this HWND-bound panel; its API
does not offer the macOS button anchor. Cancellation support requires Windows 10
version 2004 or newer; unsupported APIs produce the normal share error.

The UI hides Share… on Linux, while Rust also rejects unsupported platforms.
Cancellation is silent; showing the panel or preparing its data can return the
normal editor error. Normal exit explicitly cleans retained files because Tauri
exits without unwinding all Rust state. An abnormal process termination can leave
an OS temporary directory behind; this is not secure erasure of temporary storage.

Use versions already present in Cargo.lock as new direct app dependencies:
`objc2` 0.6.5, `objc2-app-kit` 0.3.2, `objc2-foundation` 0.3.2, `windows` 0.62.2 and
`windows-collections` 0.3.2. `objc2` and `objc2-foundation` are MIT;
`objc2-app-kit` is Zlib OR Apache-2.0 OR MIT; both Windows crates are MIT OR
Apache-2.0. No new package versions or artwork are introduced.

## Consequences

Sharing uses one renderer and one binary image request format. It does not mark
the document saved. The OS and destination application control the final transfer.
Only the currently selected privacy revision is shared; undoing a privacy edit can
restore sensitive pixels, just as for Copy and Save (decision 0017).

Native panel placement, destination availability and file lifetime require desktop
checks; M5-5 remains in progress until macOS and Windows are checked. See the
checklist in `docs/app-shell.md`.

API references: [Apple NSSharingServicePicker](https://developer.apple.com/documentation/appkit/nssharingservicepicker),
[Microsoft HWND share interop](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-idatatransfermanagerinterop),
[Microsoft ShareCanceled](https://learn.microsoft.com/en-us/uwp/api/windows.applicationmodel.datatransfer.datapackage.sharecanceled).
