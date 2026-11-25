//! Pure geometry for the in-app capture flow: desktop points to image pixels
//! and display lookup. No OS calls.

use scribblecap_capture::{DisplayInfo, ScreenRect};

/// A rectangle in image pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Map a desktop-point `rect` onto a captured display image.
///
/// The rect is clipped to the display, the near edge is floored and the far
/// edge ceiled, then clamped to the image. Pixels per point come from the
/// image size, not `DisplayInfo::scale`, because some platforms report a scale
/// that differs from the real capture resolution. Returns `None` when the
/// clipped area is empty.
pub fn to_pixels(
    display: &DisplayInfo,
    image_width: u32,
    image_height: u32,
    rect: ScreenRect,
) -> Option<PixelRect> {
    let b = display.bounds;
    if b.width <= 0.0 || b.height <= 0.0 || image_width == 0 || image_height == 0 {
        return None;
    }
    let x0 = rect.x.max(b.x);
    let y0 = rect.y.max(b.y);
    let x1 = (rect.x + rect.width).min(b.x + b.width);
    let y1 = (rect.y + rect.height).min(b.y + b.height);
    if !(x1 > x0 && y1 > y0) {
        return None;
    }
    let sx = f64::from(image_width) / b.width;
    let sy = f64::from(image_height) / b.height;
    let px0 = (((x0 - b.x) * sx).floor().max(0.0) as u32).min(image_width);
    let py0 = (((y0 - b.y) * sy).floor().max(0.0) as u32).min(image_height);
    let px1 = (((x1 - b.x) * sx).ceil().max(0.0) as u32).min(image_width);
    let py1 = (((y1 - b.y) * sy).ceil().max(0.0) as u32).min(image_height);
    if px1 <= px0 || py1 <= py0 {
        return None;
    }
    Some(PixelRect {
        x: px0,
        y: py0,
        width: px1 - px0,
        height: py1 - py0,
    })
}

/// The display containing a desktop point. Bounds are half-open:
/// `[x, x + width) x [y, y + height)`.
pub fn display_at(displays: &[DisplayInfo], x: f64, y: f64) -> Option<&DisplayInfo> {
    displays.iter().find(|d| {
        let b = d.bounds;
        x >= b.x && x < b.x + b.width && y >= b.y && y < b.y + b.height
    })
}

/// A remembered capture region and the display it was drawn on.
#[derive(Clone)]
pub struct PreviousRegion {
    pub display: DisplayInfo,
    pub rect: ScreenRect,
}

impl PreviousRegion {
    /// True when a display with the same id, bounds and scale is still present.
    pub fn still_valid(&self, displays: &[DisplayInfo]) -> bool {
        displays.iter().any(|d| {
            d.id == self.display.id
                && d.bounds == self.display.bounds
                && d.scale == self.display.scale
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_points_with_image_size_not_reported_scale() {
        // Windows reports system DPI (1.0) on a 2x monitor: image size wins.
        let d = DisplayInfo {
            id: 2,
            bounds: ScreenRect::new(1920.0, 0.0, 1280.0, 800.0),
            scale: 1.0,
        };
        let r = to_pixels(&d, 2560, 1600, ScreenRect::new(1930.5, 10.0, 100.0, 50.25)).unwrap();
        assert_eq!((r.x, r.y, r.width, r.height), (21, 20, 200, 101));
    }

    #[test]
    fn floors_a_fractional_near_edge() {
        let d = DisplayInfo {
            id: 1,
            bounds: ScreenRect::new(0.0, 0.0, 100.0, 100.0),
            scale: 1.0,
        };
        // 20 * 1.02 = 20.4 px: floor gives 20 (ceil would give 21).
        let r = to_pixels(&d, 102, 102, ScreenRect::new(20.0, 20.0, 10.0, 10.0)).unwrap();
        assert_eq!((r.x, r.y), (20, 20));
        // 20.2 * 1.02 = 20.604 px: floor gives 20 (round or ceil would give 21).
        let r = to_pixels(&d, 102, 102, ScreenRect::new(20.2, 20.2, 10.0, 10.0)).unwrap();
        assert_eq!((r.x, r.y), (20, 20));
    }

    #[test]
    fn uses_separate_x_and_y_scales_for_non_uniform_images() {
        // 2560x1200 over 1280x800 points: sx = 2.0, sy = 1.5.
        let d = DisplayInfo {
            id: 1,
            bounds: ScreenRect::new(100.0, 50.0, 1280.0, 800.0),
            scale: 2.0,
        };
        let r = to_pixels(&d, 2560, 1200, ScreenRect::new(110.0, 70.0, 30.0, 40.0)).unwrap();
        assert_eq!((r.x, r.y, r.width, r.height), (20, 30, 60, 60));
    }

    #[test]
    fn clips_to_the_display_and_rejects_empty_areas() {
        let d = DisplayInfo {
            id: 1,
            bounds: ScreenRect::new(0.0, 0.0, 100.0, 100.0),
            scale: 1.0,
        };
        assert_eq!(
            to_pixels(&d, 100, 100, ScreenRect::new(90.0, 90.0, 50.0, 50.0))
                .map(|r| (r.width, r.height)),
            Some((10, 10))
        );
        assert!(to_pixels(&d, 100, 100, ScreenRect::new(200.0, 0.0, 10.0, 10.0)).is_none());
        assert!(to_pixels(&d, 100, 100, ScreenRect::new(5.0, 5.0, 0.0, 10.0)).is_none());
    }

    #[test]
    fn finds_the_display_under_a_point_including_negative_origins() {
        let left = DisplayInfo {
            id: 1,
            bounds: ScreenRect::new(-1440.0, 0.0, 1440.0, 900.0),
            scale: 2.0,
        };
        let main = DisplayInfo {
            id: 2,
            bounds: ScreenRect::new(0.0, 0.0, 1920.0, 1080.0),
            scale: 1.0,
        };
        let all = [left.clone(), main.clone()];
        assert_eq!(display_at(&all, -10.0, 5.0).map(|d| d.id), Some(1));
        assert_eq!(display_at(&all, 0.0, 0.0).map(|d| d.id), Some(2));
        assert!(display_at(&all, 5000.0, 0.0).is_none());
    }

    #[test]
    fn previous_region_needs_the_same_display_geometry() {
        let d = DisplayInfo {
            id: 1,
            bounds: ScreenRect::new(0.0, 0.0, 100.0, 100.0),
            scale: 2.0,
        };
        let p = PreviousRegion {
            display: d.clone(),
            rect: ScreenRect::new(1.0, 1.0, 5.0, 5.0),
        };
        assert!(p.still_valid(std::slice::from_ref(&d)));
        assert!(!p.still_valid(&[DisplayInfo {
            scale: 1.0,
            ..d.clone()
        }]));
        assert!(!p.still_valid(&[DisplayInfo {
            bounds: ScreenRect::new(10.0, 0.0, 100.0, 100.0),
            ..d.clone()
        }]));
        assert!(!p.still_valid(&[DisplayInfo {
            bounds: ScreenRect::new(0.0, 0.0, 120.0, 100.0),
            ..d.clone()
        }]));
        assert!(!p.still_valid(&[DisplayInfo { id: 9, ..d }]));
        assert!(!p.still_valid(&[]));
    }
}
