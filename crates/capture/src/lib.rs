//! ScribbleCap screen capture: one backend per OS behind a common trait
//! (decision 0002).
//!
//! Coordinates are **desktop points**: the global coordinate space the OS uses for
//! window positions, origin at the top-left of the primary display, y down. Captured
//! images are in **pixels** (points × the display's scale factor).

use std::fmt;

#[cfg(target_os = "macos")]
mod macos;

/// A rectangle in desktop points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScreenRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl ScreenRect {
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// The overlap of two rectangles, if it has a positive area.
    pub fn intersect(&self, other: &Self) -> Option<Self> {
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = (self.x + self.width).min(other.x + other.width);
        let y1 = (self.y + self.height).min(other.y + other.height);
        (x1 > x0 && y1 > y0).then(|| Self::new(x0, y0, x1 - x0, y1 - y0))
    }

    fn area(&self) -> f64 {
        self.width * self.height
    }
}

/// A connected display.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayInfo {
    pub id: u32,
    pub bounds: ScreenRect,
    /// Pixels per point (2.0 on a typical Retina display).
    pub scale: f64,
}

/// A capturable on-screen window (normal application windows only).
#[derive(Clone, Debug, PartialEq)]
pub struct WindowInfo {
    pub id: u32,
    pub title: String,
    pub app_name: String,
    pub bounds: ScreenRect,
}

/// A captured image: row-major RGBA8, straight alpha, sRGB (the same layout as
/// `scribblecap_formats::RasterImage`).
#[derive(Clone, Debug, PartialEq)]
pub struct CapturedImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    /// Pixels per point of the display it came from, so the editor can show it at its
    /// on-screen size.
    pub scale: f64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaptureError {
    /// The user hasn't allowed screen capture (macOS: System Settings → Privacy &
    /// Security → Screen & System Audio Recording).
    PermissionDenied,
    /// No such window or display, or the region is off every display.
    NotFound,
    /// Capture isn't available on this OS (version) yet.
    Unsupported(&'static str),
    /// The OS capture API failed.
    Os(String),
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PermissionDenied => f.write_str("screen capture permission was not granted"),
            Self::NotFound => f.write_str("nothing to capture there"),
            Self::Unsupported(why) => write!(f, "screen capture is not supported: {why}"),
            Self::Os(msg) => write!(f, "screen capture failed: {msg}"),
        }
    }
}

impl std::error::Error for CaptureError {}

pub type Result<T> = std::result::Result<T, CaptureError>;

/// A platform capture backend. Calls block until the image is ready; don't call them
/// on a UI thread.
pub trait Capturer {
    /// Whether the app may capture the screen without asking.
    fn has_permission(&self) -> bool;
    /// Ask the OS for permission (may show a system prompt). Returns the current state.
    fn request_permission(&self) -> bool;
    fn displays(&self) -> Result<Vec<DisplayInfo>>;
    /// Normal on-screen windows, front to back. Excludes this app's own windows.
    fn list_windows(&self) -> Result<Vec<WindowInfo>>;
    /// A whole display (`None` = the primary one), without this app's windows.
    fn capture_screen(&self, display: Option<u32>) -> Result<CapturedImage>;
    /// One window by [`WindowInfo::id`], even if other windows cover it.
    fn capture_window(&self, id: u32) -> Result<CapturedImage>;
    /// A region in desktop points, without this app's windows. A region spanning
    /// several displays is clipped to the display it overlaps most.
    fn capture_region(&self, rect: ScreenRect) -> Result<CapturedImage>;
}

/// The capture backend for this OS.
pub fn backend() -> Result<Box<dyn Capturer>> {
    #[cfg(target_os = "macos")]
    {
        macos::MacCapturer::new().map(|c| Box::new(c) as Box<dyn Capturer>)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(CaptureError::Unsupported(
            "not implemented on this OS yet (roadmap M4)",
        ))
    }
}

/// The display a region should be captured from (the one it overlaps most) and the
/// region clipped to it.
pub fn region_on_display<'a>(
    rect: &ScreenRect,
    displays: &'a [DisplayInfo],
) -> Option<(&'a DisplayInfo, ScreenRect)> {
    displays
        .iter()
        .filter_map(|d| d.bounds.intersect(rect).map(|r| (d, r)))
        .max_by(|a, b| a.1.area().total_cmp(&b.1.area()))
}

/// Size in whole pixels of `points` at `scale`.
pub fn pixel_size(points: f64, scale: f64) -> usize {
    (points * scale).round().max(1.0) as usize
}

/// Convert premultiplied RGBA8 to straight alpha in place.
pub fn unpremultiply(pixels: &mut [u8]) {
    for px in pixels.as_chunks_mut::<4>().0 {
        let a = u16::from(px[3]);
        if a != 0 && a != 255 {
            for c in &mut px[..3] {
                *c = ((u16::from(*c) * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display(id: u32, x: f64, y: f64, w: f64, h: f64) -> DisplayInfo {
        DisplayInfo {
            id,
            bounds: ScreenRect::new(x, y, w, h),
            scale: 2.0,
        }
    }

    #[test]
    fn region_picks_the_display_it_overlaps_most() {
        // Primary 1440×900 at the origin, a second display to its right.
        let displays = [
            display(1, 0.0, 0.0, 1440.0, 900.0),
            display(2, 1440.0, 0.0, 1920.0, 1080.0),
        ];
        let (d, r) =
            region_on_display(&ScreenRect::new(1400.0, 100.0, 200.0, 50.0), &displays).unwrap();
        assert_eq!(d.id, 2);
        assert_eq!(r, ScreenRect::new(1440.0, 100.0, 160.0, 50.0));

        let (d, r) =
            region_on_display(&ScreenRect::new(-50.0, -50.0, 150.0, 100.0), &displays).unwrap();
        assert_eq!(d.id, 1);
        assert_eq!(r, ScreenRect::new(0.0, 0.0, 100.0, 50.0));

        assert!(region_on_display(&ScreenRect::new(5000.0, 0.0, 10.0, 10.0), &displays).is_none());
        assert!(region_on_display(&ScreenRect::new(10.0, 10.0, 0.0, 10.0), &displays).is_none());
    }

    #[test]
    fn pixel_sizes_round_and_never_reach_zero() {
        assert_eq!(pixel_size(100.0, 2.0), 200);
        assert_eq!(pixel_size(100.25, 2.0), 201);
        assert_eq!(pixel_size(0.1, 1.0), 1);
    }

    #[test]
    fn unpremultiplies() {
        let mut px = [128, 64, 0, 128, 10, 20, 30, 255, 0, 0, 0, 0, 255, 0, 0, 255];
        unpremultiply(&mut px);
        assert_eq!(
            px,
            [
                255, 128, 0, 128, 10, 20, 30, 255, 0, 0, 0, 0, 255, 0, 0, 255
            ]
        );
    }

    #[test]
    fn backend_is_available_or_explains_why() {
        match backend() {
            #[cfg(target_os = "macos")]
            Ok(_) => {}
            #[cfg(not(target_os = "macos"))]
            Ok(_) => panic!("only macOS has a backend so far"),
            Err(CaptureError::Unsupported(why)) => assert!(!why.is_empty()),
            Err(e) => panic!("unexpected {e}"),
        }
    }
}
