//! Writes an SVG sheet of the core geometry for checking by eye.
//!
//! `cargo run -p scribblecap-core --example preview [out.svg]`
//! (default `target/preview/geometry.svg`), then open the file in a browser.

use std::fmt::Write as _;

use scribblecap_core::geometry::arrow_outline;
use scribblecap_core::{Color, Path, Point};

const SIZES: [f64; 5] = [4.0, 8.0, 12.0, 16.0, 20.0];
const RED: Color = Color::rgb(0xFF, 0x00, 0x00);
const BLUE: Color = Color::rgb(0x19, 0x77, 0xFF);

struct Sheet {
    body: String,
    y: f64,
}

impl Sheet {
    fn label(&mut self, x: f64, y: f64, text: &str) {
        let _ = writeln!(
            self.body,
            r##"<text x="{x}" y="{y}" font-family="sans-serif" font-size="12" fill="#555">{text}</text>"##
        );
    }

    fn fill(&mut self, path: &Path, color: Color) {
        let hex = color.to_string();
        let _ = writeln!(
            self.body,
            r#"<path d="{}" fill="{}"/>"#,
            path.to_svg(),
            &hex[..7]
        );
    }
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/preview/geometry.svg"
        )
        .into()
    });
    let mut sheet = Sheet {
        body: String::new(),
        y: 30.0,
    };

    sheet.label(
        20.0,
        sheet.y,
        "Arrows: short (40 px), normal (120 px), long (600 px) at the five sizes; then both heads",
    );
    sheet.y += 30.0;
    for (len, both) in [(40.0, false), (120.0, false), (600.0, false), (120.0, true)] {
        for size in SIZES {
            let y = sheet.y;
            sheet.label(20.0, y + 4.0, &format!("{len} px · size {size}"));
            let path = arrow_outline(Point::new(140.0, y), Point::new(140.0 + len, y), size, both);
            sheet.fill(&path, RED);
            sheet.y += 44.0;
        }
        sheet.y += 10.0;
    }

    sheet.label(20.0, sheet.y, "Directions (every 30°), size 12");
    let center = Point::new(400.0, sheet.y + 170.0);
    for step in 0..12 {
        let a = f64::from(step) * std::f64::consts::PI / 6.0;
        let end = Point::new(center.x + 140.0 * a.cos(), center.y + 140.0 * a.sin());
        let start = Point::new(center.x + 20.0 * a.cos(), center.y + 20.0 * a.sin());
        sheet.fill(&arrow_outline(start, end, 12.0, false), BLUE);
    }
    sheet.y += 340.0;

    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="800" height="{h}" viewBox="0 0 800 {h}">
<rect width="100%" height="100%" fill="#fff"/>
{body}</svg>
"##,
        h = sheet.y,
        body = sheet.body
    );
    if let Some(dir) = std::path::Path::new(&out).parent() {
        std::fs::create_dir_all(dir).expect("create output folder");
    }
    std::fs::write(&out, svg).expect("write preview");
    println!("{out}");
}
