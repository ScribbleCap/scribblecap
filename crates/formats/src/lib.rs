//! Platform-independent raster decoding and flat export.
//!
//! [`decode`] detects the format from file bytes and applies EXIF orientation.
//! [`encode`] produces a normal flat image, without source metadata or embedded
//! editable documents. Editable container support follows in roadmap M1-8.

mod raster;

pub use raster::{DecodedRaster, RasterError, RasterFormat, RasterImage, decode, encode};
