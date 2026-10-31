//! Original shared shape and badge geometry; no fonts or platform drawing APIs.

use crate::{Color, Path, Point, Rect, Shape, Stamp};

use super::{arrow_outline, freehand_path};

/// Painting instructions for both the live UI and the flat renderer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PaintStyle {
    Fill(Color),
    /// A centered stroke. `round` selects round caps/joins; otherwise butt caps
    /// and miter joins (limit 4). All paths use source-over blending.
    Stroke {
        color: Color,
        width: f64,
        round: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct PaintedPath {
    pub path: Path,
    pub style: PaintStyle,
}

/// Geometry that cannot be laid out with the bundled fonts or within limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnsupportedShape(pub &'static str);

impl std::fmt::Display for UnsupportedShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for UnsupportedShape {}

/// Convert a validated shape to reusable image-pixel paths in paint order.
/// Zero-area shapes and zero-length arrows have no painted paths. Marker and
/// highlighter use their stored width/alpha; fitting is performed only once.
pub fn shape_paths(shape: &Shape) -> Result<Vec<PaintedPath>, UnsupportedShape> {
    let mut output = Vec::new();
    match shape {
        Shape::Arrow(a) => fill(
            &mut output,
            arrow_outline(a.start, a.end, a.width, a.head_at_start),
            a.color,
        ),
        Shape::Line(l) => {
            if l.start == l.end {
                fill(&mut output, circle(l.start, l.width / 2.0), l.color);
            } else {
                let mut path = Path::new();
                path.move_to(l.start).line_to(l.end);
                stroke(&mut output, path, l.color, l.width, true);
            }
        }
        Shape::Rectangle(r) => {
            if r.rect.width == 0.0 || r.rect.height == 0.0 {
                return Ok(output);
            }
            let path = rounded_rect(r.rect, r.corner_radius);
            if let Some(color) = r.fill {
                fill(&mut output, path.clone(), color);
            }
            stroke(&mut output, path, r.stroke, r.width, false);
        }
        Shape::Ellipse(e) => {
            if e.rect.width == 0.0 || e.rect.height == 0.0 {
                return Ok(output);
            }
            let path = ellipse(e.rect);
            if let Some(color) = e.fill {
                fill(&mut output, path.clone(), color);
            }
            stroke(&mut output, path, e.stroke, e.width, false);
        }
        Shape::Freehand(f) => {
            if let Some(&first) = f.points.first() {
                if f.points.iter().all(|p| *p == first) {
                    fill(&mut output, circle(first, f.width / 2.0), f.color);
                } else {
                    stroke(
                        &mut output,
                        freehand_path(&f.points, f.width),
                        f.color,
                        f.width,
                        true,
                    );
                }
            }
        }
        Shape::Stamp(stamp) => output = stamp_paths(stamp),
        Shape::Text(text) => return super::text::text_paths(text),
        Shape::Step(step) => return super::text::step_paths(step),
    }
    Ok(output)
}

pub(super) fn fill(output: &mut Vec<PaintedPath>, path: Path, color: Color) {
    if !path.is_empty() {
        output.push(PaintedPath {
            path,
            style: PaintStyle::Fill(color),
        });
    }
}

pub(super) fn stroke(
    output: &mut Vec<PaintedPath>,
    path: Path,
    color: Color,
    width: f64,
    round: bool,
) {
    output.push(PaintedPath {
        path,
        style: PaintStyle::Stroke {
            color,
            width,
            round,
        },
    });
}

const K: f64 = 0.552_284_749_830_793_6;

pub(super) fn rounded_rect(r: Rect, radius: f64) -> Path {
    let (x, y, right, bottom) = (r.x, r.y, r.x + r.width, r.y + r.height);
    let a = radius.min(r.width / 2.0).min(r.height / 2.0);
    let mut p = Path::new();
    if a == 0.0 {
        p.move_to(Point::new(x, y))
            .line_to(Point::new(right, y))
            .line_to(Point::new(right, bottom))
            .line_to(Point::new(x, bottom))
            .close();
    } else {
        let c = a * K;
        p.move_to(Point::new(x + a, y))
            .line_to(Point::new(right - a, y))
            .cubic_to(
                Point::new(right - a + c, y),
                Point::new(right, y + a - c),
                Point::new(right, y + a),
            )
            .line_to(Point::new(right, bottom - a))
            .cubic_to(
                Point::new(right, bottom - a + c),
                Point::new(right - a + c, bottom),
                Point::new(right - a, bottom),
            )
            .line_to(Point::new(x + a, bottom))
            .cubic_to(
                Point::new(x + a - c, bottom),
                Point::new(x, bottom - a + c),
                Point::new(x, bottom - a),
            )
            .line_to(Point::new(x, y + a))
            .cubic_to(
                Point::new(x, y + a - c),
                Point::new(x + a - c, y),
                Point::new(x + a, y),
            )
            .close();
    }
    p
}

fn ellipse(r: Rect) -> Path {
    let (rx, ry) = (r.width / 2.0, r.height / 2.0);
    let (x, y) = (r.x + rx, r.y + ry);
    let (kx, ky) = (rx * K, ry * K);
    let mut p = Path::new();
    p.move_to(Point::new(x + rx, y))
        .cubic_to(
            Point::new(x + rx, y + ky),
            Point::new(x + kx, y + ry),
            Point::new(x, y + ry),
        )
        .cubic_to(
            Point::new(x - kx, y + ry),
            Point::new(x - rx, y + ky),
            Point::new(x - rx, y),
        )
        .cubic_to(
            Point::new(x - rx, y - ky),
            Point::new(x - kx, y - ry),
            Point::new(x, y - ry),
        )
        .cubic_to(
            Point::new(x + kx, y - ry),
            Point::new(x + rx, y - ky),
            Point::new(x + rx, y),
        )
        .close();
    p
}

pub(super) fn circle(center: Point, radius: f64) -> Path {
    ellipse(Rect::new(
        center.x - radius,
        center.y - radius,
        radius * 2.0,
        radius * 2.0,
    ))
}

fn polyline(points: &[(f64, f64)], closed: bool) -> Path {
    let mut p = Path::new();
    if let Some(&(x, y)) = points.first() {
        p.move_to(Point::new(x, y));
    }
    for &(x, y) in points.iter().skip(1) {
        p.line_to(Point::new(x, y));
    }
    if closed {
        p.close();
    }
    p
}

fn stamp_paths(stamp: &Stamp) -> Vec<PaintedPath> {
    let mut out = Vec::new();
    let color = match stamp.stamp.as_str() {
        "approved" => Color::rgb(28, 158, 89),
        "rejected" => Color::rgb(222, 57, 70),
        "question" => Color::rgb(31, 111, 235),
        "exclaim" => Color::rgb(230, 135, 23),
        "perfect" => Color::rgb(137, 80, 220),
        _ => Color::rgb(90, 102, 116),
    };
    if let Some(angle) = stamp.tail_angle {
        let (sin, cos) = sin_cos_degrees(angle);
        let tail = polyline(&[(0.65, -0.4), (1.5, 0.0), (0.65, 0.4)], true)
            .map_points(|p| Point::new(p.x * cos - p.y * sin, p.x * sin + p.y * cos));
        fill(&mut out, tail, color);
    }
    fill(&mut out, circle(Point::new(0.0, 0.0), 1.0), color);
    let white = Color::rgb(255, 255, 255);
    match stamp.stamp.as_str() {
        "approved" => stroke(
            &mut out,
            polyline(&[(-0.47, 0.0), (-0.12, 0.35), (0.48, -0.35)], false),
            white,
            0.16,
            true,
        ),
        "rejected" => {
            stroke(
                &mut out,
                polyline(&[(-0.34, -0.34), (0.34, 0.34)], false),
                white,
                0.16,
                true,
            );
            stroke(
                &mut out,
                polyline(&[(-0.34, 0.34), (0.34, -0.34)], false),
                white,
                0.16,
                true,
            );
        }
        "exclaim" => {
            stroke(
                &mut out,
                polyline(&[(0.0, -0.48), (0.0, 0.13)], false),
                white,
                0.17,
                true,
            );
            fill(&mut out, circle(Point::new(0.0, 0.46), 0.095), white);
        }
        "question" => {
            let mut p = Path::new();
            p.move_to(Point::new(-0.3, -0.25))
                .cubic_to(
                    Point::new(-0.3, -0.65),
                    Point::new(0.4, -0.65),
                    Point::new(0.33, -0.23),
                )
                .cubic_to(
                    Point::new(0.31, -0.08),
                    Point::new(0.0, -0.02),
                    Point::new(0.0, 0.15),
                );
            stroke(&mut out, p, white, 0.16, true);
            fill(&mut out, circle(Point::new(0.0, 0.46), 0.095), white);
        }
        "perfect" => fill(
            &mut out,
            polyline(
                &[
                    (0.0, -0.60),
                    (0.14, -0.19),
                    (0.57, -0.19),
                    (0.23, 0.07),
                    (0.35, 0.49),
                    (0.0, 0.24),
                    (-0.35, 0.49),
                    (-0.23, 0.07),
                    (-0.57, -0.19),
                    (-0.14, -0.19),
                ],
                true,
            ),
            white,
        ),
        _ => stroke(
            &mut out,
            polyline(
                &[(0.0, -0.43), (0.43, 0.0), (0.0, 0.43), (-0.43, 0.0)],
                true,
            ),
            white,
            0.14,
            true,
        ),
    }
    let radius = stamp.diameter / 2.0;
    for item in &mut out {
        item.path = item.path.map_points(|p| {
            Point::new(stamp.center.x + p.x * radius, stamp.center.y + p.y * radius)
        });
        if let PaintStyle::Stroke { width, .. } = &mut item.style {
            *width *= radius;
        }
    }
    out
}

// Fixed polynomial arithmetic avoids platform libm differences in stamp tails.
// Reduce to ±45 degrees, then evaluate Taylor polynomials (error < 1e-14).
fn sin_cos_degrees(angle: f64) -> (f64, f64) {
    let angle = angle.rem_euclid(360.0);
    let quadrant = (angle / 90.0).round() as i32;
    let x = (angle - f64::from(quadrant) * 90.0) * (std::f64::consts::PI / 180.0);
    let xx = x * x;
    let sin = x
        * (1.0
            + xx * (-1.0 / 6.0
                + xx * (1.0 / 120.0
                    + xx * (-1.0 / 5040.0
                        + xx * (1.0 / 362880.0
                            + xx * (-1.0 / 39916800.0
                                + xx * (1.0 / 6227020800.0 + xx * (-1.0 / 1307674368000.0))))))));
    let cos = 1.0
        + xx * (-1.0 / 2.0
            + xx * (1.0 / 24.0
                + xx * (-1.0 / 720.0
                    + xx * (1.0 / 40320.0
                        + xx * (-1.0 / 3628800.0
                            + xx * (1.0 / 479001600.0 + xx * (-1.0 / 87178291200.0)))))));
    match quadrant.rem_euclid(4) {
        0 => (sin, cos),
        1 => (cos, -sin),
        2 => (-sin, -cos),
        _ => (-cos, sin),
    }
}
