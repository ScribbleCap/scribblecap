//! Pure helpers for the X11 backend: decoding `GetImage` replies, rounding regions to
//! whole pixels and spotting Wayland sessions. No X11 calls here, so this compiles and
//! is tested on every OS.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use crate::ScreenRect;

/// How a `ZPixmap` image from the X server is laid out (from the connection setup's
/// pixmap formats and the drawable's visual).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PixelLayout {
    /// 16, 24 or 32.
    pub bits_per_pixel: u8,
    /// Each row is padded to a multiple of this many bits (8, 16 or 32).
    pub scanline_pad: u8,
    /// The server's image byte order is MSB first.
    pub big_endian: bool,
    pub red_mask: u32,
    pub green_mask: u32,
    pub blue_mask: u32,
    /// Bits holding (premultiplied) alpha, or 0 for an opaque visual.
    pub alpha_mask: u32,
}

impl PixelLayout {
    /// A visual of `depth` bits with these colour masks: on a 32-bit visual the bits
    /// left over are alpha (ARGB windows under a compositor).
    pub fn new(
        bits_per_pixel: u8,
        scanline_pad: u8,
        big_endian: bool,
        depth: u8,
        [red_mask, green_mask, blue_mask]: [u32; 3],
    ) -> Self {
        let alpha_mask = if depth == 32 {
            !(red_mask | green_mask | blue_mask)
        } else {
            0
        };
        Self {
            bits_per_pixel,
            scanline_pad,
            big_endian,
            red_mask,
            green_mask,
            blue_mask,
            alpha_mask,
        }
    }

    fn stride(&self, width: usize) -> usize {
        let pad = usize::from(self.scanline_pad.max(8));
        (width * usize::from(self.bits_per_pixel)).div_ceil(pad) * pad / 8
    }
}

/// Scale the bits of `pixel` under `mask` to 0..=255.
fn channel(pixel: u32, mask: u32) -> u8 {
    if mask == 0 {
        return 0;
    }
    let max = mask >> mask.trailing_zeros();
    let value = (pixel & mask) >> mask.trailing_zeros();
    ((u64::from(value) * 255 + u64::from(max) / 2) / u64::from(max)) as u8
}

/// Convert a `ZPixmap` image to straight-alpha RGBA8. `None` if the layout isn't a
/// true-colour one we understand or the data is too short.
pub(crate) fn zpixmap_to_rgba(
    data: &[u8],
    width: usize,
    height: usize,
    layout: &PixelLayout,
) -> Option<Vec<u8>> {
    let bytes = match layout.bits_per_pixel {
        16 => 2,
        24 => 3,
        32 => 4,
        _ => return None,
    };
    let stride = layout.stride(width);
    if height > 0 && data.len() < stride * (height - 1) + width * bytes {
        return None;
    }
    let mut out = Vec::with_capacity(width * height * 4);
    for row in 0..height {
        let line = &data[row * stride..];
        for px in line[..width * bytes].chunks_exact(bytes) {
            let pixel = px.iter().enumerate().fold(0u32, |acc, (i, &b)| {
                let shift = if layout.big_endian {
                    8 * (bytes - 1 - i)
                } else {
                    8 * i
                };
                acc | (u32::from(b) << shift)
            });
            let a = if layout.alpha_mask == 0 {
                255
            } else {
                channel(pixel, layout.alpha_mask)
            };
            out.extend_from_slice(&[
                channel(pixel, layout.red_mask),
                channel(pixel, layout.green_mask),
                channel(pixel, layout.blue_mask),
                a,
            ]);
        }
    }
    if layout.alpha_mask != 0 {
        crate::unpremultiply(&mut out);
    }
    Some(out)
}

/// A rectangle in whole X11 pixels: `(x, y, width, height)`, rounded outwards and
/// limited to what an X11 request can carry. `None` if it is empty.
pub(crate) fn pixel_rect(rect: &ScreenRect) -> Option<(i16, i16, u16, u16)> {
    let clamp = |v: f64| v.clamp(f64::from(i16::MIN), f64::from(i16::MAX));
    let (x0, y0) = (clamp(rect.x.floor()), clamp(rect.y.floor()));
    let x1 = clamp((rect.x + rect.width).ceil());
    let y1 = clamp((rect.y + rect.height).ceil());
    (x1 > x0 && y1 > y0).then_some((x0 as i16, y0 as i16, (x1 - x0) as u16, (y1 - y0) as u16))
}

/// Whether this is a Wayland session. There XWayland only sees X11 windows, and
/// grabbing its root window gives black or stale pixels for everything else, so the
/// X11 backend refuses (Wayland is roadmap M4-4).
pub(crate) fn is_wayland_session(
    xdg_session_type: Option<&str>,
    wayland_display: Option<&str>,
) -> bool {
    xdg_session_type.is_some_and(|t| t.eq_ignore_ascii_case("wayland"))
        || wayland_display.is_some_and(|d| !d.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    const RGB: [u32; 3] = [0x00ff_0000, 0x0000_ff00, 0x0000_00ff];

    #[test]
    fn decodes_little_endian_bgrx() {
        let layout = PixelLayout::new(32, 32, false, 24, RGB);
        // Two pixels: red, then (10, 20, 30); the padding byte is ignored.
        let data = [0, 0, 255, 99, 30, 20, 10, 0];
        assert_eq!(
            zpixmap_to_rgba(&data, 2, 1, &layout).unwrap(),
            [255, 0, 0, 255, 10, 20, 30, 255]
        );
    }

    #[test]
    fn decodes_big_endian_xrgb() {
        let layout = PixelLayout::new(32, 32, true, 24, RGB);
        let data = [0, 10, 20, 30];
        assert_eq!(
            zpixmap_to_rgba(&data, 1, 1, &layout).unwrap(),
            [10, 20, 30, 255]
        );
    }

    #[test]
    fn decodes_rgb565_with_row_padding() {
        let layout = PixelLayout::new(16, 32, false, 16, [0xf800, 0x07e0, 0x001f]);
        // 1×2 image: each 2-byte row is padded to 4 bytes. White, then pure green.
        let data = [0xff, 0xff, 0, 0, 0xe0, 0x07, 0, 0];
        assert_eq!(
            zpixmap_to_rgba(&data, 1, 2, &layout).unwrap(),
            [255, 255, 255, 255, 0, 255, 0, 255]
        );
    }

    #[test]
    fn decodes_premultiplied_argb() {
        let layout = PixelLayout::new(32, 32, false, 32, RGB);
        // Half-transparent white, premultiplied: (128, 128, 128, 128).
        let data = [128, 128, 128, 128];
        assert_eq!(
            zpixmap_to_rgba(&data, 1, 1, &layout).unwrap(),
            [255, 255, 255, 128]
        );
    }

    #[test]
    fn rejects_short_data_and_odd_formats() {
        let layout = PixelLayout::new(32, 32, false, 24, RGB);
        assert!(zpixmap_to_rgba(&[0; 7], 2, 1, &layout).is_none());
        let odd = PixelLayout::new(8, 32, false, 8, RGB);
        assert!(zpixmap_to_rgba(&[0; 4], 1, 1, &odd).is_none());
    }

    #[test]
    fn pixel_rects_round_outwards_and_clamp() {
        assert_eq!(
            pixel_rect(&ScreenRect::new(10.5, 20.2, 100.0, 50.0)),
            Some((10, 20, 101, 51))
        );
        assert_eq!(
            pixel_rect(&ScreenRect::new(-40000.0, 0.0, 80000.0, 10.0)),
            Some((i16::MIN, 0, u16::MAX, 10))
        );
        assert_eq!(pixel_rect(&ScreenRect::new(5.0, 5.0, 0.0, 10.0)), None);
    }

    #[test]
    fn detects_wayland_sessions() {
        assert!(is_wayland_session(Some("wayland"), None));
        assert!(is_wayland_session(Some("x11"), Some("wayland-0")));
        assert!(!is_wayland_session(Some("x11"), None));
        assert!(!is_wayland_session(None, Some("")));
        assert!(!is_wayland_session(None, None));
    }
}
