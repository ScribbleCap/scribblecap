//! Element geometry: turns document elements into [`Path`](crate::path::Path)s.

mod arrow;
mod freehand;
mod primitives;
mod text;

pub use arrow::arrow_outline;
pub use freehand::{PointFilter, filter_points, freehand_path};
pub use primitives::{PaintStyle, PaintedPath, UnsupportedShape, shape_paths};
pub use text::{TextLayout, layout_text};
