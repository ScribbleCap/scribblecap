# Command line (`scribblecap`)

_M5-4._ Headless access to the same core as the app, for scripts and agents. The
binary is built from `crates/cli` (`cargo run -p scribblecap-cli -- <args>`).

```text
scribblecap annotate <IMAGE> <DOCUMENT.json> -o <OUTPUT> [--format <FMT>]
scribblecap render   <EDITABLE>              -o <OUTPUT> [--format <FMT>]
scribblecap convert  <INPUT>                 -o <OUTPUT> [--format <FMT>] [--flat]
scribblecap --version | --help
```

`FMT` is one of `png`, `jpeg`/`jpg`, `webp`, `tiff`/`tif`, `bmp`, `gif`. Without
`--format`, the output's file extension is used. An unrecognised extension is a usage
error. Input formats are detected from the file contents, not the extension.

Exit codes: `0` success, `1` the operation failed (unreadable input, invalid document,
unsupported target, ...), `2` invalid usage. Messages go to stderr. The output file is
written only after all of its bytes are produced, so a failure never leaves a partial
file. Existing output files are overwritten.

## `annotate`: attach an editable document to an image

The image becomes the document's background. It is stored byte-for-byte as the
background resource (see [editable-containers.md](editable-containers.md)). The output
is a normal flattened image, with the annotations rendered by the core renderer. It
also holds the editable document, so the output must be PNG, JPEG or WebP.

The document is `scribblecap/1` JSON ([file-format.md](file-format.md)), and
its `canvas` must match the image size. There are two ways to give the background:

- **Omit it** (simplest). Leave out `background` and `resources`. The CLI adds a
  `background` resource with id `background`, origin `[0, 0]`, and the image's MIME
  type.
- **Declare it**: set `background.resource` and list exactly that one resource. The
  image bytes are attached under that id.

Annotating a file that already holds an editable document is refused. Text and step
elements cannot be rendered yet (M1-4b), so documents that contain them fail.

```sh
cat > notes.json <<'EOF'
{
  "format": "scribblecap/1",
  "canvas": { "width": 1280, "height": 800 },
  "elements": [
    { "id": "a1", "kind": "rectangle",
      "rect": { "x": 100, "y": 120, "width": 400, "height": 160 },
      "stroke": "#FF0000FF", "width": 6, "cornerRadius": 20 },
    { "id": "a2", "kind": "arrow",
      "start": [700, 500], "end": [520, 260], "color": "#FF0000FF", "width": 12 }
  ]
}
EOF
scribblecap annotate screenshot.png notes.json -o annotated.png
```

## `render`: flatten an editable file

The command reads an editable PNG/JPEG/WebP, decodes its background resource, and
renders the document again. The result is a plain image with no embedded document,
in any supported format. A file without an editable document is an error; use
`convert` for plain images.

```sh
scribblecap render annotated.png -o share.jpg
```

## `convert`: change raster format

The command re-encodes the image's pixels. If the input holds an editable document
and the target is PNG, JPEG or WebP, the document and its resources are kept. The
stored preview pixels are reused; nothing is rendered again. For TIFF, BMP or GIF
targets, a warning is printed and a flat image is written. `--flat` always drops
the document, without a warning. A damaged editable payload is an error, not a
silent downgrade.

```sh
scribblecap convert annotated.png -o annotated.webp        # still editable
scribblecap convert annotated.png -o print.tiff            # flat, with a warning
scribblecap convert annotated.png --flat --format jpeg -o out.bin
```

JPEG output uses quality 90 and is composited on white. GIF output is quantised. See
[raster-formats.md](raster-formats.md).
