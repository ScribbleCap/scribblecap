//! Element geometry: turns document elements into [`Path`](crate::path::Path)s.

mod arrow;
mod freehand;

pub use arrow::arrow_outline;
pub use freehand::{PointFilter, filter_points, freehand_path};
