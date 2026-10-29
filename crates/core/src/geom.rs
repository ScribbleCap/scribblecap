//! Plain geometry values used by the document model.

use serde::{Deserialize, Serialize};

/// A point in image pixels. Serialised as `[x, y]`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(from = "[f64; 2]", into = "[f64; 2]")]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub(crate) fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

impl From<[f64; 2]> for Point {
    fn from([x, y]: [f64; 2]) -> Self {
        Self { x, y }
    }
}

impl From<Point> for [f64; 2] {
    fn from(p: Point) -> Self {
        [p.x, p.y]
    }
}

/// A size in image pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

impl Size {
    pub const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }

    pub(crate) fn is_positive(self) -> bool {
        is_positive(self.width) && is_positive(self.height)
    }
}

/// An axis-aligned rectangle in image pixels; `x`, `y` is the top-left corner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Finite origin and a size that is finite and not negative.
    pub(crate) fn is_valid(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && is_non_negative(self.width)
            && is_non_negative(self.height)
    }
}

pub(crate) fn is_positive(v: f64) -> bool {
    v.is_finite() && v > 0.0
}

pub(crate) fn is_non_negative(v: f64) -> bool {
    v.is_finite() && v >= 0.0
}
