//! ScribbleCap core: document model, geometry and flat rendering.
//!
//! Must stay free of UI and OS APIs (see `AGENTS.md`). The document format is specified
//! in `docs/file-format.md`.

pub mod color;
pub mod document;
pub mod element;
pub mod geom;
pub mod geometry;
pub mod path;
pub mod render;

pub use color::Color;
pub use document::{Background, Document, LoadError, Resource, ResourceId, ValidationError};
pub use element::{
    Arrow, Element, ElementId, Ellipse, Freehand, FreehandTool, Line, Rectangle, Shape, Stamp,
    Step, Text, TextAlign, TextStyle,
};
pub use geom::{Point, Rect, Size};
pub use path::{Path, PathEl};

/// Value of the top-level `"format"` key in a saved ScribbleCap document
/// (decision 0003).
pub const DOCUMENT_FORMAT: &str = "scribblecap/1";
