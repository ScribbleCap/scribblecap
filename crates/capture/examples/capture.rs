//! Try the capture backend from a terminal.
//!
//! ```sh
//! cargo run -p scribblecap-capture --example capture -- list
//! cargo run -p scribblecap-capture --example capture -- displays
//! cargo run -p scribblecap-capture --example capture -- screen [display-id]
//! cargo run -p scribblecap-capture --example capture -- window <window-id>
//! cargo run -p scribblecap-capture --example capture -- region <x> <y> <width> <height>
//! ```
//!
//! Images are written to `target/capture/<what>.png`. On macOS the terminal app needs
//! Screen Recording permission (System Settings → Privacy & Security → Screen & System
//! Audio Recording); the first run asks for it.

use std::process::ExitCode;

use scribblecap_capture::{CapturedImage, ScreenRect, backend};
use scribblecap_formats::{RasterFormat, RasterImage, encode};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let capturer = backend()?;
    if !capturer.has_permission() && !capturer.request_permission() {
        return Err(
            "screen capture permission not granted: allow your terminal in System \
                    Settings → Privacy & Security → Screen & System Audio Recording, then \
                    restart the terminal"
                .into(),
        );
    }
    let num = |i: usize| -> Result<f64, Box<dyn std::error::Error>> {
        Ok(args.get(i).ok_or("missing number")?.parse()?)
    };
    let image = match args.first().map(String::as_str) {
        Some("list") => {
            for w in capturer.list_windows()? {
                let b = w.bounds;
                println!(
                    "{:>6}  {:<24} {:<40} {}×{} at {},{}",
                    w.id, w.app_name, w.title, b.width, b.height, b.x, b.y
                );
            }
            return Ok(());
        }
        Some("displays") => {
            for d in capturer.displays()? {
                let b = d.bounds;
                println!(
                    "{:>6}  {}×{} pt at {},{}  scale {}",
                    d.id, b.width, b.height, b.x, b.y, d.scale
                );
            }
            return Ok(());
        }
        Some("screen") => {
            let id = args.get(1).map(|s| s.parse()).transpose()?;
            save("screen", &capturer.capture_screen(id)?)?
        }
        Some("window") => {
            let id = args
                .get(1)
                .ok_or("missing window id (see `list`)")?
                .parse()?;
            save("window", &capturer.capture_window(id)?)?
        }
        Some("region") => {
            let rect = ScreenRect::new(num(1)?, num(2)?, num(3)?, num(4)?);
            save("region", &capturer.capture_region(rect)?)?
        }
        _ => {
            return Err(
                "usage: capture list|displays|screen [id]|window <id>|region x y w h".into(),
            );
        }
    };
    println!("{image}");
    Ok(())
}

fn save(name: &str, image: &CapturedImage) -> Result<String, Box<dyn std::error::Error>> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/capture");
    std::fs::create_dir_all(dir)?;
    let path = std::fs::canonicalize(dir)?.join(format!("{name}.png"));
    let raster = RasterImage::new(image.width, image.height, image.pixels.clone())?;
    std::fs::write(&path, encode(&raster, RasterFormat::Png)?)?;
    Ok(format!(
        "Wrote {} ({}×{} px, scale {})",
        path.display(),
        image.width,
        image.height,
        image.scale
    ))
}
