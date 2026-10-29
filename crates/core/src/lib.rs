//! ScribbleCap core: document model, geometry and flat rendering.
//!
//! Must stay free of UI and OS APIs (see `AGENTS.md`). Filled in by roadmap M1.

/// Value of the top-level `"format"` key in a saved ScribbleCap document
/// (decision 0003).
pub const DOCUMENT_FORMAT: &str = "scribblecap/1";
