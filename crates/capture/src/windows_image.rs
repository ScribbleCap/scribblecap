//! Pure Windows readback/coordinate helpers, tested on every OS.
use crate::{CaptureError, CapturedImage, Result, ScreenRect, unpremultiply};

pub(super) const MAX_IMAGE_BYTES: usize = 512 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Round both edges in the same global coordinate space; adjacent regions share
/// an edge, including on monitors left/above the primary display.
pub(super) fn crop_rect(region: ScreenRect, display: ScreenRect, scale: f64) -> Result<PixelRect> {
    if ![region.x, region.y, region.width, region.height, scale]
        .iter()
        .all(|v| v.is_finite())
        || scale <= 0.0
        || region.width <= 0.0
        || region.height <= 0.0
    {
        return Err(CaptureError::NotFound);
    }
    let x = ((region.x - display.x) * scale).round().max(0.0) as u32;
    let y = ((region.y - display.y) * scale).round().max(0.0) as u32;
    let right = ((region.x + region.width - display.x) * scale)
        .round()
        .max(0.0) as u32;
    let bottom = ((region.y + region.height - display.y) * scale)
        .round()
        .max(0.0) as u32;
    let width = right.saturating_sub(x);
    let height = bottom.saturating_sub(y);
    if width == 0 || height == 0 {
        return Err(CaptureError::NotFound);
    }
    Ok(PixelRect {
        x,
        y,
        width,
        height,
    })
}

pub(super) fn bgra_to_rgba(
    data: &[u8],
    width: u32,
    height: u32,
    pitch: usize,
    crop: Option<PixelRect>,
    scale: f64,
) -> Result<CapturedImage> {
    let bad = || CaptureError::Os("invalid Windows capture buffer or changed capture size".into());
    let row_bytes = (width as usize).checked_mul(4).ok_or_else(bad)?;
    let required = pitch.checked_mul(height as usize).ok_or_else(bad)?;
    let crop = crop.unwrap_or(PixelRect {
        x: 0,
        y: 0,
        width,
        height,
    });
    let size = (crop.width as usize)
        .checked_mul(crop.height as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(bad)?;
    if width == 0
        || height == 0
        || crop.width == 0
        || crop.height == 0
        || pitch < row_bytes
        || required > data.len()
        || required > MAX_IMAGE_BYTES
        || size > MAX_IMAGE_BYTES
        || !scale.is_finite()
        || scale <= 0.0
        || crop.x.checked_add(crop.width).is_none_or(|x| x > width)
        || crop.y.checked_add(crop.height).is_none_or(|y| y > height)
    {
        return Err(bad());
    }
    let mut pixels = Vec::with_capacity(size);
    for y in crop.y..crop.y + crop.height {
        let start = y as usize * pitch + crop.x as usize * 4;
        let end = start + crop.width as usize * 4;
        for px in data[start..end].as_chunks::<4>().0 {
            pixels.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
        }
    }
    unpremultiply(&mut pixels);
    Ok(CapturedImage {
        width: crop.width,
        height: crop.height,
        pixels,
        scale,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padded_bgra_rows_crop_and_unpremultiply_without_padding() {
        let data = [
            0, 0, 255, 255, 0, 64, 128, 128, 99, 99, 99, 99, 255, 0, 0, 255, 0, 255, 0, 255, 99,
            99, 99, 99,
        ];
        let image = bgra_to_rgba(
            &data,
            2,
            2,
            12,
            Some(PixelRect {
                x: 1,
                y: 0,
                width: 1,
                height: 2,
            }),
            1.5,
        )
        .unwrap();
        assert_eq!(image.pixels, [255, 128, 0, 128, 0, 255, 0, 255]);
        assert_eq!((image.width, image.height, image.scale), (1, 2, 1.5));
        let all = bgra_to_rgba(&data, 2, 2, 12, None, 1.0).unwrap();
        assert_eq!(all.pixels.len(), 16);
        assert_eq!(&all.pixels[..4], &[255, 0, 0, 255]);
        assert_eq!(&all.pixels[8..12], &[0, 0, 255, 255]);
    }

    #[test]
    fn rejects_short_rows_truncated_data_and_stale_crop_sizes() {
        assert!(bgra_to_rgba(&[0; 16], 2, 2, 7, None, 1.0).is_err());
        assert!(bgra_to_rgba(&[0; 15], 2, 2, 8, None, 1.0).is_err());
        for crop in [
            PixelRect {
                x: 1,
                y: 0,
                width: 2,
                height: 1,
            },
            PixelRect {
                x: u32::MAX,
                y: 0,
                width: 2,
                height: 1,
            },
            PixelRect {
                x: 0,
                y: 0,
                width: 0,
                height: 1,
            },
        ] {
            assert!(bgra_to_rgba(&[0; 16], 2, 2, 8, Some(crop), 1.0).is_err());
        }
        assert!(bgra_to_rgba(&[], 1, u32::MAX, usize::MAX, None, 1.0).is_err());
    }

    #[test]
    fn negative_monitor_origin_and_fractional_scale_share_rounded_edges() {
        let d = ScreenRect::new(-1280.0, -100.0, 1280.0, 720.0);
        let a = crop_rect(ScreenRect::new(-1279.0, -98.0, 10.0, 4.0), d, 1.5).unwrap();
        let b = crop_rect(ScreenRect::new(-1269.0, -98.0, 10.0, 4.0), d, 1.5).unwrap();
        assert_eq!(
            a,
            PixelRect {
                x: 2,
                y: 3,
                width: 15,
                height: 6
            }
        );
        assert_eq!(a.x + a.width, b.x);
        for w in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(crop_rect(ScreenRect::new(0.0, 0.0, w, 1.0), d, 1.0).is_err());
        }
    }
}
