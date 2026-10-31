//! Versioned editable data inside otherwise ordinary image containers.
//!
//! ```
//! use scribblecap_core::{Document, Size};
//! use scribblecap_formats::{EditableDocument, RasterFormat, RasterImage, encode_editable, open};
//! let editable = EditableDocument::new(Document::new(Size::new(1.0, 1.0)), vec![]).unwrap();
//! let preview = RasterImage::new(1, 1, vec![255, 255, 255, 255]).unwrap();
//! let bytes = encode_editable(&preview, RasterFormat::Png, &editable).unwrap();
//! assert_eq!(open(&bytes).unwrap().editable, Some(editable));
//! ```

mod containers;
mod payload;

use std::collections::HashSet;
use std::fmt;

use scribblecap_core::{Document, LoadError, ResourceId, ValidationError};

use crate::{DecodedRaster, RasterError, RasterFormat, RasterImage};

pub const MAX_PAYLOAD_BYTES: usize = 256 * 1024 * 1024;
const MAX_JSON_BYTES: usize = 16 * 1024 * 1024;
const MAX_RESOURCES: usize = 4096;
const MAX_ID_BYTES: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceData {
    pub id: ResourceId,
    pub bytes: Vec<u8>,
}

/// An editable document plus exactly the resources declared in its inventory.
/// Resources are normalized to document order by `new` and by extraction.
#[derive(Clone, Debug, PartialEq)]
pub struct EditableDocument {
    pub document: Document,
    pub resources: Vec<ResourceData>,
}

impl EditableDocument {
    pub fn new(
        document: Document,
        mut resources: Vec<ResourceData>,
    ) -> Result<Self, EditableError> {
        validate_resources(&document, &resources)?;
        let order: std::collections::HashMap<_, _> = document
            .resources
            .iter()
            .enumerate()
            .map(|(i, r)| (&r.id, i))
            .collect();
        resources.sort_by_key(|r| order[&r.id]);
        Ok(Self {
            document,
            resources,
        })
    }
}

fn validate_resources(
    document: &Document,
    resources: &[ResourceData],
) -> Result<(), EditableError> {
    document.validate().map_err(EditableError::Validation)?;
    if resources.len() > MAX_RESOURCES {
        return Err(EditableError::Limits);
    }
    if resources.len() != document.resources.len() {
        return Err(EditableError::InvalidResources);
    }
    let inventory: HashSet<_> = document.resources.iter().map(|r| &r.id).collect();
    let mut seen = HashSet::new();
    for resource in resources {
        if resource.id.0.len() > MAX_ID_BYTES || resource.bytes.len() > MAX_PAYLOAD_BYTES {
            return Err(EditableError::Limits);
        }
        if resource.bytes.is_empty()
            || !inventory.contains(&resource.id)
            || !seen.insert(&resource.id)
        {
            return Err(EditableError::InvalidResources);
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
pub struct OpenedImage {
    /// Flattened preview. Re-edit using the background resource, not this image.
    pub raster: DecodedRaster,
    pub editable: Option<EditableDocument>,
}

#[derive(Debug)]
pub enum EditableError {
    UnsupportedFormat,
    InvalidContainer(&'static str),
    InvalidPayload(&'static str),
    InvalidResources,
    UnsupportedVersion(u32),
    PreviewDimensions,
    PreviewChanged,
    Limits,
    Document(LoadError),
    Validation(ValidationError),
    Raster(RasterError),
}

impl fmt::Display for EditableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedFormat => f.write_str("editable saving supports PNG, JPEG and WebP only"),
            Self::InvalidContainer(reason) => write!(f, "invalid image container: {reason}"),
            Self::InvalidPayload(reason) => write!(f, "damaged editable payload: {reason}"),
            Self::InvalidResources => f.write_str("resource data must match the document inventory exactly, with nonempty data and no duplicates"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported editable container version {v}; preserve the original file"),
            Self::PreviewDimensions => f.write_str("preview dimensions do not match the document canvas"),
            Self::PreviewChanged => f.write_str("flattened image changed without updating editable data; preserve the original file"),
            Self::Limits => f.write_str("editable document exceeds size/count limits or available memory"),
            Self::Document(e) => write!(f, "embedded document: {e}"),
            Self::Validation(e) => write!(f, "invalid editable document: {e}"),
            Self::Raster(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for EditableError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Document(e) => Some(e),
            Self::Validation(e) => Some(e),
            Self::Raster(e) => Some(e),
            _ => None,
        }
    }
}

/// Save a fresh flat preview plus a current editable payload. Old container bytes
/// are never reused, so previous hidden resources cannot accumulate on re-save.
/// The caller supplies an up-to-date preview; this API does not render annotations.
pub fn encode_editable(
    preview: &RasterImage,
    format: RasterFormat,
    editable: &EditableDocument,
) -> Result<Vec<u8>, EditableError> {
    if !matches!(
        format,
        RasterFormat::Png | RasterFormat::Jpeg | RasterFormat::WebP
    ) {
        return Err(EditableError::UnsupportedFormat);
    }
    validate_dimensions(&editable.document, preview.width(), preview.height())?;
    validate_resources(&editable.document, &editable.resources)?;
    let flat = crate::encode(preview, format).map_err(EditableError::Raster)?;
    let flat = containers::prepare(flat, format, preview)?;
    let payload = payload::encode(editable, crc32fast::hash(&flat))?;
    containers::embed(flat, format, &payload)
}

fn validate_dimensions(document: &Document, width: u32, height: u32) -> Result<(), EditableError> {
    if document.canvas.width.ceil() != f64::from(width)
        || document.canvas.height.ceil() != f64::from(height)
    {
        return Err(EditableError::PreviewDimensions);
    }
    Ok(())
}

/// Extract metadata without decoding the preview pixels. Recognized damaged or
/// unsupported payloads are errors, not silently downgraded to flat images.
pub fn extract_editable(bytes: &[u8]) -> Result<Option<EditableDocument>, EditableError> {
    let extracted = containers::extract(bytes)?;
    let Some(payload) = extracted.payload else {
        return Ok(None);
    };
    let (editable, fingerprint) = payload::decode(payload)?;
    if fingerprint != extracted.fingerprint {
        return Err(EditableError::PreviewChanged);
    }
    if let Some((w, h)) = extracted.dimensions {
        validate_dimensions(&editable.document, w, h)?;
    }
    Ok(Some(editable))
}

/// Open both ordinary pixels and any embedded editable data. Callers can use the
/// separate flat decoder for an explicit recovery view after a metadata error,
/// but should never overwrite the original as if it were an ordinary flat file.
pub fn open(bytes: &[u8]) -> Result<OpenedImage, EditableError> {
    let editable = extract_editable(bytes)?;
    let raster = crate::decode(bytes).map_err(EditableError::Raster)?;
    if let Some(editable) = &editable {
        validate_dimensions(
            &editable.document,
            raster.image.width(),
            raster.image.height(),
        )?;
    }
    Ok(OpenedImage { raster, editable })
}
