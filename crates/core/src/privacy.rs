//! Destructive privacy operations. Only sanitized output pixels are returned.
use crate::canvas::{CanvasError, CanvasImage};
use crate::render::MAX_RENDER_BYTES;

#[derive(Clone, Copy, Debug)]
pub enum PrivacyOperation {
    Redact,
    Pixelate { block: u32 },
}

/// Apply an integer rectangle to straight RGBA8. Pixelation averages each block
/// over white and writes opaque pixels; hidden RGB under transparency is removed.
pub fn apply(
    width: u32,
    height: u32,
    pixels: &[u8],
    rect: [u32; 4],
    operation: PrivacyOperation,
) -> Result<CanvasImage, CanvasError> {
    let expected = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|n| n.checked_mul(4))
        .ok_or(CanvasError)?;
    let [x, y, w, h] = rect;
    if width == 0
        || height == 0
        || expected > MAX_RENDER_BYTES
        || expected != pixels.len() as u64
        || w == 0
        || h == 0
        || x.checked_add(w).is_none_or(|n| n > width)
        || y.checked_add(h).is_none_or(|n| n > height)
    {
        return Err(CanvasError);
    }
    let block = match operation {
        PrivacyOperation::Redact => 1,
        PrivacyOperation::Pixelate { block } if (2..=256).contains(&block) => block,
        _ => return Err(CanvasError),
    };
    let mut out = pixels.to_vec();
    for by in (y..y + h).step_by(block as usize) {
        for bx in (x..x + w).step_by(block as usize) {
            let right = bx.saturating_add(block).min(x + w);
            let bottom = by.saturating_add(block).min(y + h);
            let mut color = [0, 0, 0, 255];
            if matches!(operation, PrivacyOperation::Pixelate { .. }) {
                let mut sums = [0_u64; 3];
                let count = u64::from(right - bx) * u64::from(bottom - by);
                for py in by..bottom {
                    for px in bx..right {
                        let i = ((u64::from(py) * u64::from(width) + u64::from(px)) * 4) as usize;
                        let a = u64::from(pixels[i + 3]);
                        for c in 0..3 {
                            sums[c] += (u64::from(pixels[i + c]) * a + 255 * (255 - a) + 127) / 255;
                        }
                    }
                }
                for c in 0..3 {
                    color[c] = (sums[c] / count) as u8;
                }
            }
            for py in by..bottom {
                for px in bx..right {
                    let i = ((u64::from(py) * u64::from(width) + u64::from(px)) * 4) as usize;
                    out[i..i + 4].copy_from_slice(&color);
                }
            }
        }
    }
    Ok(CanvasImage {
        width,
        height,
        pixels: out,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redact_replaces_every_channel_and_leaves_other_pixels_untouched() {
        let source = vec![13, 29, 47, 0, 3, 5, 7, 255];
        let out = apply(2, 1, &source, [0, 0, 1, 1], PrivacyOperation::Redact).unwrap();
        assert_eq!(out.pixels, [0, 0, 0, 255, 3, 5, 7, 255]);
        assert_eq!(source[0], 13);
    }
    #[test]
    fn pixelate_averages_clipped_blocks_and_removes_hidden_rgb() {
        let source = [10, 20, 30, 255, 30, 40, 50, 255, 77, 88, 99, 0];
        let out = apply(
            3,
            1,
            &source,
            [0, 0, 3, 1],
            PrivacyOperation::Pixelate { block: 2 },
        )
        .unwrap();
        assert_eq!(
            out.pixels,
            [20, 30, 40, 255, 20, 30, 40, 255, 255, 255, 255, 255]
        );
    }
    #[test]
    fn invalid_regions_and_buffers_fail_without_modification() {
        for rect in [[0, 0, 0, 1], [1, 0, 2, 1], [u32::MAX, 0, 2, 1]] {
            assert!(apply(2, 1, &[0; 8], rect, PrivacyOperation::Redact).is_err());
        }
        assert!(apply(2, 1, &[0; 7], [0, 0, 1, 1], PrivacyOperation::Redact).is_err());
        assert!(
            apply(
                2,
                1,
                &[0; 8],
                [0, 0, 1, 1],
                PrivacyOperation::Pixelate { block: 1 }
            )
            .is_err()
        );
    }
}
