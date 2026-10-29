//! The editable document and its JSON form. Schema: `docs/file-format.md`.

use std::collections::{HashMap, HashSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::DOCUMENT_FORMAT;
use crate::element::{Element, ElementId, Shape};
use crate::geom::{Point, Size, is_non_negative, is_positive};

/// Id of a binary resource (image) stored next to the document by the container
/// format.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ResourceId(pub String);

impl fmt::Display for ResourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for ResourceId {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

/// A binary resource listed by the document. The bytes live in the container.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resource {
    pub id: ResourceId,
    /// MIME type, e.g. `image/png`.
    pub media_type: String,
}

/// The un-annotated image under the annotations.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Background {
    pub resource: ResourceId,
    /// Where the image's top-left corner sits on the canvas (non-zero after the canvas
    /// was expanded).
    #[serde(default, skip_serializing_if = "is_origin")]
    pub origin: Point,
}

/// An editable ScribbleCap document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    /// Canvas size in pixels; the flat export has exactly this size.
    pub canvas: Size,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Background>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<Resource>,
    /// Painted in order: first is bottom-most.
    #[serde(default)]
    pub elements: Vec<Element>,
}

/// Wire form: adds the `format` key in front of the document's own keys.
#[derive(Serialize)]
struct Wire<'a> {
    format: &'static str,
    #[serde(flatten)]
    document: &'a Document,
}

impl Document {
    /// An empty document with no background.
    pub fn new(canvas: Size) -> Self {
        Self {
            canvas,
            background: None,
            resources: Vec::new(),
            elements: Vec::new(),
        }
    }

    /// Parse and validate a document.
    pub fn from_json(json: &str) -> Result<Self, LoadError> {
        let value: serde_json::Value = serde_json::from_str(json).map_err(LoadError::Json)?;
        match value.get("format") {
            Some(serde_json::Value::String(f)) if f == DOCUMENT_FORMAT => {}
            Some(serde_json::Value::String(f)) if f.starts_with("scribblecap/") => {
                return Err(LoadError::UnsupportedFormat(f.clone()));
            }
            _ => return Err(LoadError::NotADocument),
        }
        let document = Self::deserialize(value).map_err(LoadError::Json)?;
        document.validate().map_err(LoadError::Invalid)?;
        Ok(document)
    }

    /// Validate and serialise as compact JSON. An invalid document is never written.
    pub fn to_json(&self) -> Result<String, ValidationError> {
        self.validate()?;
        let wire = Wire {
            format: DOCUMENT_FORMAT,
            document: self,
        };
        // Serialising these types cannot fail: all map keys are strings.
        Ok(serde_json::to_string(&wire).expect("document serialises"))
    }

    /// Check the rules in `docs/file-format.md` that the JSON shape alone can't express.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if !self.canvas.is_positive() {
            return Err(ValidationError::InvalidCanvas);
        }

        let mut resources = HashSet::new();
        for r in &self.resources {
            if r.id.0.is_empty() || !resources.insert(&r.id) {
                return Err(ValidationError::DuplicateResource(r.id.clone()));
            }
        }
        if let Some(bg) = &self.background {
            if !resources.contains(&bg.resource) {
                return Err(ValidationError::UnknownResource(bg.resource.clone()));
            }
            if !bg.origin.is_finite() {
                return Err(ValidationError::InvalidBackground);
            }
        }

        let mut kinds = HashMap::new();
        for e in &self.elements {
            if e.id.0.is_empty() {
                return Err(ValidationError::EmptyId);
            }
            if kinds.insert(&e.id, &e.shape).is_some() {
                return Err(ValidationError::DuplicateId(e.id.clone()));
            }
            if let Some(field) = invalid_field(&e.shape) {
                return Err(ValidationError::InvalidValue {
                    element: e.id.clone(),
                    field,
                });
            }
        }

        for e in &self.elements {
            if let Shape::Text(t) = &e.shape {
                let Some(target) = &t.attached_to else {
                    continue;
                };
                let ok = t.style == crate::element::TextStyle::Label
                    && target != &e.id
                    && kinds
                        .get(target)
                        .is_some_and(|s| !matches!(s, Shape::Text(_)));
                if !ok {
                    return Err(ValidationError::InvalidAttachment {
                        element: e.id.clone(),
                        target: target.clone(),
                    });
                }
            }
        }
        Ok(())
    }
}

/// The first field of `shape` that breaks a numeric rule, if any.
fn invalid_field(shape: &Shape) -> Option<&'static str> {
    let check = |ok: bool, field| (!ok).then_some(field);
    match shape {
        Shape::Arrow(a) => check(a.start.is_finite(), "start")
            .or(check(a.end.is_finite(), "end"))
            .or(check(is_positive(a.width), "width")),
        Shape::Line(l) => check(l.start.is_finite(), "start")
            .or(check(l.end.is_finite(), "end"))
            .or(check(is_positive(l.width), "width")),
        Shape::Rectangle(r) => check(r.rect.is_valid(), "rect")
            .or(check(is_positive(r.width), "width"))
            .or(check(is_non_negative(r.corner_radius), "cornerRadius")),
        Shape::Ellipse(e) => {
            check(e.rect.is_valid(), "rect").or(check(is_positive(e.width), "width"))
        }
        Shape::Freehand(f) => check(is_positive(f.width), "width").or(check(
            !f.points.is_empty() && f.points.iter().all(|p| p.is_finite()),
            "points",
        )),
        Shape::Text(t) => check(t.origin.is_finite(), "origin")
            .or(check(is_positive(t.font_size), "fontSize"))
            .or(check(t.wrap_width.is_none_or(is_positive), "wrapWidth")),
        Shape::Stamp(s) => check(s.center.is_finite(), "center")
            .or(check(!s.stamp.is_empty(), "stamp"))
            .or(check(is_positive(s.diameter), "diameter"))
            .or(check(s.tail_angle.is_none_or(f64::is_finite), "tailAngle")),
        Shape::Step(s) => check(s.center.is_finite(), "center")
            .or(check(s.number >= 1, "number"))
            .or(check(is_positive(s.diameter), "diameter")),
    }
}

fn is_origin(p: &Point) -> bool {
    *p == Point::default()
}

/// Why a document could not be loaded.
#[derive(Debug)]
pub enum LoadError {
    /// Not valid JSON, or JSON that doesn't match the schema.
    Json(serde_json::Error),
    /// No `"format": "scribblecap/…"` key: not a ScribbleCap document.
    NotADocument,
    /// A ScribbleCap document from a newer (or unknown) version of the format.
    UnsupportedFormat(String),
    /// Well-formed but breaks a rule of the format.
    Invalid(ValidationError),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(e) => write!(f, "malformed document: {e}"),
            Self::NotADocument => f.write_str("not a ScribbleCap document"),
            Self::UnsupportedFormat(v) => write!(
                f,
                "document format {v:?} is not supported (this version reads {DOCUMENT_FORMAT:?})"
            ),
            Self::Invalid(e) => write!(f, "invalid document: {e}"),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(e) => Some(e),
            Self::Invalid(e) => Some(e),
            _ => None,
        }
    }
}

/// A rule of the format that a document breaks.
#[derive(Clone, Debug, PartialEq)]
pub enum ValidationError {
    InvalidCanvas,
    InvalidBackground,
    DuplicateResource(ResourceId),
    UnknownResource(ResourceId),
    EmptyId,
    DuplicateId(ElementId),
    InvalidValue {
        element: ElementId,
        field: &'static str,
    },
    InvalidAttachment {
        element: ElementId,
        target: ElementId,
    },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCanvas => f.write_str("canvas width and height must be positive"),
            Self::InvalidBackground => f.write_str("background origin must be finite"),
            Self::DuplicateResource(id) => write!(f, "resource id {id:?} is empty or repeated"),
            Self::UnknownResource(id) => write!(f, "resource {id:?} is not listed"),
            Self::EmptyId => f.write_str("element id is empty"),
            Self::DuplicateId(id) => write!(f, "element id {id:?} is repeated"),
            Self::InvalidValue { element, field } => {
                write!(f, "element {element:?}: invalid {field}")
            }
            Self::InvalidAttachment { element, target } => write!(
                f,
                "element {element:?}: only a label can be attached, to another non-text element (got {target:?})"
            ),
        }
    }
}

impl std::error::Error for ValidationError {}
