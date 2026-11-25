# 0014 — Bounded sharing lifetime and binary pixel requests

_Status: Accepted · 2026-11-24_

## Context

Decision 0013 retained every drag directory until exit and sent output pixels as
JSON number arrays. Repeated drags accumulate files, and JSON conversion creates
large intermediate arrays and strings. A reloaded webview also restarts its local
gesture counter while Rust retains the cancellation watermark.

## Decision

Supersede decision 0013's unbounded retention and JSON pixel transport. Keep the
newest five generated drag directories, with an additional reference retained by
the native completion callback while a drag remains active. Rust issues gesture
IDs for each pointer gesture, so IDs remain monotonic across webview reloads.

Copy, save, drag and canvas transform commands receive Tauri raw binary request
bodies. Use an internal `SCOP` version 1 envelope: four magic bytes, a u32
little-endian version, a u32 little-endian metadata byte length, UTF-8 JSON
metadata, then straight RGBA8 bytes. Metadata includes dimensions and each
command's options/document; pixels never enter a JSON number array. Metadata is
bounded to 16 MiB and pixels to 256 MiB. Reject unsupported versions, malformed
metadata and any pixel length inconsistent with dimensions before copying pixels.
This is an in-process command protocol, not an editable file format.

## Consequences

Active native drags keep their source files even when later drags evict their
cache entries. Completed drags keep a five-share grace window for asynchronous
recipients; recipients that postpone reading across more than five subsequent
shares may lose access. Normal application exit explicitly clears retained files.

The raw envelope requires one contiguous frontend allocation and a native pixel
copy. It removes JSON pixel conversion, but total process memory is still greater
than the pixel cap. Tauri may use a compatibility transport depending on webview
capabilities; actual native IPC throughput remains a desktop measurement.
See [the reproducible 4K preparation benchmark](../pixel-ipc-benchmark.md).

The app uses the existing workspace `serde` and `serde_json` dependencies (MIT OR
Apache-2.0); no new dependency versions or platform integration are introduced.
