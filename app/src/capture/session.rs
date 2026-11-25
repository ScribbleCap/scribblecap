//! The in-app capture flow without windows: hide guard, freeze, crop, save names.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::NaiveDateTime;
use scribblecap_capture::{CapturedImage, Capturer, DisplayInfo, ScreenRect, WindowInfo};
use scribblecap_core::canvas::{CanvasOperation, transform};
use scribblecap_formats::RasterImage;

use super::geometry::{PreviousRegion, to_pixels};
use super::naming::capture_file_name;

/// Wait after hiding the editor or closing the badge before freezing, so the
/// window server has removed them from the screen.
pub const HIDE_SETTLE_MS: u64 = 150;

/// Shows the editor again when dropped: on success, early return, error or panic.
pub struct EditorHidden<F: FnMut()> {
    show: Option<F>,
}

impl<F: FnMut()> EditorHidden<F> {
    /// Construct right after hiding the editor.
    pub fn new(show: F) -> Self {
        Self { show: Some(show) }
    }

    /// The editor stays hidden: overlays or the countdown now own showing it.
    pub fn keep_hidden(mut self) {
        self.show = None;
    }
}

impl<F: FnMut()> Drop for EditorHidden<F> {
    fn drop(&mut self) {
        if let Some(mut show) = self.show.take() {
            show();
        }
    }
}

/// One display's picture, taken before the overlays open.
pub struct Frozen {
    pub display: DisplayInfo,
    pub image: CapturedImage,
}

/// Capture every display with `capture_screen(Some(id))`.
pub fn freeze(capturer: &dyn Capturer) -> scribblecap_capture::Result<Vec<Frozen>> {
    capturer
        .displays()?
        .into_iter()
        .map(|display| {
            capturer
                .capture_screen(Some(display.id))
                .map(|image| Frozen { display, image })
        })
        .collect()
}

/// Crop a desktop-point rectangle out of a frozen display picture.
pub fn crop(frozen: &Frozen, rect: ScreenRect) -> Result<RasterImage, String> {
    let image = &frozen.image;
    let area = to_pixels(&frozen.display, image.width, image.height, rect)
        .ok_or("The selection is outside the screen.")?;
    let cropped = transform(
        image.width,
        image.height,
        &image.pixels,
        CanvasOperation::Crop {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height,
        },
    )
    .map_err(|e| e.to_string())?;
    RasterImage::new(cropped.width, cropped.height, cropped.pixels).map_err(|e| e.to_string())
}

/// A whole captured picture as an editor image.
pub fn raster(image: CapturedImage) -> Result<RasterImage, String> {
    RasterImage::new(image.width, image.height, image.pixels).map_err(|e| e.to_string())
}

/// The next free capture path in `folder`. Only a hint: save with `save_capture`.
pub fn capture_target(folder: &Path, at: NaiveDateTime) -> Option<PathBuf> {
    Some(folder.join(capture_file_name(at, |n| folder.join(n).exists())?))
}

/// Write `bytes` to a new capture file in `folder`, never replacing a file.
/// A name taken meanwhile is retried with a fresh time from `now`; any other
/// failure (a missing folder, a file system without hard links) is reported once.
pub fn save_capture(
    folder: &Path,
    bytes: &[u8],
    mut now: impl FnMut() -> NaiveDateTime,
) -> Result<PathBuf, String> {
    for _ in 0..1000 {
        let Some(path) = capture_target(folder, now()) else {
            std::thread::sleep(std::time::Duration::from_millis(1));
            continue;
        };
        match crate::output::save_new(&path, bytes) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => {
                return Err(format!(
                    "The capture could not be saved in the capture folder ({}): {e}",
                    folder.display()
                ));
            }
        }
    }
    Err("The capture could not be saved: no free file name in the capture folder.".into())
}

/// Desktop points from a physical cursor position, using the monitor under it
/// (`(x, y, width, height, scale)` in physical pixels).
pub fn cursor_points(cursor: (f64, f64), monitors: &[(f64, f64, f64, f64, f64)]) -> (f64, f64) {
    let scale = monitors
        .iter()
        .find(|(x, y, w, h, _)| {
            cursor.0 >= *x && cursor.0 < x + w && cursor.1 >= *y && cursor.1 < y + h
        })
        .map_or(1.0, |m| m.4)
        .max(f64::MIN_POSITIVE);
    (cursor.0 / scale, cursor.1 / scale)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CaptureMode {
    Region,
    Window,
    Screen,
    Previous,
}

/// The countdown lengths offered in the menu.
pub fn valid_timer(seconds: u8) -> bool {
    matches!(seconds, 3 | 5 | 10)
}

/// Marks a capture as running; dropping it (also on error or panic) ends it.
pub struct Busy(Arc<AtomicBool>);

impl Busy {
    /// `None` while another capture is running.
    pub fn start(flag: &Arc<AtomicBool>) -> Option<Self> {
        flag.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| Self(flag.clone()))
    }

    /// The overlays now own the capture: `release` ends it on select, cancel or close.
    pub fn keep(self) {
        std::mem::forget(self);
    }

    pub fn release(flag: &AtomicBool) {
        flag.store(false, Ordering::SeqCst);
    }
}

impl Drop for Busy {
    fn drop(&mut self) {
        Self::release(&self.0);
    }
}

/// Overlays that never called back (their page failed to load) are cancelled.
pub const OVERLAY_WATCHDOG: Duration = Duration::from_secs(10);

pub fn watchdog_expired(opened: Instant, contacted: bool, now: Instant) -> bool {
    !contacted && now.duration_since(opened) >= OVERLAY_WATCHDOG
}

#[derive(Default)]
pub struct CaptureInner {
    pub frozen: Vec<Frozen>,
    pub windows: Vec<WindowInfo>,
    pub previous: Option<PreviousRegion>,
    pub cancel: Arc<AtomicBool>,
    pub result: Option<RasterImage>,
    /// The overlay mode while overlays are open.
    pub mode: Option<CaptureMode>,
    /// Set while any capture runs, from the command until the editor has the result.
    pub busy: Arc<AtomicBool>,
    /// Counts overlay sessions, so a stale watchdog never cancels a newer one.
    pub generation: u64,
    /// An overlay page asked for its info or frame.
    pub contacted: bool,
}

#[derive(Default)]
pub struct CaptureState(pub Mutex<CaptureInner>);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::naming::tests as naming_tests;
    use scribblecap_capture::{
        CaptureError, CapturedImage, Capturer, DisplayInfo, ScreenRect, WindowInfo,
    };

    struct FakeCapturer {
        fail: bool,
    }
    impl FakeCapturer {
        fn two_displays() -> Self {
            Self { fail: false }
        }
        fn failing() -> Self {
            Self { fail: true }
        }
        /// Display `d`'s pixel at (x, y).
        fn pixel(d: u8, x: u8, y: u8) -> [u8; 4] {
            [d, x, y, 255]
        }
        fn list() -> Vec<DisplayInfo> {
            vec![
                DisplayInfo {
                    id: 1,
                    bounds: ScreenRect::new(0.0, 0.0, 1920.0, 1080.0),
                    scale: 1.0,
                },
                DisplayInfo {
                    id: 2,
                    bounds: ScreenRect::new(1920.0, 0.0, 100.0, 50.0),
                    scale: 2.0,
                },
            ]
        }
    }
    impl Capturer for FakeCapturer {
        fn has_permission(&self) -> bool {
            !self.fail
        }
        fn request_permission(&self) -> bool {
            !self.fail
        }
        fn displays(&self) -> scribblecap_capture::Result<Vec<DisplayInfo>> {
            Ok(Self::list())
        }
        fn list_windows(&self) -> scribblecap_capture::Result<Vec<WindowInfo>> {
            Ok(vec![])
        }
        fn capture_screen(
            &self,
            display: Option<u32>,
        ) -> scribblecap_capture::Result<CapturedImage> {
            if self.fail {
                return Err(CaptureError::PermissionDenied);
            }
            let id = display.ok_or(CaptureError::NotFound)?;
            // Display 1: a small 4x4 picture; display 2: 2x its 100x50 points.
            let (w, h, scale) = if id == 1 {
                (4, 4, 1.0)
            } else {
                (200, 100, 2.0)
            };
            let mut pixels = Vec::new();
            for y in 0..h {
                for x in 0..w {
                    pixels.extend_from_slice(&Self::pixel(id as u8, x as u8, y as u8));
                }
            }
            Ok(CapturedImage {
                width: w,
                height: h,
                pixels,
                scale,
            })
        }
        fn capture_window(&self, _: u32) -> scribblecap_capture::Result<CapturedImage> {
            Err(CaptureError::NotFound)
        }
        fn capture_region(&self, _: ScreenRect) -> scribblecap_capture::Result<CapturedImage> {
            Err(CaptureError::NotFound)
        }
    }

    #[test]
    fn editor_is_shown_again_on_error_and_panic() {
        let shown = std::cell::Cell::new(0);
        {
            let _g = EditorHidden::new(|| shown.set(shown.get() + 1));
        }
        assert_eq!(shown.get(), 1);
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _g = EditorHidden::new(|| shown.set(shown.get() + 1));
            panic!("worker failed");
        }));
        assert!(r.is_err());
        assert_eq!(shown.get(), 2);
    }

    #[test]
    fn a_kept_hidden_editor_is_not_shown_by_the_guard() {
        let shown = std::cell::Cell::new(0);
        EditorHidden::new(|| shown.set(shown.get() + 1)).keep_hidden();
        assert_eq!(shown.get(), 0);
    }

    #[test]
    fn freezes_every_display_and_crops_exact_pixels() {
        let frozen = freeze(&FakeCapturer::two_displays()).unwrap();
        assert_eq!(
            frozen.iter().map(|f| f.display.id).collect::<Vec<_>>(),
            [1, 2]
        );
        let image = crop(&frozen[1], ScreenRect::new(1921.0, 1.0, 2.0, 1.0)).unwrap(); // display 2 at x=1920, 2x
        assert_eq!((image.width(), image.height()), (4, 2));
        assert_eq!(&image.pixels()[..4], FakeCapturer::pixel(2, 2, 2));
    }

    #[test]
    fn crop_outside_the_display_is_an_error() {
        let frozen = freeze(&FakeCapturer::two_displays()).unwrap();
        assert!(crop(&frozen[1], ScreenRect::new(0.0, 0.0, 10.0, 10.0)).is_err());
    }

    #[test]
    fn freeze_failure_reports_the_backend_error() {
        assert!(matches!(
            freeze(&FakeCapturer::failing()),
            Err(CaptureError::PermissionDenied)
        ));
    }

    #[test]
    fn capture_save_uses_folder_and_time_name_without_a_dialog() {
        let dir = tempfile::tempdir().unwrap();
        let t = capture_target(dir.path(), naming_tests::at(1)).unwrap();
        assert_eq!(t, dir.path().join("Screenshot 2026-11-24 18.42.07.png"));
        std::fs::write(&t, b"existing").unwrap();
        assert_eq!(
            capture_target(dir.path(), naming_tests::at(1))
                .unwrap()
                .file_name()
                .unwrap(),
            "Screenshot 2026-11-24 18.42.07.001.png"
        );
        assert_eq!(std::fs::read(&t).unwrap(), b"existing");
    }

    #[test]
    fn saving_a_capture_never_overwrites_and_retries_with_a_fresh_time() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Screenshot 2026-11-24 18.42.07.png"), b"a").unwrap();
        std::fs::write(
            dir.path().join("Screenshot 2026-11-24 18.42.07.001.png"),
            b"b",
        )
        .unwrap();
        let mut times = [naming_tests::at(1), naming_tests::at(2)].into_iter();
        let path = save_capture(dir.path(), b"new", || times.next().unwrap()).unwrap();
        assert_eq!(
            path,
            dir.path().join("Screenshot 2026-11-24 18.42.07.002.png")
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
        assert_eq!(
            std::fs::read(dir.path().join("Screenshot 2026-11-24 18.42.07.png")).unwrap(),
            b"a"
        );
    }

    #[test]
    fn saving_into_a_missing_folder_fails_once_with_a_message() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("gone");
        let mut calls = 0;
        let error = save_capture(&missing, b"x", || {
            calls += 1;
            naming_tests::at(calls)
        })
        .unwrap_err();
        assert_eq!(calls, 1);
        assert!(error.contains("capture folder"), "{error}");
    }

    #[test]
    fn cursor_maps_from_physical_pixels_to_desktop_points() {
        // A 2x monitor at physical (0, 0) and a 1x one at physical (2880, 0).
        let monitors = [
            (0.0, 0.0, 2880.0, 1800.0, 2.0),
            (2880.0, 0.0, 1920.0, 1080.0, 1.0),
        ];
        assert_eq!(cursor_points((100.0, 50.0), &monitors), (50.0, 25.0));
        assert_eq!(cursor_points((2890.0, 5.0), &monitors), (2890.0, 5.0));
        assert_eq!(cursor_points((-5.0, 5.0), &monitors), (-5.0, 5.0));
    }

    #[test]
    fn modes_and_timer_values_follow_the_ui_contract() {
        assert_eq!(
            serde_json::from_str::<CaptureMode>("\"previous\"").unwrap(),
            CaptureMode::Previous
        );
        assert!(valid_timer(3) && valid_timer(5) && valid_timer(10));
        assert!(!valid_timer(0) && !valid_timer(4));
    }

    #[test]
    fn only_one_capture_runs_and_the_flag_is_released_on_every_exit() {
        let busy = Arc::new(AtomicBool::new(false));
        let first = Busy::start(&busy).unwrap();
        assert!(Busy::start(&busy).is_none());
        drop(first);
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _b = Busy::start(&busy).unwrap();
            panic!("worker failed");
        }));
        assert!(r.is_err());
        let overlays = Busy::start(&busy).unwrap();
        overlays.keep();
        assert!(Busy::start(&busy).is_none());
        Busy::release(&busy);
        assert!(Busy::start(&busy).is_some());
    }

    #[test]
    fn overlays_that_never_call_back_are_cancelled_after_ten_seconds() {
        let opened = std::time::Instant::now();
        let later = |s| opened + std::time::Duration::from_secs(s);
        assert!(!watchdog_expired(opened, false, later(9)));
        assert!(watchdog_expired(opened, false, later(10)));
        assert!(!watchdog_expired(opened, true, later(60)));
    }
}
