//! Vector paths: the common output of all element geometry, consumed by the flat
//! renderer, the UI and (later) PDF export.

use std::fmt::Write as _;

use crate::geom::Point;

/// One drawing command of a [`Path`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathEl {
    MoveTo(Point),
    LineTo(Point),
    /// Cubic Bézier: two control points, then the end point.
    CubicTo(Point, Point, Point),
    Close,
}

/// A sequence of subpaths in image pixels. Filled with the non-zero rule.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    els: Vec<PathEl>,
}

impl Path {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn move_to(&mut self, p: Point) -> &mut Self {
        self.els.push(PathEl::MoveTo(p));
        self
    }

    pub fn line_to(&mut self, p: Point) -> &mut Self {
        self.els.push(PathEl::LineTo(p));
        self
    }

    pub fn cubic_to(&mut self, c1: Point, c2: Point, p: Point) -> &mut Self {
        self.els.push(PathEl::CubicTo(c1, c2, p));
        self
    }

    pub fn close(&mut self) -> &mut Self {
        self.els.push(PathEl::Close);
        self
    }

    pub fn elements(&self) -> &[PathEl] {
        &self.els
    }

    pub fn is_empty(&self) -> bool {
        self.els.is_empty()
    }

    /// Every point of the path, control points included, in order.
    pub fn points(&self) -> impl Iterator<Item = Point> + '_ {
        self.els.iter().flat_map(|el| match *el {
            PathEl::MoveTo(p) | PathEl::LineTo(p) => vec![p],
            PathEl::CubicTo(c1, c2, p) => vec![c1, c2, p],
            PathEl::Close => vec![],
        })
    }

    /// The same path with `f` applied to every point.
    pub fn map_points(&self, f: impl Fn(Point) -> Point) -> Self {
        let els = self
            .els
            .iter()
            .map(|el| match *el {
                PathEl::MoveTo(p) => PathEl::MoveTo(f(p)),
                PathEl::LineTo(p) => PathEl::LineTo(f(p)),
                PathEl::CubicTo(c1, c2, p) => PathEl::CubicTo(f(c1), f(c2), f(p)),
                PathEl::Close => PathEl::Close,
            })
            .collect();
        Self { els }
    }

    /// SVG path data (`d` attribute), numbers rounded to 3 decimals.
    pub fn to_svg(&self) -> String {
        let mut d = String::new();
        let mut put = |cmd: &str, pts: &[Point]| {
            if !d.is_empty() {
                d.push(' ');
            }
            d.push_str(cmd);
            for p in pts {
                let _ = write!(d, " {} {}", num(p.x), num(p.y));
            }
        };
        for el in &self.els {
            match *el {
                PathEl::MoveTo(p) => put("M", &[p]),
                PathEl::LineTo(p) => put("L", &[p]),
                PathEl::CubicTo(c1, c2, p) => put("C", &[c1, c2, p]),
                PathEl::Close => put("Z", &[]),
            }
        }
        d
    }
}

/// Up to 3 decimals, no trailing zeros, no `-0`.
fn num(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_output() {
        let mut p = Path::new();
        p.move_to(Point::new(0.0, -0.0001))
            .line_to(Point::new(1.5, 2.0))
            .cubic_to(
                Point::new(1.0 / 3.0, 10.0),
                Point::new(-2.25, 0.1),
                Point::new(100.0, 7.0),
            )
            .close();
        assert_eq!(p.to_svg(), "M 0 0 L 1.5 2 C 0.333 10 -2.25 0.1 100 7 Z");
    }

    #[test]
    fn map_points_keeps_structure() {
        let mut p = Path::new();
        p.move_to(Point::new(1.0, 2.0))
            .line_to(Point::new(3.0, 4.0))
            .close();
        let q = p.map_points(|pt| Point::new(pt.x * 2.0, pt.y + 1.0));
        assert_eq!(q.to_svg(), "M 2 3 L 6 5 Z");
        assert_eq!(q.points().count(), 2);
    }
}
