# Raw pixel request preparation benchmark

Run `node ui/scripts/benchmark-pixel-request.mjs` from the repository root with the
Node version pinned by the UI. The script loads the production SCOP encoder and
compares it with the previous `Array.from(pixels)` plus `JSON.stringify` path.
It uses a 3840 × 2160 RGBA image with repeating bytes 0–255, one warm-up and the
median of three measured runs per encoder. Metadata contains dimensions and a
small document string; real annotation documents can be larger.

On macOS arm64, Node v24.14.0, measured 2026-11-24:

| Request preparation | Body bytes | Median time |
|---|---:|---:|
| JSON number array and stringify | 118,454,483 | 1,794.10 ms |
| SCOP binary envelope | 33,177,684 | 4.83 ms |

This is about 72% fewer body bytes and 371× less frontend preparation time for
this fixture. The binary packet is the 33,177,600 pixel bytes plus 84 bytes of
framing and metadata. JSON expansion depends on pixel values.

These measurements cover frontend preparation only. They exclude native IPC,
Rust decoding, rendering, image encoding, dialogs and clipboard/drag delivery.
They do not establish an end-to-end desktop speedup or peak memory measurement.
Tauri's webview fallback can further affect actual transport cost.
