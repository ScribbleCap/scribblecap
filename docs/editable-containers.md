# Editable PNG, JPEG and WebP (envelope version 1)

_M1-8 implementation; wire format proposed in [decision 0008](decisions/0008-editable-container-layout.md)._

Each file is a normal flattened image with one hidden editable document and its
resources. Ordinary viewers display the flattened image. ScribbleCap reopens the
document and decodes its original background resource to move/remove annotations.
The flattened preview must never become the restored document's background.

## Public API

`EditableDocument::new(document, resources)` validates the document and its exact
resource inventory and sorts `ResourceData { id, bytes }` into document order.
Public fields may be edited; saving validates again.

- `encode_editable(&preview, format, &editable)` writes PNG, JPEG or WebP. The caller
  supplies freshly rendered straight RGBA8 pixels. Dimensions must equal the canvas
  extents rounded up. TIFF/BMP/GIF remain flat-only.
- `extract_editable(&bytes)` checks framing/checksums and returns optional metadata
  without decoding preview pixels. Plain supported images return `None`.
- `open(&bytes)` additionally decodes the preview and returns `OpenedImage { raster,
  editable }`. Recognized damaged/future data is an error, rather than successful
  opening with silently lost editability. A caller may offer an explicit flat recovery
  view using `decode`, but must preserve the original and avoid overwriting it.

The code handles bytes only. App-level atomic file replacement and user dialogs
belong to the later save workflow, not this library operation.

## Common envelope

All integers in this envelope are **big-endian**. No fields are compressed.

| Field | Bytes | Meaning |
|---|---:|---|
| Magic | 8 | ASCII `SCAPDOC` followed by NUL |
| Version | 4 | u32, currently 1 |
| Preview CRC32 | 4 | Integrity binding to the base preview, defined below |
| JSON length | 8 | u64 byte length |
| Resource count | 4 | u32 entry count |
| Document | variable | UTF-8 `scribblecap/1` JSON, exactly JSON length bytes |
| Resource entries | variable | Entries in document resource order |
| Envelope CRC32 | 4 | IEEE CRC32 of every preceding envelope byte |

Each resource is `id_length:u32`, UTF-8 id bytes, `data_length:u64`, then raw data.
Ids are opaque strings matching the existing document model; they are never used
as filesystem paths. Data must be nonempty. Duplicate, missing or undeclared ids
are rejected. Extra bytes after the envelope entries are rejected.

Limits: envelope 256 MiB, JSON 16 MiB, 4096 resources, 4096 bytes per id. Resource
data and encoded preview allocation coexist with parsed JSON. These limits do not
bound total process memory. A future envelope version must use a new version value;
the independent JSON document version remains unchanged.

## PNG

Write one **`scED`** chunk after all IDAT chunks and before IEND. It carries the
whole envelope, with standard PNG length/type/data/CRC framing. `s` means ancillary,
`c` means private, the third letter is uppercase/reserved, and uppercase `D` means
unsafe to copy after critical image changes. Readers allow other chunks after it
but reject a second `scED` or one before image data. Every chunk's bounds/CRC and
the terminal IEND are checked; trailing bytes are rejected.

The preview CRC32 is over the exact PNG bytes with `scED` removed, including its
signature and all other chunks. This follows the [PNG specification](https://www.w3.org/TR/png-3/)
for unknown private chunks, CRCs and unsafe-to-copy behavior.

## JPEG

Write normal JPEG bytes through their EOI marker, then the envelope, then an
8-byte big-endian envelope length and 8-byte ASCII **`SCAPEDIT`**. The footer
length must cover exactly all bytes between EOI and that footer.

Readers parse marker/segment lengths and entropy stuffing/restart markers to find
the actual EOI. Embedded thumbnail EOI bytes inside APP segments are skipped as
segment content. Non-ScribbleCap vendor trailers are ignored; a recognized partial
or damaged ScribbleCap trailer is an error. The preview CRC32 covers all original
JPEG bytes through EOI. JPEG marker layout is specified by
[ITU T.81](https://www.w3.org/Graphics/JPEG/itu-t81.pdf).

## WebP

Use extended WebP with a `VP8X` header followed by ordinary encoded image chunks,
then one unknown RIFF chunk named **`SCAP`** containing the envelope. Preserve a
preexisting VP8X or add it with the canvas size and the encoded VP8L alpha-used bit.
No reserved feature flag is used for SCAP. The chunk length and RIFF length are
little-endian; odd payload lengths get one zero pad byte excluded from chunk length.
The RIFF size equals total file size minus 8, with no trailing bytes.

The preview CRC32 is over the WebP with SCAP (including its framing/pad) removed
and the RIFF length adjusted to match that base file. The VP8X header stays in
the base file. See the [WebP container specification](https://developers.google.com/speed/webp/docs/riff_container).

## Re-saving, stripping and privacy

Each save creates a fresh encoded preview and embeds only the current document's
declared resource bytes. Old metadata never accumulates. Original background bytes
are preserved, so removing annotations can restore the original. Destructive
pixelation/redaction must replace those resource bytes and prune obsolete resources
before saving; covering sensitive pixels in a preview alone does not remove them.

CRC32 checks detect accidental damage and stale metadata; they do not prevent
intentional tampering. If another service strips the envelope entirely, the image
survives and opens flat; no reader can reconstruct missing editability. Unsupported
or damaged recognized payloads must not be automatically flattened over the source.

## Compatibility checks

Generate synthetic samples without proprietary material:

```sh
cargo run -p scribblecap-formats --example editable_samples -- target/editable-samples
```

Automated tests open all three outputs with an ordinary image decoder, round-trip
documents/resources, remove annotations to recover the original, verify fresh-save
replacement, and reject corrupt/duplicate/truncated/future payloads and changed previews.

Local native-decoder check (2026-10-30): macOS ImageIO, via `sips`, successfully
decoded all three 320×200 samples and converted each to PNG. This supplements the
library tests; it is not a record of default-viewer testing on all three OSes.

M1-8 additionally requires opening PNG/JPEG/WebP samples in the default viewer on
macOS, Windows and Linux. Record viewer/OS results in the PR. Until those manual
checks and three-platform CI pass, the roadmap entry remains in progress.
