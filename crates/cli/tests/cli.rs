//! Integration tests: run the `scribblecap` binary on temporary files.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};

use scribblecap_formats::{RasterFormat, RasterImage, decode, encode, extract_editable, open};

fn temp_dir() -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "scribblecap-cli-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run<S: AsRef<OsStr>>(args: &[S]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_scribblecap"))
        .args(args)
        .output()
        .unwrap()
}

fn ok(o: &Output) {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}

/// 8x6 opaque blue image.
fn write_blue_png(dir: &Path) -> PathBuf {
    let image = RasterImage::new(8, 6, [0, 0, 255, 255].repeat(48)).unwrap();
    let path = dir.join("shot.png");
    std::fs::write(&path, encode(&image, RasterFormat::Png).unwrap()).unwrap();
    path
}

/// Thick red rectangle stroke over the top-left corner; no background declared.
const DOC: &str = r##"{
  "format": "scribblecap/1",
  "canvas": { "width": 8, "height": 6 },
  "elements": [
    { "id": "r1", "kind": "rectangle", "rect": { "x": 0, "y": 0, "width": 4, "height": 4 },
      "stroke": "#FF0000FF", "width": 4 }
  ]
}"##;

fn write_doc(dir: &Path, json: &str) -> PathBuf {
    let doc = dir.join("doc.json");
    std::fs::write(&doc, json).unwrap();
    doc
}

fn annotate(dir: &Path, out_name: &str) -> PathBuf {
    let image = write_blue_png(dir);
    let doc = write_doc(dir, DOC);
    let out = dir.join(out_name);
    ok(&run(&[
        OsStr::new("annotate"),
        image.as_os_str(),
        doc.as_os_str(),
        OsStr::new("-o"),
        out.as_os_str(),
    ]));
    out
}

fn pixel(image: &RasterImage, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * image.width() + x) * 4) as usize;
    image.pixels()[i..i + 4].try_into().unwrap()
}

#[test]
fn version() {
    let o = run(&["--version"]);
    ok(&o);
    assert!(String::from_utf8_lossy(&o.stdout).starts_with("scribblecap "));
}

#[test]
fn help_lists_commands() {
    let o = run(&["--help"]);
    ok(&o);
    let text = String::from_utf8_lossy(&o.stdout);
    for command in ["annotate", "render", "convert"] {
        assert!(text.contains(command), "{text}");
    }
}

#[test]
fn unknown_command_fails_with_usage() {
    let o = run(&["frobnicate"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("Usage"));
}

#[test]
fn missing_output_is_usage_error() {
    let o = run(&["convert", "a.png"]);
    assert_eq!(o.status.code(), Some(2));
}

#[test]
fn annotate_embeds_document_and_original_background() {
    let dir = temp_dir();
    let out = annotate(&dir, "out.png");
    let opened = open(&std::fs::read(&out).unwrap()).unwrap();
    let editable = opened.editable.expect("editable document embedded");
    let bg = editable
        .document
        .background
        .as_ref()
        .expect("background added");
    assert_eq!(editable.document.elements.len(), 1);
    assert_eq!(editable.resources.len(), 1);
    assert_eq!(editable.resources[0].id, bg.resource);
    assert_eq!(editable.document.resources[0].media_type, "image/png");
    assert_eq!(
        editable.resources[0].bytes,
        std::fs::read(dir.join("shot.png")).unwrap()
    );
    // Preview is flattened: annotation drawn over the blue background.
    assert_eq!(pixel(&opened.raster.image, 0, 0), [255, 0, 0, 255]);
    assert_eq!(pixel(&opened.raster.image, 7, 5), [0, 0, 255, 255]);
}

#[test]
fn annotate_uses_declared_background_id() {
    let dir = temp_dir();
    let image = write_blue_png(&dir);
    let doc = write_doc(
        &dir,
        r#"{"format":"scribblecap/1","canvas":{"width":8,"height":6},
            "background":{"resource":"bg-1"},
            "resources":[{"id":"bg-1","mediaType":"image/png"}],"elements":[]}"#,
    );
    let out = dir.join("out.jpg");
    ok(&run(&[
        OsStr::new("annotate"),
        image.as_os_str(),
        doc.as_os_str(),
        OsStr::new("-o"),
        out.as_os_str(),
    ]));
    let editable = extract_editable(&std::fs::read(&out).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(editable.resources[0].id.0, "bg-1");
}

#[test]
fn annotate_rejects_flat_only_target() {
    let dir = temp_dir();
    let image = write_blue_png(&dir);
    let doc = write_doc(&dir, DOC);
    let out = dir.join("out.bmp");
    let o = run(&[
        OsStr::new("annotate"),
        image.as_os_str(),
        doc.as_os_str(),
        OsStr::new("-o"),
        out.as_os_str(),
    ]);
    assert_eq!(o.status.code(), Some(1));
    assert!(!out.exists());
}

#[test]
fn annotate_rejects_invalid_json() {
    let dir = temp_dir();
    let image = write_blue_png(&dir);
    let doc = write_doc(&dir, "{}");
    let out = dir.join("out.png");
    let o = run(&[
        OsStr::new("annotate"),
        image.as_os_str(),
        doc.as_os_str(),
        OsStr::new("-o"),
        out.as_os_str(),
    ]);
    assert_eq!(o.status.code(), Some(1));
    assert!(!out.exists());
}

#[test]
fn render_flattens_to_plain_image() {
    let dir = temp_dir();
    let editable = annotate(&dir, "out.webp");
    let flat = dir.join("flat.jpg");
    ok(&run(&[
        OsStr::new("render"),
        editable.as_os_str(),
        OsStr::new("-o"),
        flat.as_os_str(),
    ]));
    let bytes = std::fs::read(&flat).unwrap();
    assert!(extract_editable(&bytes).unwrap().is_none());
    let decoded = decode(&bytes).unwrap();
    assert_eq!(decoded.format, RasterFormat::Jpeg);
    let p = pixel(&decoded.image, 0, 0);
    assert!(p[0] > 200 && p[2] < 60, "{p:?}");
}

#[test]
fn render_requires_editable_input() {
    let dir = temp_dir();
    let image = write_blue_png(&dir);
    let out = dir.join("x.png");
    let o = run(&[
        OsStr::new("render"),
        image.as_os_str(),
        OsStr::new("-o"),
        out.as_os_str(),
    ]);
    assert_eq!(o.status.code(), Some(1));
    assert!(!out.exists());
}

#[test]
fn convert_preserves_document_when_supported() {
    let dir = temp_dir();
    let png = annotate(&dir, "out.png");
    let jpg = dir.join("out.jpg");
    ok(&run(&[
        OsStr::new("convert"),
        png.as_os_str(),
        OsStr::new("-o"),
        jpg.as_os_str(),
    ]));
    let before = open(&std::fs::read(&png).unwrap()).unwrap().editable;
    let after = open(&std::fs::read(&jpg).unwrap()).unwrap();
    assert_eq!(after.raster.format, RasterFormat::Jpeg);
    assert!(before.is_some());
    assert_eq!(after.editable, before);
}

#[test]
fn convert_to_flat_only_format_warns_and_drops_document() {
    let dir = temp_dir();
    let png = annotate(&dir, "out.png");
    let tiff = dir.join("out.tiff");
    let o = run(&[
        OsStr::new("convert"),
        png.as_os_str(),
        OsStr::new("-o"),
        tiff.as_os_str(),
    ]);
    ok(&o);
    assert!(String::from_utf8_lossy(&o.stderr).contains("warning"));
    let decoded = decode(&std::fs::read(&tiff).unwrap()).unwrap();
    assert_eq!(decoded.format, RasterFormat::Tiff);
    assert_eq!(pixel(&decoded.image, 0, 0), [255, 0, 0, 255]);
}

#[test]
fn convert_flat_flag_and_format_override() {
    let dir = temp_dir();
    let png = annotate(&dir, "out.png");
    let out = dir.join("noext");
    ok(&run(&[
        OsStr::new("convert"),
        png.as_os_str(),
        OsStr::new("--flat"),
        OsStr::new("--format"),
        OsStr::new("webp"),
        OsStr::new("-o"),
        out.as_os_str(),
    ]));
    let bytes = std::fs::read(&out).unwrap();
    assert_eq!(decode(&bytes).unwrap().format, RasterFormat::WebP);
    assert!(extract_editable(&bytes).unwrap().is_none());
}

#[test]
fn convert_plain_image() {
    let dir = temp_dir();
    let png = write_blue_png(&dir);
    let gif = dir.join("x.gif");
    ok(&run(&[
        OsStr::new("convert"),
        png.as_os_str(),
        OsStr::new("-o"),
        gif.as_os_str(),
    ]));
    let decoded = decode(&std::fs::read(&gif).unwrap()).unwrap();
    assert_eq!(decoded.format, RasterFormat::Gif);
}

#[test]
fn unknown_extension_needs_format() {
    let dir = temp_dir();
    let png = write_blue_png(&dir);
    let out = dir.join("x.unknown");
    let o = run(&[
        OsStr::new("convert"),
        png.as_os_str(),
        OsStr::new("-o"),
        out.as_os_str(),
    ]);
    assert_eq!(o.status.code(), Some(2));
}
