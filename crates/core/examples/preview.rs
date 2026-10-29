//! Writes an SVG sheet of the core geometry for checking by eye.
//!
//! `cargo run -p scribblecap-core --example preview [out.svg]`
//! (default `target/preview/geometry.svg`), then open the file in a browser.

use std::fmt::Write as _;

use scribblecap_core::geometry::{arrow_outline, filter_points, freehand_path};
use scribblecap_core::{Color, Path, Point};

const SIZES: [f64; 5] = [4.0, 8.0, 12.0, 16.0, 20.0];
const RED: Color = Color::rgb(0xFF, 0x00, 0x00);
const BLUE: Color = Color::rgb(0x19, 0x77, 0xFF);
const GREY: Color = Color::rgb(0x88, 0x88, 0x88);
const YELLOW: Color = Color::rgba(0xFF, 0xF7, 0x00, 0x66);

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

    fn stroke(&mut self, path: &Path, color: Color, width: f64) {
        let hex = color.to_string();
        let _ = writeln!(
            self.body,
            r#"<path d="{}" fill="none" stroke="{}" stroke-opacity="{:.2}" stroke-width="{width}" stroke-linecap="round" stroke-linejoin="round"/>"#,
            path.to_svg(),
            &hex[..7],
            f64::from(color.a) / 255.0
        );
    }
}

/// Deterministic jitter in [-amp, amp].
fn jitter(seed: &mut u64, amp: f64) -> f64 {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*seed >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0) * amp
}

/// Raw pointer input for the freehand demos, relative to (0, 0), about 340 × 120.
fn scribbles() -> Vec<(&'static str, Vec<Point>, Color, f64)> {
    let mut seed = 5;
    let mut wave = Vec::new();
    for i in 0..170 {
        let x = f64::from(i) * 2.0;
        wave.push(Point::new(
            x + jitter(&mut seed, 0.8),
            60.0 + 40.0 * (x / 40.0).sin() + jitter(&mut seed, 1.2),
        ));
    }
    let mut zigzag = Vec::new();
    for i in 0..170 {
        let x = f64::from(i) * 2.0;
        let up = (i / 34) % 2 == 0;
        let y = if up {
            (i % 34) as f64 * 3.0
        } else {
            102.0 - (i % 34) as f64 * 3.0
        };
        zigzag.push(Point::new(
            x + jitter(&mut seed, 0.6),
            10.0 + y + jitter(&mut seed, 0.6),
        ));
    }
    let mut lasso = Vec::new();
    for i in 0..=150 {
        let a = f64::from(i) / 150.0 * std::f64::consts::TAU;
        lasso.push(Point::new(
            170.0 + 150.0 * a.cos() + jitter(&mut seed, 1.0),
            60.0 + 50.0 * a.sin() + jitter(&mut seed, 1.0),
        ));
    }
    vec![
        ("marker, size 6: wave", wave, RED, 6.0),
        ("marker, size 6: zig-zag", zigzag, BLUE, 6.0),
        ("highlighter, size 24: loop", lasso, YELLOW, 24.0),
    ]
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

    sheet.label(
        20.0,
        sheet.y,
        "Freehand: raw input (left), filtered + fitted (right; thin line = filtered points)",
    );
    sheet.y += 20.0;
    for (name, raw, color, width) in scribbles() {
        sheet.label(20.0, sheet.y + 12.0, name);
        let at = |dx: f64, dy: f64| move |p: Point| Point::new(p.x + dx, p.y + dy);
        let y = sheet.y + 30.0;
        let mut polyline = Path::new();
        polyline.move_to(raw[0]);
        raw[1..].iter().for_each(|&p| {
            polyline.line_to(p);
        });
        sheet.stroke(&polyline.map_points(at(20.0, y)), color, width);

        let filtered = filter_points(&raw);
        let fitted = freehand_path(&filtered, width);
        sheet.stroke(&fitted.map_points(at(420.0, y)), color, width);
        let mut points = Path::new();
        points.move_to(filtered[0]);
        filtered[1..].iter().for_each(|&p| {
            points.line_to(p);
        });
        sheet.stroke(&points.map_points(at(420.0, y)), GREY, 0.5);
        sheet.y += 180.0;
    }

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
    let shown = std::fs::canonicalize(&out).map_or(out, |p| p.display().to_string());
    println!("Wrote {shown}");
}
