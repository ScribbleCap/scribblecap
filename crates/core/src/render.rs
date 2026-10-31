//! CPU-only flat rendering. Output is straight RGBA8 for format encoders.
//!
//! ```
//! use scribblecap_core::{Document, Size};
//! use scribblecap_core::render::render;
//! let image = render(&Document::new(Size::new(2.0, 1.0)), None).unwrap();
//! assert_eq!(image.pixels(), &[0; 8]);
//! ```

use std::fmt;

use crate::geometry::{PaintStyle, shape_paths};
use crate::{Document, ElementId, Path, PathEl, ResourceId, ValidationError};

/// Per-buffer budget (256 MiB), checked before allocating output or a background.
pub const MAX_RENDER_BYTES: u64 = 256 * 1024 * 1024;

/// Flat pixels in top-left-origin, row-major RGBA8 with straight alpha.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedImage {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl RenderedImage {
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
    pub fn into_pixels(self) -> Vec<u8> {
        self.pixels
    }
}

/// Borrowed, already-decoded straight RGBA8 background, at its native pixel size.
#[derive(Clone, Copy, Debug)]
pub struct BackgroundImage<'a> {
    resource: &'a ResourceId,
    width: u32,
    height: u32,
    pixels: &'a [u8],
}

impl<'a> BackgroundImage<'a> {
    pub fn new(
        resource: &'a ResourceId,
        width: u32,
        height: u32,
        pixels: &'a [u8],
    ) -> Result<Self, RenderError> {
        let length = buffer_length(width, height).ok_or(RenderError::InvalidBackground)?;
        if length != pixels.len() {
            return Err(RenderError::InvalidBackground);
        }
        Ok(Self {
            resource,
            width,
            height,
            pixels,
        })
    }
}

#[derive(Debug)]
pub enum RenderError {
    InvalidDocument(ValidationError),
    InvalidCanvas,
    InvalidBackground,
    UnexpectedBackground,
    MissingBackground(ResourceId),
    UnsupportedElement {
        element: ElementId,
        kind: &'static str,
    },
    InvalidGeometry(ElementId),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDocument(e) => write!(f, "invalid document: {e}"),
            Self::InvalidCanvas => {
                f.write_str("canvas dimensions exceed the renderer's pixel/buffer limits")
            }
            Self::InvalidBackground => {
                f.write_str("background must be nonzero RGBA8 within the render budget")
            }
            Self::UnexpectedBackground => {
                f.write_str("background pixels supplied for a document without a background")
            }
            Self::MissingBackground(id) => write!(f, "decoded pixels missing for background {id}"),
            Self::UnsupportedElement { element, kind } => write!(f, "element {element}: {kind}"),
            Self::InvalidGeometry(id) => write!(
                f,
                "element {id}: geometry cannot be represented by the renderer"
            ),
        }
    }
}

impl std::error::Error for RenderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidDocument(e) => Some(e),
            _ => None,
        }
    }
}

fn buffer_length(width: u32, height: u32) -> Option<usize> {
    let length = u64::from(width)
        .checked_mul(u64::from(height))?
        .checked_mul(4)?;
    if width == 0
        || height == 0
        || width > i32::MAX as u32 / 4
        || height > i32::MAX as u32
        || length > MAX_RENDER_BYTES
    {
        return None;
    }
    usize::try_from(length).ok()
}

fn dimension(value: f64) -> Option<u32> {
    let value = value.ceil();
    if value > 0.0 && value <= f64::from(u32::MAX) {
        Some(value as u32)
    } else {
        None
    }
}

/// Render supported elements, bottom to top, over an optional decoded background.
/// Empty space is transparent; paths are antialiased and clipped to the canvas.
/// Fractional canvas dimensions round up. Missing backgrounds, invalid geometry,
/// and unavailable glyphs return errors; no incomplete output is returned.
pub fn render(
    document: &Document,
    background: Option<BackgroundImage<'_>>,
) -> Result<RenderedImage, RenderError> {
    document.validate().map_err(RenderError::InvalidDocument)?;
    let width = dimension(document.canvas.width).ok_or(RenderError::InvalidCanvas)?;
    let height = dimension(document.canvas.height).ok_or(RenderError::InvalidCanvas)?;
    let length = buffer_length(width, height).ok_or(RenderError::InvalidCanvas)?;
    match (&document.background, &background) {
        (Some(bg), Some(pixels)) if &bg.resource == pixels.resource => {}
        (Some(bg), _) => return Err(RenderError::MissingBackground(bg.resource.clone())),
        (None, Some(_)) => return Err(RenderError::UnexpectedBackground),
        (None, None) => {}
    }
    let size = tiny_skia::IntSize::from_wh(width, height).ok_or(RenderError::InvalidCanvas)?;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(length)
        .map_err(|_| RenderError::InvalidCanvas)?;
    pixels.resize(length, 0);
    let mut pixmap = tiny_skia::Pixmap::from_vec(pixels, size).ok_or(RenderError::InvalidCanvas)?;
    if let (Some(info), Some(background)) = (&document.background, background) {
        let x = coordinate(info.origin.x).ok_or(RenderError::InvalidBackground)?;
        let y = coordinate(info.origin.y).ok_or(RenderError::InvalidBackground)?;
        let size = tiny_skia::IntSize::from_wh(background.width, background.height)
            .ok_or(RenderError::InvalidBackground)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(background.pixels.len())
            .map_err(|_| RenderError::InvalidBackground)?;
        bytes.extend_from_slice(background.pixels);
        for pixel in bytes.as_chunks_mut::<4>().0 {
            let alpha = u16::from(pixel[3]);
            for channel in &mut pixel[..3] {
                *channel = ((u16::from(*channel) * alpha + 127) / 255) as u8;
            }
        }
        let background =
            tiny_skia::Pixmap::from_vec(bytes, size).ok_or(RenderError::InvalidBackground)?;
        pixmap.draw_pixmap(
            0,
            0,
            background.as_ref(),
            &tiny_skia::PixmapPaint::default(),
            tiny_skia::Transform::from_translate(x, y),
            None,
        );
    }
    for element in &document.elements {
        let paths = shape_paths(&element.shape).map_err(|e| RenderError::UnsupportedElement {
            element: element.id.clone(),
            kind: e.0,
        })?;
        if let crate::Shape::Arrow(arrow) = &element.shape
            && arrow.start != arrow.end
            && paths.is_empty()
        {
            return Err(RenderError::InvalidGeometry(element.id.clone()));
        }
        for item in paths {
            let Some(path) = skia_path(&item.path)
                .map_err(|()| RenderError::InvalidGeometry(element.id.clone()))?
            else {
                continue;
            };
            let (color, style) = match item.style {
                PaintStyle::Fill(color) => (color, None),
                PaintStyle::Stroke {
                    color,
                    width,
                    round,
                } => {
                    let width = coordinate(width)
                        .filter(|v| *v > 0.0)
                        .ok_or_else(|| RenderError::InvalidGeometry(element.id.clone()))?;
                    let stroke = tiny_skia::Stroke {
                        width,
                        line_cap: if round {
                            tiny_skia::LineCap::Round
                        } else {
                            tiny_skia::LineCap::Butt
                        },
                        line_join: if round {
                            tiny_skia::LineJoin::Round
                        } else {
                            tiny_skia::LineJoin::Miter
                        },
                        miter_limit: 4.0,
                        ..tiny_skia::Stroke::default()
                    };
                    (color, Some(stroke))
                }
            };
            let mut paint = tiny_skia::Paint {
                force_hq_pipeline: true,
                ..tiny_skia::Paint::default()
            };
            paint.set_color_rgba8(color.r, color.g, color.b, color.a);
            if let Some(stroke) = style {
                // stroke_path silently skips an unrepresentable stroke. Build
                // its outline explicitly so export fails instead of losing it.
                let outline = path
                    .stroke(&stroke, 1.0)
                    .ok_or_else(|| RenderError::InvalidGeometry(element.id.clone()))?;
                pixmap.fill_path(
                    &outline,
                    &paint,
                    tiny_skia::FillRule::Winding,
                    tiny_skia::Transform::identity(),
                    None,
                );
            } else {
                pixmap.fill_path(
                    &path,
                    &paint,
                    tiny_skia::FillRule::Winding,
                    tiny_skia::Transform::identity(),
                    None,
                );
            }
        }
    }
    Ok(RenderedImage {
        width,
        height,
        pixels: pixmap.take_demultiplied(),
    })
}

fn coordinate(value: f64) -> Option<f32> {
    let value = value as f32;
    value.is_finite().then_some(value)
}

fn skia_path(path: &Path) -> Result<Option<tiny_skia::Path>, ()> {
    if path.is_empty() {
        return Ok(None);
    }
    let mut builder = tiny_skia::PathBuilder::new();
    let point = |p: crate::Point| -> Result<(f32, f32), ()> {
        Ok((coordinate(p.x).ok_or(())?, coordinate(p.y).ok_or(())?))
    };
    for element in path.elements() {
        match *element {
            PathEl::MoveTo(p) => {
                let (x, y) = point(p)?;
                builder.move_to(x, y);
            }
            PathEl::LineTo(p) => {
                let (x, y) = point(p)?;
                builder.line_to(x, y);
            }
            PathEl::CubicTo(a, b, c) => {
                let (ax, ay) = point(a)?;
                let (bx, by) = point(b)?;
                let (cx, cy) = point(c)?;
                builder.cubic_to(ax, ay, bx, by, cx, cy);
            }
            PathEl::Close => builder.close(),
        }
    }
    builder.finish().map(Some).ok_or(())
}
