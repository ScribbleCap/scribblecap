//! macOS backend: ScreenCaptureKit (`SCScreenshotManager`, macOS 14+).
//!
//! Calling Objective-C and CoreGraphics needs `unsafe`; it is allowed in this module
//! only. Each block states what it relies on.
#![allow(unsafe_code)]

use std::ptr::NonNull;
use std::sync::mpsc;
use std::time::Duration;

use block2::RcBlock;
use objc2::AnyThread;
use objc2::rc::Retained;
use objc2_core_foundation::{CFRetained, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGColorSpace, CGContext, CGImage, CGImageAlphaInfo,
    CGImageByteOrderInfo, CGMainDisplayID, CGPreflightScreenCaptureAccess,
    CGRequestScreenCaptureAccess, kCGColorSpaceSRGB,
};
use objc2_foundation::{NSArray, NSError, NSOperatingSystemVersion, NSProcessInfo};
use objc2_screen_capture_kit::{
    SCCaptureResolutionType, SCContentFilter, SCDisplay, SCScreenshotManager, SCShareableContent,
    SCStreamConfiguration, SCWindow,
};

use crate::{
    CaptureError, CapturedImage, Capturer, DisplayInfo, Result, ScreenRect, WindowInfo, pixel_size,
    region_on_display, unpremultiply,
};

/// How long to wait for ScreenCaptureKit before giving up.
const TIMEOUT: Duration = Duration::from_secs(10);
/// `SCStreamErrorUserDeclined`: the user hasn't granted Screen Recording.
const USER_DECLINED: isize = -3801;

pub(crate) struct MacCapturer;

impl MacCapturer {
    pub(crate) fn new() -> Result<Self> {
        let min = NSOperatingSystemVersion {
            majorVersion: 14,
            minorVersion: 0,
            patchVersion: 0,
        };
        if !NSProcessInfo::processInfo().isOperatingSystemAtLeastVersion(min) {
            return Err(CaptureError::Unsupported("needs macOS 14 or later"));
        }
        Ok(Self)
    }
}

impl Capturer for MacCapturer {
    fn has_permission(&self) -> bool {
        CGPreflightScreenCaptureAccess()
    }

    fn request_permission(&self) -> bool {
        CGRequestScreenCaptureAccess()
    }

    fn displays(&self) -> Result<Vec<DisplayInfo>> {
        let content = shareable_content()?;
        // SAFETY: plain property reads on a valid SCShareableContent.
        let displays = unsafe { content.displays() };
        Ok(displays.iter().map(|d| display_info(&d)).collect())
    }

    fn list_windows(&self) -> Result<Vec<WindowInfo>> {
        let content = shareable_content()?;
        // SAFETY: plain property reads on valid SCShareableContent/SCWindow objects.
        let windows = unsafe { content.windows() };
        Ok(windows
            .iter()
            .filter(|w| is_listed(w))
            .map(|w| unsafe {
                WindowInfo {
                    id: w.windowID(),
                    title: w.title().map(|t| t.to_string()).unwrap_or_default(),
                    app_name: w
                        .owningApplication()
                        .map(|a| a.applicationName().to_string())
                        .unwrap_or_default(),
                    bounds: rect(w.frame()),
                }
            })
            .collect())
    }

    fn capture_screen(&self, display: Option<u32>) -> Result<CapturedImage> {
        let content = shareable_content()?;
        let id = display.unwrap_or_else(|| CGMainDisplayID());
        // SAFETY: property read on valid SCShareableContent.
        let displays = unsafe { content.displays() };
        let display = displays
            .iter()
            .find(|d| unsafe { d.displayID() } == id)
            .ok_or(CaptureError::NotFound)?;
        let info = display_info(&display);
        let filter = display_filter(&content, &display);
        let config = configuration(info.bounds.width, info.bounds.height, info.scale);
        capture(&filter, &config, info.scale)
    }

    fn capture_window(&self, id: u32) -> Result<CapturedImage> {
        let content = shareable_content()?;
        // SAFETY: property reads on valid SCShareableContent/SCWindow objects.
        let windows = unsafe { content.windows() };
        let window = windows
            .iter()
            .find(|w| unsafe { w.windowID() } == id)
            .ok_or(CaptureError::NotFound)?;
        // SAFETY: `window` comes from the current shareable content.
        let filter = unsafe {
            SCContentFilter::initWithDesktopIndependentWindow(SCContentFilter::alloc(), &window)
        };
        // SAFETY: infoForFilter (macOS 14+, checked in `new`) on a valid filter.
        let info = unsafe { SCShareableContent::infoForFilter(&filter) };
        let (frame, scale) = unsafe { (info.contentRect(), f64::from(info.pointPixelScale())) };
        let config = configuration(frame.size.width, frame.size.height, scale);
        capture(&filter, &config, scale)
    }

    fn capture_region(&self, region: ScreenRect) -> Result<CapturedImage> {
        let content = shareable_content()?;
        // SAFETY: property read on valid SCShareableContent.
        let displays: Vec<Retained<SCDisplay>> = unsafe { content.displays() }.iter().collect();
        let infos: Vec<DisplayInfo> = displays.iter().map(|d| display_info(d)).collect();
        let (info, clipped) = region_on_display(&region, &infos).ok_or(CaptureError::NotFound)?;
        let display = displays
            .iter()
            .find(|d| unsafe { d.displayID() } == info.id)
            .ok_or(CaptureError::NotFound)?;
        let filter = display_filter(&content, display);
        let config = configuration(clipped.width, clipped.height, info.scale);
        let source = CGRect::new(
            CGPoint::new(clipped.x - info.bounds.x, clipped.y - info.bounds.y),
            CGSize::new(clipped.width, clipped.height),
        );
        // SAFETY: setter on a configuration we own; the rect is in display-local points.
        unsafe { config.setSourceRect(source) };
        capture(&filter, &config, info.scale)
    }
}

/// ScreenCaptureKit objects are reference-counted Objective-C objects that the
/// framework itself hands across its own queues; we only move them from the callback
/// queue to the waiting thread and never use them concurrently.
struct SendPtr<T>(T);
// SAFETY: see above.
unsafe impl<T> Send for SendPtr<T> {}

fn shareable_content() -> Result<Retained<SCShareableContent>> {
    if !CGPreflightScreenCaptureAccess() {
        return Err(CaptureError::PermissionDenied);
    }
    let (tx, rx) = mpsc::sync_channel(1);
    let handler = RcBlock::new(
        move |content: *mut SCShareableContent, error: *mut NSError| {
            // SAFETY: ScreenCaptureKit passes either a valid object or null; retaining keeps
            // it alive after the callback returns.
            let result = match unsafe { Retained::retain(content) } {
                Some(content) => Ok(SendPtr(content)),
                None => Err(os_error(error)),
            };
            let _ = tx.send(result);
        },
    );
    // SAFETY: the handler matches the documented block signature.
    unsafe {
        SCShareableContent::getShareableContentExcludingDesktopWindows_onScreenWindowsOnly_completionHandler(
            true, true, &handler,
        );
    }
    wait(rx).map(|c| c.0)
}

fn capture(
    filter: &SCContentFilter,
    config: &SCStreamConfiguration,
    scale: f64,
) -> Result<CapturedImage> {
    let (tx, rx) = mpsc::sync_channel(1);
    let handler = RcBlock::new(move |image: *mut CGImage, error: *mut NSError| {
        let result = match NonNull::new(image) {
            // SAFETY: a non-null CGImage from ScreenCaptureKit; retain it past the
            // callback.
            Some(image) => Ok(SendPtr(unsafe { CFRetained::retain(image) })),
            None => Err(os_error(error)),
        };
        let _ = tx.send(result);
    });
    // SAFETY: valid filter and configuration; macOS 14+ is checked in `new`.
    unsafe {
        SCScreenshotManager::captureImageWithFilter_configuration_completionHandler(
            filter,
            config,
            Some(&handler),
        );
    }
    let image = wait(rx)?.0;
    to_rgba(&image, scale)
}

fn wait<T>(rx: mpsc::Receiver<Result<T>>) -> Result<T> {
    rx.recv_timeout(TIMEOUT)
        .map_err(|_| CaptureError::Os("ScreenCaptureKit did not answer".into()))?
}

fn os_error(error: *mut NSError) -> CaptureError {
    // SAFETY: ScreenCaptureKit passes a valid NSError or null.
    match unsafe { error.as_ref() } {
        Some(e) if e.code() == USER_DECLINED => CaptureError::PermissionDenied,
        Some(e) => CaptureError::Os(e.localizedDescription().to_string()),
        None => CaptureError::Os("unknown ScreenCaptureKit error".into()),
    }
}

fn display_info(display: &SCDisplay) -> DisplayInfo {
    // SAFETY: plain property reads on a valid SCDisplay.
    let (id, frame) = unsafe { (display.displayID(), display.frame()) };
    DisplayInfo {
        id,
        bounds: rect(frame),
        scale: display_scale(id, frame.size.width).unwrap_or(1.0),
    }
}

/// Pixels per point of a display, from its current mode.
fn display_scale(id: u32, width_points: f64) -> Option<f64> {
    let mode = objc2_core_graphics::CGDisplayCopyDisplayMode(id)?;
    let pixels = objc2_core_graphics::CGDisplayMode::pixel_width(Some(&mode)) as f64;
    (width_points > 0.0 && pixels > 0.0).then(|| pixels / width_points)
}

/// The whole display minus this app's own windows.
fn display_filter(content: &SCShareableContent, display: &SCDisplay) -> Retained<SCContentFilter> {
    let own = std::process::id() as i32;
    // SAFETY: property reads on valid objects; the filter keeps its own references.
    unsafe {
        let ours: Vec<Retained<SCWindow>> = content
            .windows()
            .iter()
            .filter(|w| w.owningApplication().is_some_and(|a| a.processID() == own))
            .collect();
        let excluded = NSArray::from_retained_slice(&ours);
        SCContentFilter::initWithDisplay_excludingWindows(
            SCContentFilter::alloc(),
            display,
            &excluded,
        )
    }
}

fn configuration(width: f64, height: f64, scale: f64) -> Retained<SCStreamConfiguration> {
    // SAFETY: setters on a configuration we own.
    unsafe {
        let config = SCStreamConfiguration::new();
        config.setWidth(pixel_size(width, scale));
        config.setHeight(pixel_size(height, scale));
        config.setShowsCursor(false);
        config.setCaptureResolution(SCCaptureResolutionType::Best);
        config
    }
}

/// Normal app windows that are on screen, have a size, and aren't ours.
fn is_listed(window: &SCWindow) -> bool {
    let own = std::process::id() as i32;
    // SAFETY: plain property reads on a valid SCWindow.
    unsafe {
        let frame = window.frame();
        window.isOnScreen()
            && window.windowLayer() == 0
            && frame.size.width > 1.0
            && frame.size.height > 1.0
            && window
                .owningApplication()
                .is_some_and(|a| a.processID() != own)
    }
}

fn rect(r: CGRect) -> ScreenRect {
    ScreenRect::new(r.origin.x, r.origin.y, r.size.width, r.size.height)
}

/// Draw a CGImage into an sRGB RGBA8 buffer (converting from the display's colour
/// space) and return it with straight alpha.
fn to_rgba(image: &CGImage, scale: f64) -> Result<CapturedImage> {
    let (w, h) = (CGImage::width(Some(image)), CGImage::height(Some(image)));
    let failed = || CaptureError::Os("could not convert the captured image".into());
    if w == 0 || h == 0 {
        return Err(failed());
    }
    let mut pixels = vec![0u8; w * h * 4];
    // SAFETY: kCGColorSpaceSRGB is a valid constant name.
    let srgb = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB })).ok_or_else(failed)?;
    let info = CGImageAlphaInfo::PremultipliedLast.0 | CGImageByteOrderInfo::Order32Big.0;
    // SAFETY: `pixels` is exactly w × h × 4 bytes, outlives the context, and the
    // bytes-per-row match.
    let context: CFRetained<CGContext> = unsafe {
        CGBitmapContextCreate(
            pixels.as_mut_ptr().cast(),
            w,
            h,
            8,
            w * 4,
            Some(&srgb),
            info,
        )
    }
    .ok_or_else(failed)?;
    let bounds = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(w as f64, h as f64));
    CGContext::draw_image(Some(&context), bounds, Some(image));
    drop(context);
    unpremultiply(&mut pixels);
    Ok(CapturedImage {
        width: u32::try_from(w).map_err(|_| failed())?,
        height: u32::try_from(h).map_err(|_| failed())?,
        pixels,
        scale,
    })
}
