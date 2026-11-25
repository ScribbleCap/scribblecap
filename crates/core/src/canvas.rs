//! Bounded, lossless pixel rearrangements and nearest-neighbour image resizing.
//!
//! These operate on a background resource; callers keep the previous resource for undo.
use std::fmt;

use crate::render::MAX_RENDER_BYTES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasOperation {
    Crop {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
    Resize {
        width: u32,
        height: u32,
    },
    /// Place the old image at (x, y) on a larger transparent canvas.
    Expand {
        width: u32,
        height: u32,
        x: u32,
        y: u32,
    },
    RotateClockwise,
    RotateCounterclockwise,
    FlipHorizontal,
    FlipVertical,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanvasImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CanvasError;
impl fmt::Display for CanvasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Invalid canvas dimensions, pixel buffer, or transform bounds")
    }
}
impl std::error::Error for CanvasError {}

fn length(width: u32, height: u32) -> Result<usize, CanvasError> {
    let bytes = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|n| n.checked_mul(4))
        .ok_or(CanvasError)?;
    if width == 0 || height == 0 || bytes > MAX_RENDER_BYTES {
        return Err(CanvasError);
    }
    usize::try_from(bytes).map_err(|_| CanvasError)
}

/// Transform straight RGBA8 without modifying the input. Resize samples the nearest
/// source pixel, preserving sharp screenshot edges and avoiding alpha fringes.
pub fn transform(
    width: u32,
    height: u32,
    pixels: &[u8],
    op: CanvasOperation,
) -> Result<CanvasImage, CanvasError> {
    if length(width, height)? != pixels.len() {
        return Err(CanvasError);
    }
    use CanvasOperation::*;
    let (out_width, out_height) = match op {
        Crop {
            x,
            y,
            width: w,
            height: h,
        } => {
            if x.checked_add(w).is_none_or(|end| end > width)
                || y.checked_add(h).is_none_or(|end| end > height)
            {
                return Err(CanvasError);
            }
            (w, h)
        }
        Resize { width, height } => (width, height),
        Expand {
            width: w,
            height: h,
            x,
            y,
        } => {
            if x.checked_add(width).is_none_or(|end| end > w)
                || y.checked_add(height).is_none_or(|end| end > h)
            {
                return Err(CanvasError);
            }
            (w, h)
        }
        RotateClockwise | RotateCounterclockwise => (height, width),
        FlipHorizontal | FlipVertical => (width, height),
    };
    let mut output = vec![0; length(out_width, out_height)?];
    for y in 0..out_height {
        for x in 0..out_width {
            let source = match op {
                Crop {
                    x: left, y: top, ..
                } => Some((x + left, y + top)),
                Resize { .. } => Some((
                    (u64::from(x) * u64::from(width) / u64::from(out_width)) as u32,
                    (u64::from(y) * u64::from(height) / u64::from(out_height)) as u32,
                )),
                Expand {
                    x: left, y: top, ..
                } => x
                    .checked_sub(left)
                    .zip(y.checked_sub(top))
                    .filter(|&(sx, sy)| sx < width && sy < height),
                RotateClockwise => Some((y, height - 1 - x)),
                RotateCounterclockwise => Some((width - 1 - y, x)),
                FlipHorizontal => Some((width - 1 - x, y)),
                FlipVertical => Some((x, height - 1 - y)),
            };
            if let Some((sx, sy)) = source {
                let src = (u64::from(sy) * u64::from(width) * 4 + u64::from(sx) * 4) as usize;
                let dst = (u64::from(y) * u64::from(out_width) * 4 + u64::from(x) * 4) as usize;
                output[dst..dst + 4].copy_from_slice(&pixels[src..src + 4]);
            }
        }
    }
    Ok(CanvasImage {
        width: out_width,
        height: out_height,
        pixels: output,
    })
}
