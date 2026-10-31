//! Deterministic bundled-font shaping and image-pixel glyph outlines.
use rustybuzz::{
    Face, UnicodeBuffer,
    ttf_parser::{GlyphId, OutlineBuilder},
};
use unicode_segmentation::UnicodeSegmentation;

use super::primitives::{circle, fill, rounded_rect, stroke};
use super::{PaintedPath, UnsupportedShape};
use crate::{Color, Path, Point, Rect, Step, Text, TextAlign, TextStyle};

const FONT: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const MAX_TEXT: usize = 16_384;

/// The glyph path and logical box, before decoration. Origin is the top-left of
/// the font's ascender box; width includes the requested wrapping/alignment box.
#[derive(Clone, Debug, PartialEq)]
pub struct TextLayout {
    pub path: Path,
    pub width: f64,
    pub height: f64,
}

fn face() -> Face<'static> {
    Face::from_slice(FONT, 0).expect("bundled Noto Sans is a valid font")
}

fn shape(face: &Face<'_>, text: &str) -> Result<Vec<rustybuzz::GlyphBuffer>, UnsupportedShape> {
    let bidi = unicode_bidi::BidiInfo::new(text, None);
    let mut result = Vec::new();
    if let Some(paragraph) = bidi.paragraphs.first() {
        let (levels, runs) = bidi.visual_runs(paragraph, paragraph.range.clone());
        for run in runs {
            let mut buffer = UnicodeBuffer::new();
            buffer.push_str(&text[run.clone()]);
            buffer.guess_segment_properties();
            buffer.set_direction(if levels[run.start].is_rtl() {
                rustybuzz::Direction::RightToLeft
            } else {
                rustybuzz::Direction::LeftToRight
            });
            let buffer = rustybuzz::shape(face, &[], buffer);
            if buffer.glyph_infos().iter().any(|g| g.glyph_id == 0) {
                return Err(UnsupportedShape(
                    "text contains a glyph absent from bundled Noto Sans",
                ));
            }
            result.push(buffer);
        }
    }
    Ok(result)
}

fn advance(face: &Face<'_>, text: &str, scale: f64) -> Result<f64, UnsupportedShape> {
    Ok(shape(face, text)?
        .iter()
        .flat_map(|b| b.glyph_positions())
        .map(|p| f64::from(p.x_advance) * scale)
        .sum())
}

fn wrap<'a>(
    face: &Face<'_>,
    text: &'a str,
    scale: f64,
    width: Option<f64>,
) -> Result<Vec<&'a str>, UnsupportedShape> {
    let Some(width) = width else {
        return Ok(vec![text]);
    };
    let budget = std::cell::Cell::new(1_000_000_usize);
    let measure = |value: &str| {
        let remaining = budget
            .get()
            .checked_sub(value.len())
            .ok_or(UnsupportedShape("text wrapping exceeds shaping budget"))?;
        budget.set(remaining);
        advance(face, value, scale)
    };
    let mut lines = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let remaining = &text[start..];
        if measure(remaining)? <= width {
            lines.push(remaining);
            break;
        }
        let mut end = 0;
        for (boundary, _) in unicode_linebreak::linebreaks(remaining) {
            if measure(remaining[..boundary].trim_end())? > width {
                break;
            }
            end = boundary;
        }
        if end == 0 {
            for (i, grapheme) in remaining.grapheme_indices(true) {
                let boundary = i + grapheme.len();
                if end > 0 && measure(&remaining[..boundary])? > width {
                    break;
                }
                end = boundary;
            }
        }
        lines.push(remaining[..end].trim_end());
        start += end;
    }
    if text.is_empty() {
        lines.push("");
    }
    Ok(lines)
}

/// Shape text using bundled Noto Sans only. Unknown family names use that font;
/// unsupported glyphs, invalid metrics and excessive input return explicit errors.
pub fn layout_text(text: &Text) -> Result<TextLayout, UnsupportedShape> {
    if text.text.len() > MAX_TEXT
        || !text.origin.is_finite()
        || !text.font_size.is_finite()
        || text.font_size <= 0.0
        || text.wrap_width.is_some_and(|w| !w.is_finite() || w <= 0.0)
    {
        return Err(UnsupportedShape("invalid or excessive text layout"));
    }
    let face = face();
    let scale = text.font_size / f64::from(face.units_per_em());
    let ascent = f64::from(face.ascender()) * scale;
    let line_height = f64::from(
        i32::from(face.ascender()) - i32::from(face.descender()) + i32::from(face.line_gap()),
    ) * scale;
    let normalized = text
        .text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\t', "    ");
    if normalized.len() > MAX_TEXT {
        return Err(UnsupportedShape("normalized text exceeds layout limit"));
    }
    let mut lines = Vec::new();
    for paragraph in normalized.split('\n') {
        lines.extend(wrap(&face, paragraph, scale, text.wrap_width)?);
    }
    let widths = lines
        .iter()
        .map(|s| advance(&face, s, scale))
        .collect::<Result<Vec<_>, _>>()?;
    let width = text
        .wrap_width
        .unwrap_or_else(|| widths.iter().copied().fold(0.0, f64::max));
    let height = line_height * lines.len() as f64;
    if !width.is_finite()
        || !height.is_finite()
        || width > f64::from(f32::MAX)
        || height > f64::from(f32::MAX)
    {
        return Err(UnsupportedShape("text metrics exceed geometry limits"));
    }
    let mut outline = GlyphOutline {
        path: Path::new(),
        pen: Point::new(0.0, 0.0),
        scale,
        current: Point::new(0.0, 0.0),
    };
    for (index, (line, measured)) in lines.iter().zip(widths).enumerate() {
        let offset = match text.align {
            TextAlign::Left => 0.0,
            TextAlign::Center => (width - measured) / 2.0,
            TextAlign::Right => width - measured,
        };
        let mut x = text.origin.x + offset;
        let baseline = text.origin.y + ascent + index as f64 * line_height;
        for glyphs in shape(&face, line)? {
            for (info, pos) in glyphs.glyph_infos().iter().zip(glyphs.glyph_positions()) {
                outline.pen = Point::new(
                    x + f64::from(pos.x_offset) * scale,
                    baseline - f64::from(pos.y_offset) * scale,
                );
                face.outline_glyph(GlyphId(info.glyph_id as u16), &mut outline);
                x += f64::from(pos.x_advance) * scale;
            }
        }
    }

    Ok(TextLayout {
        path: outline.path,
        width,
        height,
    })
}

struct GlyphOutline {
    path: Path,
    pen: Point,
    scale: f64,
    current: Point,
}
impl GlyphOutline {
    fn point(&self, x: f32, y: f32) -> Point {
        Point::new(
            self.pen.x + f64::from(x) * self.scale,
            self.pen.y - f64::from(y) * self.scale,
        )
    }
}
impl OutlineBuilder for GlyphOutline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.current = self.point(x, y);
        self.path.move_to(self.current);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.current = self.point(x, y);
        self.path.line_to(self.current);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let control = self.point(x1, y1);
        let end = self.point(x, y);
        let a = Point::new(
            self.current.x + (control.x - self.current.x) * (2.0 / 3.0),
            self.current.y + (control.y - self.current.y) * (2.0 / 3.0),
        );
        let b = Point::new(
            end.x + (control.x - end.x) * (2.0 / 3.0),
            end.y + (control.y - end.y) * (2.0 / 3.0),
        );
        self.path.cubic_to(a, b, end);
        self.current = end;
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let end = self.point(x, y);
        self.path
            .cubic_to(self.point(x1, y1), self.point(x2, y2), end);
        self.current = end;
    }
    fn close(&mut self) {
        self.path.close();
    }
}

fn contrast(color: Color) -> Color {
    let light = u32::from(color.r) * 299 + u32::from(color.g) * 587 + u32::from(color.b) * 114;
    if light < 128_000 {
        Color::rgba(255, 255, 255, color.a)
    } else {
        Color::rgba(0, 0, 0, color.a)
    }
}

pub(super) fn text_paths(text: &Text) -> Result<Vec<PaintedPath>, UnsupportedShape> {
    let layout = layout_text(text)?;
    let mut paths = Vec::new();
    if text.text.is_empty() {
        return Ok(paths);
    }
    match text.style {
        TextStyle::Plain | TextStyle::Label => stroke(
            &mut paths,
            layout.path.clone(),
            contrast(text.color),
            text.font_size * 0.28,
            true,
        ),
        TextStyle::Bubble => {
            let pad = text.font_size * 0.3;
            let rect = Rect::new(
                text.origin.x - pad,
                text.origin.y - pad,
                layout.width + 2.0 * pad,
                layout.height + 2.0 * pad,
            );
            let mut tail = Path::new();
            tail.move_to(Point::new(rect.x + pad, rect.y + rect.height - pad))
                .line_to(Point::new(rect.x + pad, rect.y + rect.height + pad))
                .line_to(Point::new(rect.x + 3.0 * pad, rect.y + rect.height - pad))
                .close();
            fill(&mut paths, tail, contrast(text.color));
            fill(&mut paths, rounded_rect(rect, pad), contrast(text.color));
        }
    }
    fill(&mut paths, layout.path, text.color);
    Ok(paths)
}

pub(super) fn step_paths(step: &Step) -> Result<Vec<PaintedPath>, UnsupportedShape> {
    if !step.center.is_finite()
        || !step.diameter.is_finite()
        || step.diameter <= 0.0
        || step.number == 0
    {
        return Err(UnsupportedShape("invalid step geometry"));
    }
    let text = Text {
        origin: Point::new(0.0, 0.0),
        text: step.number.to_string(),
        style: TextStyle::Plain,
        color: contrast(step.color),
        font_size: 1.0,
        font: None,
        wrap_width: None,
        align: TextAlign::Left,
        attached_to: None,
    };
    let layout = layout_text(&text)?;
    // Fit and center the actual ink, including multi-digit u32 numbers.
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for p in layout.path.points() {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }
    let scale =
        (step.diameter * 0.65 / (max_x - min_x)).min(step.diameter * 0.55 / (max_y - min_y));
    let path = layout.path.map_points(|p| {
        Point::new(
            step.center.x + (p.x - (min_x + max_x) / 2.0) * scale,
            step.center.y + (p.y - (min_y + max_y) / 2.0) * scale,
        )
    });
    let mut paths = Vec::new();
    fill(
        &mut paths,
        circle(step.center, step.diameter / 2.0),
        step.color,
    );
    fill(&mut paths, path, contrast(step.color));
    Ok(paths)
}
