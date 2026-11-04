//! `scribblecap` command line: `annotate`, `render` and `convert`. See `docs/cli.md`.
//!
//! Exit codes: 0 success, 1 the operation failed, 2 invalid usage.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use scribblecap_core::render::{BackgroundImage, render};
use scribblecap_core::{Background, Document, Point, Resource, ResourceId};
use scribblecap_formats::{
    EditableDocument, RasterFormat, RasterImage, ResourceData, decode, encode, encode_editable,
    extract_editable, open,
};

const USAGE: &str = "\
Usage:
  scribblecap annotate <IMAGE> <DOCUMENT.json> -o <OUTPUT> [--format <FMT>]
      Attach an editable document to a plain image. The image becomes the
      document's background; OUTPUT is a flattened, editable PNG/JPEG/WebP.
  scribblecap render <EDITABLE> -o <OUTPUT> [--format <FMT>]
      Re-render an editable file's document into a plain flat image.
  scribblecap convert <INPUT> -o <OUTPUT> [--format <FMT>] [--flat]
      Change raster format. An embedded document is kept when OUTPUT is
      PNG/JPEG/WebP (unless --flat); other targets are written flat.
  scribblecap --version | --help

FMT: png, jpeg (jpg), webp, tiff (tif), bmp, gif. Default: OUTPUT's extension.";

/// A failure: usage errors exit 2, operation errors exit 1.
enum Failure {
    Usage(String),
    Run(String),
}

fn run_err(e: impl std::fmt::Display) -> Failure {
    Failure::Run(e.to_string())
}

struct Args {
    inputs: Vec<PathBuf>,
    output: PathBuf,
    format: RasterFormat,
    flat: bool,
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    match dispatch(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Usage(message)) => {
            eprintln!("scribblecap: {message}\n\n{USAGE}");
            ExitCode::from(2)
        }
        Err(Failure::Run(message)) => {
            eprintln!("scribblecap: error: {message}");
            ExitCode::from(1)
        }
    }
}

fn dispatch(args: Vec<OsString>) -> Result<(), Failure> {
    let Some(command) = args.first().and_then(|a| a.to_str()) else {
        return Err(Failure::Usage("missing command".into()));
    };
    match command {
        "--version" | "-V" => {
            println!("scribblecap {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "--help" | "-h" | "help" => {
            println!("{USAGE}");
            Ok(())
        }
        "annotate" => annotate(&parse(&args[1..], 2, false)?),
        "render" => render_cmd(&parse(&args[1..], 1, false)?),
        "convert" => convert(&parse(&args[1..], 1, true)?),
        other => Err(Failure::Usage(format!("unknown command `{other}`"))),
    }
}

fn parse(args: &[OsString], positional: usize, allow_flat: bool) -> Result<Args, Failure> {
    let mut inputs = Vec::new();
    let mut output = None;
    let mut format = None;
    let mut flat = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.to_str() {
            Some("-o" | "--output") => {
                let value = iter
                    .next()
                    .ok_or_else(|| Failure::Usage("-o needs a path".into()))?;
                output = Some(PathBuf::from(value));
            }
            Some("--format") => {
                let value = iter
                    .next()
                    .and_then(|v| v.to_str())
                    .ok_or_else(|| Failure::Usage("--format needs a value".into()))?;
                format = Some(
                    parse_format(value)
                        .ok_or_else(|| Failure::Usage(format!("unknown format `{value}`")))?,
                );
            }
            Some("--flat") if allow_flat => flat = true,
            Some(s) if s.starts_with('-') && s.len() > 1 => {
                return Err(Failure::Usage(format!("unknown option `{s}`")));
            }
            _ => inputs.push(PathBuf::from(arg)),
        }
    }
    if inputs.len() != positional {
        return Err(Failure::Usage(format!(
            "expected {positional} input path(s), got {}",
            inputs.len()
        )));
    }
    let output = output.ok_or_else(|| Failure::Usage("missing -o <OUTPUT>".into()))?;
    let format = match format {
        Some(format) => format,
        None => output
            .extension()
            .and_then(|e| e.to_str())
            .and_then(parse_format)
            .ok_or_else(|| {
                Failure::Usage(format!(
                    "cannot infer format from `{}`; pass --format",
                    output.display()
                ))
            })?,
    };
    Ok(Args {
        inputs,
        output,
        format,
        flat,
    })
}

fn parse_format(name: &str) -> Option<RasterFormat> {
    Some(match name.to_ascii_lowercase().as_str() {
        "png" => RasterFormat::Png,
        "jpg" | "jpeg" => RasterFormat::Jpeg,
        "webp" => RasterFormat::WebP,
        "tif" | "tiff" => RasterFormat::Tiff,
        "bmp" => RasterFormat::Bmp,
        "gif" => RasterFormat::Gif,
        _ => return None,
    })
}

fn media_type(format: RasterFormat) -> &'static str {
    match format {
        RasterFormat::Png => "image/png",
        RasterFormat::Jpeg => "image/jpeg",
        RasterFormat::WebP => "image/webp",
        RasterFormat::Tiff => "image/tiff",
        RasterFormat::Bmp => "image/bmp",
        RasterFormat::Gif => "image/gif",
    }
}

fn supports_editable(format: RasterFormat) -> bool {
    matches!(
        format,
        RasterFormat::Png | RasterFormat::Jpeg | RasterFormat::WebP
    )
}

fn read(path: &Path) -> Result<Vec<u8>, Failure> {
    std::fs::read(path).map_err(|e| Failure::Run(format!("{}: {e}", path.display())))
}

/// Output bytes are fully produced before this is called, so a failed operation
/// never leaves a partial file (and never clobbers an input given as output).
fn write(path: &Path, bytes: &[u8]) -> Result<(), Failure> {
    std::fs::write(path, bytes).map_err(|e| Failure::Run(format!("{}: {e}", path.display())))
}

fn to_raster(pixels: Vec<u8>, width: u32, height: u32) -> Result<RasterImage, Failure> {
    RasterImage::new(width, height, pixels).map_err(run_err)
}

/// Render `document` over its background, decoding the background resource bytes.
fn flatten(document: &Document, resources: &[ResourceData]) -> Result<RasterImage, Failure> {
    let decoded = match &document.background {
        Some(bg) => {
            let data = resources
                .iter()
                .find(|r| r.id == bg.resource)
                .ok_or_else(|| {
                    Failure::Run(format!("missing background resource {}", bg.resource))
                })?;
            Some((&bg.resource, decode(&data.bytes).map_err(run_err)?.image))
        }
        None => None,
    };
    let background = match &decoded {
        Some((id, image)) => Some(
            BackgroundImage::new(id, image.width(), image.height(), image.pixels())
                .map_err(run_err)?,
        ),
        None => None,
    };
    let rendered = render(document, background).map_err(run_err)?;
    let (width, height) = (rendered.width(), rendered.height());
    to_raster(rendered.into_pixels(), width, height)
}

fn annotate(args: &Args) -> Result<(), Failure> {
    if !supports_editable(args.format) {
        return Err(Failure::Run(
            "editable output must be PNG, JPEG or WebP (use `render` for a flat image)".into(),
        ));
    }
    let image_bytes = read(&args.inputs[0])?;
    if extract_editable(&image_bytes).map_err(run_err)?.is_some() {
        return Err(Failure::Run(format!(
            "{} already contains an editable document",
            args.inputs[0].display()
        )));
    }
    let source_format = decode(&image_bytes).map_err(run_err)?.format;
    let json = std::fs::read_to_string(&args.inputs[1])
        .map_err(|e| Failure::Run(format!("{}: {e}", args.inputs[1].display())))?;
    let mut document = Document::from_json(&json).map_err(run_err)?;

    // The image is the background. A document may declare it (one resource, used
    // by `background`) or leave it out, in which case it is added here.
    let id = match &document.background {
        Some(bg) => {
            if document.resources.len() != 1 || document.resources[0].id != bg.resource {
                return Err(Failure::Run(
                    "document must declare exactly one resource: its background".into(),
                ));
            }
            bg.resource.clone()
        }
        None => {
            if !document.resources.is_empty() {
                return Err(Failure::Run(
                    "document without a background must not declare resources".into(),
                ));
            }
            let id = ResourceId::from("background");
            document.background = Some(Background {
                resource: id.clone(),
                origin: Point::default(),
            });
            document.resources.push(Resource {
                id: id.clone(),
                media_type: media_type(source_format).into(),
            });
            id
        }
    };
    let editable = EditableDocument::new(
        document,
        vec![ResourceData {
            id,
            bytes: image_bytes,
        }],
    )
    .map_err(run_err)?;
    let preview = flatten(&editable.document, &editable.resources)?;
    let bytes = encode_editable(&preview, args.format, &editable).map_err(run_err)?;
    write(&args.output, &bytes)
}

fn render_cmd(args: &Args) -> Result<(), Failure> {
    let bytes = read(&args.inputs[0])?;
    let editable = extract_editable(&bytes).map_err(run_err)?.ok_or_else(|| {
        Failure::Run(format!(
            "{} has no editable document (use `convert` for plain images)",
            args.inputs[0].display()
        ))
    })?;
    let flat = flatten(&editable.document, &editable.resources)?;
    write(&args.output, &encode(&flat, args.format).map_err(run_err)?)
}

fn convert(args: &Args) -> Result<(), Failure> {
    let opened = open(&read(&args.inputs[0])?).map_err(run_err)?;
    let bytes = match &opened.editable {
        Some(editable) if !args.flat && supports_editable(args.format) => {
            encode_editable(&opened.raster.image, args.format, editable).map_err(run_err)?
        }
        Some(_) if !args.flat => {
            eprintln!(
                "scribblecap: warning: target format cannot hold the editable document; \
                 writing a flat image"
            );
            encode(&opened.raster.image, args.format).map_err(run_err)?
        }
        _ => encode(&opened.raster.image, args.format).map_err(run_err)?,
    };
    write(&args.output, &bytes)
}
