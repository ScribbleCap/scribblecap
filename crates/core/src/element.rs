//! Annotation elements. Schema: `docs/file-format.md`.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::color::Color;
use crate::geom::{Point, Rect};

/// Unique id of an element within a document (the app uses UUIDs; any non-empty
/// string is valid).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ElementId(pub String);

impl fmt::Display for ElementId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for ElementId {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

/// One annotation. Elements are painted in document order (first is bottom-most).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Element {
    pub id: ElementId,
    #[serde(flatten)]
    pub shape: Shape,
}

/// What an element draws, tagged by `"kind"` in JSON.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Shape {
    Arrow(Arrow),
    Line(Line),
    Rectangle(Rectangle),
    Ellipse(Ellipse),
    Freehand(Freehand),
    Text(Text),
    Stamp(Stamp),
    Step(Step),
}

/// Filled tapered arrow from `start` to the tip at `end`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Arrow {
    pub start: Point,
    pub end: Point,
    pub color: Color,
    /// Arrow size in pixels (controls head and shaft width).
    pub width: f64,
    /// Also draw a head at `start`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub head_at_start: bool,
}

/// Straight stroked line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    pub start: Point,
    pub end: Point,
    pub color: Color,
    /// Stroke width in pixels.
    pub width: f64,
}

/// Rectangle; rounded when `corner_radius > 0`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rectangle {
    pub rect: Rect,
    pub stroke: Color,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Color>,
    /// Stroke width in pixels.
    pub width: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub corner_radius: f64,
}

/// Ellipse inscribed in `rect`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ellipse {
    pub rect: Rect,
    pub stroke: Color,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Color>,
    /// Stroke width in pixels.
    pub width: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FreehandTool {
    Marker,
    Highlighter,
}

/// Freehand stroke. `points` are the filtered input points; the smooth curve is
/// derived from them by the geometry code, not stored.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Freehand {
    pub tool: FreehandTool,
    pub color: Color,
    /// Stroke width in pixels.
    pub width: f64,
    pub points: Vec<Point>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextStyle {
    /// Text with a contrasting outline.
    #[default]
    Plain,
    /// Text in a filled speech bubble.
    Bubble,
    /// Text attached to another element (see [`Text::attached_to`]).
    Label,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// A block of text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Text {
    /// Top-left corner of the text box.
    pub origin: Point,
    pub text: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub style: TextStyle,
    pub color: Color,
    /// Font size in pixels.
    pub font_size: f64,
    /// Font family; the app's default sans-serif font when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    /// Wrap lines at this width in pixels; no wrapping when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap_width: Option<f64>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub align: TextAlign,
    /// For `label` style: the element this label belongs to (moves and deletes with it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attached_to: Option<ElementId>,
}

/// A round badge from the app's stamp set.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stamp {
    pub center: Point,
    /// Stamp identifier, e.g. `"approved"`. Unknown names render as a generic stamp.
    pub stamp: String,
    pub diameter: f64,
    /// Direction of the pointer tail in degrees (0 = right, 90 = down); no tail when
    /// absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tail_angle: Option<f64>,
}

/// A numbered circle (1, 2, 3…).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub center: Point,
    /// Starts at 1.
    pub number: u32,
    pub color: Color,
    pub diameter: f64,
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}
