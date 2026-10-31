//! Platform-independent raster decoding and flat export.
//!
//! [`decode`] detects the format from file bytes and applies EXIF orientation.
//! [`encode`] produces a normal flat image, without source metadata or embedded
//! editable documents. [`encode_editable`] writes the preview plus a versioned
//! document/resources payload in PNG, JPEG or WebP; [`open`] restores both.

mod editable;
mod raster;

pub use editable::{
    EditableDocument, EditableError, OpenedImage, ResourceData, encode_editable, extract_editable,
    open,
};
pub use raster::{DecodedRaster, RasterError, RasterFormat, RasterImage, decode, encode};
