//! Capture inside the app: Tauri commands and windows around the pure flow in
//! `session`. OS capture code stays in `scribblecap-capture`.

pub mod geometry;
pub mod naming;
pub mod session;

use std::sync::atomic::Ordering;
use std::time::Duration;

use scribblecap_capture::{CaptureError, Capturer, ScreenRect, WindowInfo};
use scribblecap_formats::RasterImage;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, ipc::Response};

use geometry::{PreviousRegion, display_at};
use session::{
    Busy, CaptureMode, CaptureState, EditorHidden, Frozen, HIDE_SETTLE_MS, OVERLAY_WATCHDOG, crop,
    cursor_points, freeze, raster, save_capture, valid_timer, watchdog_expired,
};

const OVERLAY_PREFIX: &str = "capture-overlay-";
const COUNTDOWN: &str = "capture-countdown";
const BADGE_SIZE: f64 = 120.0;
const BADGE_MARGIN: f64 = 24.0;
#[cfg(target_os = "macos")]
const PERMISSION: &str = "Screen capture needs permission. Allow ScribbleCap in System Settings → Privacy & Security → Screen & System Audio Recording, then try again.";

/// A rectangle in desktop points, as the UI sends and receives it.
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl From<ScreenRect> for Rect {
    fn from(r: ScreenRect) -> Self {
        Self {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
        }
    }
}
impl From<Rect> for ScreenRect {
    fn from(r: Rect) -> Self {
        ScreenRect::new(r.x, r.y, r.width, r.height)
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowOutline {
    pub id: u32,
    pub title: String,
    pub app_name: String,
    pub bounds: Rect,
}
impl From<&WindowInfo> for WindowOutline {
    fn from(w: &WindowInfo) -> Self {
        Self {
            id: w.id,
            title: w.title.clone(),
            app_name: w.app_name.clone(),
            bounds: w.bounds.into(),
        }
    }
}

#[derive(serde::Serialize)]
pub struct CaptureAvailability {
    pub wayland: bool,
    pub previous: bool,
}

#[derive(serde::Serialize)]
pub struct OverlayInfo {
    pub bounds: Rect,
    pub windows: Vec<WindowOutline>,
    pub mode: CaptureMode,
}

fn lock(app: &AppHandle) -> Result<std::sync::MutexGuard<'_, session::CaptureInner>, String> {
    app.state::<CaptureState>()
        .inner()
        .0
        .lock()
        .map_err(|_| "The capture is unavailable.".to_string())
}

/// A selection that arrives after the capture ended (cancelled, timed out) must have no side effects.
fn require_active(mode: Option<CaptureMode>) -> Result<CaptureMode, String> {
    mode.ok_or_else(|| "This capture has ended.".to_string())
}

fn message(error: CaptureError) -> String {
    match error {
        #[cfg(target_os = "macos")]
        CaptureError::PermissionDenied => PERMISSION.into(),
        #[cfg(not(target_os = "macos"))]
        CaptureError::PermissionDenied => {
            "Screen capture was not allowed or was cancelled. Allow screen capture for ScribbleCap and try again.".into()
        }
        other => format!("The screen could not be captured: {other}."),
    }
}

fn backend() -> Result<Box<dyn Capturer>, String> {
    let capturer = scribblecap_capture::backend().map_err(message)?;
    #[cfg(target_os = "macos")]
    if !capturer.has_permission() && !capturer.request_permission() {
        return Err(PERMISSION.into());
    }
    Ok(capturer)
}

fn show_editor(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Hide only the editor window (never the whole app) and wait until it is gone.
fn hide_editor(app: &AppHandle) -> EditorHidden<impl FnMut() + use<>> {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    let shown = app.clone();
    let guard = EditorHidden::new(move || show_editor(&shown));
    std::thread::sleep(Duration::from_millis(HIDE_SETTLE_MS));
    guard
}

fn close_capture_windows(app: &AppHandle) {
    for (label, window) in app.webview_windows() {
        if label.starts_with(OVERLAY_PREFIX) || label == COUNTDOWN {
            let _ = window.close();
        }
    }
}

/// The pointer in desktop points.
fn pointer(app: &AppHandle) -> Option<(f64, f64)> {
    let cursor = app.cursor_position().ok()?;
    let monitors: Vec<_> = app
        .available_monitors()
        .ok()?
        .iter()
        .map(|m| {
            let (p, s) = (m.position(), m.size());
            (
                f64::from(p.x),
                f64::from(p.y),
                f64::from(s.width),
                f64::from(s.height),
                m.scale_factor(),
            )
        })
        .collect();
    Some(cursor_points((cursor.x, cursor.y), &monitors))
}

/// Freeze every display and open one overlay per display. The editor stays
/// hidden until a selection or a cancel.
fn open_overlays<F: FnMut()>(
    app: &AppHandle,
    capturer: &dyn Capturer,
    mode: CaptureMode,
    hidden: EditorHidden<F>,
    busy: Busy,
) -> Result<Option<RasterImage>, String> {
    let frozen = freeze(capturer).map_err(message)?;
    let windows = if mode == CaptureMode::Window {
        capturer.list_windows().unwrap_or_default()
    } else {
        Vec::new()
    };
    let displays: Vec<_> = frozen.iter().map(|f| f.display.clone()).collect();
    {
        let mut state = lock(app)?;
        state.frozen = frozen;
        state.windows = windows;
        state.mode = Some(mode);
        state.result = None;
        state.contacted = false;
        state.generation += 1;
        if state.cancel.load(Ordering::SeqCst) {
            // Cancelled while freezing: the cancel already ran, so do not open overlays.
            drop(state);
            discard(app);
            return Ok(None); // dropping `hidden` shows the editor, `busy` frees the slot
        }
    }
    for display in displays {
        let b = display.bounds;
        let built = WebviewWindowBuilder::new(
            app,
            format!("{OVERLAY_PREFIX}{}", display.id),
            WebviewUrl::App(format!("index.html?overlay={}", display.id).into()),
        )
        .title("Capture")
        .decorations(false)
        .always_on_top(true)
        .resizable(false)
        .skip_taskbar(true)
        .shadow(false)
        .position(b.x, b.y)
        .inner_size(b.width, b.height)
        .focused(true)
        .build();
        if let Err(e) = built {
            discard(app);
            close_capture_windows(app);
            return Err(format!("The capture overlay could not be opened: {e}"));
        }
    }
    hidden.keep_hidden();
    busy.keep();
    start_watchdog(app);
    Ok(None)
}

/// Cancel overlays whose page never called back, so the editor never stays hidden.
fn start_watchdog(app: &AppHandle) {
    let Ok(generation) = lock(app).map(|s| s.generation) else {
        return;
    };
    let opened = std::time::Instant::now();
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(OVERLAY_WATCHDOG);
        let expired = lock(&app).is_ok_and(|s| {
            s.generation == generation
                && s.mode.is_some()
                && watchdog_expired(opened, s.contacted, std::time::Instant::now())
        });
        if expired {
            cancel(&app);
        }
    });
}

/// Drop frozen pixels and window outlines.
fn discard(app: &AppHandle) {
    if let Ok(mut state) = lock(app) {
        state.frozen.clear();
        state.windows.clear();
        state.mode = None;
        state.result = None;
    }
}

/// Capture with the editor already hidden. `None` means overlays are open.
fn capture_hidden<F: FnMut()>(
    app: &AppHandle,
    mode: CaptureMode,
    hidden: EditorHidden<F>,
    busy: Busy,
) -> Result<Option<RasterImage>, String> {
    let capturer = backend()?;
    if scribblecap_capture::is_wayland() {
        // The portal dialog decides what is captured; cropping is done in the editor.
        let image = capturer.capture_screen(None).map_err(message)?;
        return raster(image).map(Some);
    }
    match mode {
        CaptureMode::Screen => {
            let displays = capturer.displays().map_err(message)?;
            let display = pointer(app)
                .and_then(|(x, y)| display_at(&displays, x, y))
                .or(displays.first())
                .ok_or("No display was found to capture.")?;
            let image = capturer.capture_screen(Some(display.id)).map_err(message)?;
            raster(image).map(Some)
        }
        CaptureMode::Previous => {
            let displays = capturer.displays().map_err(message)?;
            let previous = lock(app)?
                .previous
                .clone()
                .filter(|p| p.still_valid(&displays));
            let Some(previous) = previous else {
                return open_overlays(app, capturer.as_ref(), CaptureMode::Region, hidden, busy);
            };
            let image = capturer
                .capture_screen(Some(previous.display.id))
                .map_err(message)?;
            let frozen = Frozen {
                display: previous.display,
                image,
            };
            crop(&frozen, previous.rect).map(Some)
        }
        CaptureMode::Region | CaptureMode::Window => {
            open_overlays(app, capturer.as_ref(), mode, hidden, busy)
        }
    }
}

fn now() -> chrono::NaiveDateTime {
    chrono::Local::now().naive_local()
}

/// Open a finished capture in the editor session, auto-saving it when enabled.
/// Blocks on the folder picker: call off the main thread.
fn finish(app: &AppHandle, image: RasterImage) -> Result<Vec<u8>, String> {
    let title = naming::capture_title(now());
    crate::shell::remember(app, None, image.clone(), None)?;
    let auto_save = app
        .state::<crate::settings::SettingsState>()
        .0
        .lock()
        .map_err(|_| "The settings are unavailable.")?
        .auto_save;
    let mut target = None;
    if auto_save && let Some(folder) = crate::settings::ensure_capture_folder(app)? {
        // An editable PNG with no annotations yet (M2-9 rule).
        let saved = crate::output::editable_png(&image)
            .and_then(|bytes| save_capture(&folder, &bytes, now));
        match saved {
            Ok(path) => target = Some(path),
            // The capture still opens; the UI shows why it was not saved.
            Err(e) => {
                let _ = app.emit_to("main", "capture-save-failed", e);
            }
        }
    }
    let shown = target
        .as_ref()
        .and_then(|p| p.file_name())
        .map_or_else(|| title.clone(), |n| n.to_string_lossy().into_owned());
    {
        let session = app.state::<crate::shell::ImageSession>();
        let mut session = session
            .0
            .lock()
            .map_err(|_| "The image session is unavailable.")?;
        session.capture_name = Some(title.clone());
        session.target = target;
    }
    crate::shell::set_title(app, &shown);
    crate::shell::packet(&title, image)
}

/// Claim the single capture slot; a running capture's cancel flag is left alone.
fn starting(app: &AppHandle) -> Result<Busy, String> {
    let state = lock(app)?;
    let busy = Busy::start(&state.busy).ok_or("A capture is already in progress.")?;
    state.cancel.store(false, Ordering::SeqCst);
    Ok(busy)
}

#[tauri::command]
pub fn shell_capture_state(app: AppHandle) -> Result<CaptureAvailability, String> {
    Ok(CaptureAvailability {
        wayland: scribblecap_capture::is_wayland(),
        previous: lock(&app)?.previous.is_some(),
    })
}

#[tauri::command]
pub async fn shell_capture(app: AppHandle, mode: CaptureMode) -> Result<Response, String> {
    let busy = starting(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let image = {
            let hidden = hide_editor(&app);
            capture_hidden(&app, mode, hidden, busy)?
        };
        // An empty packet: overlays are open (the result follows `capture-ready`)
        // or the timer was cancelled.
        match image {
            Some(image) => finish(&app, image).map(Response::new),
            None => Ok(Response::new(Vec::new())),
        }
    })
    .await
    .map_err(|e| format!("The capture stopped: {e}"))?
}

#[tauri::command]
pub async fn shell_capture_timer(app: AppHandle, seconds: u8) -> Result<Response, String> {
    if !valid_timer(seconds) {
        return Err("The timer can be 3, 5 or 10 seconds.".into());
    }
    let busy = starting(&app)?;
    let cancel = lock(&app)?.cancel.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let image = {
            let hidden = hide_editor(&app);
            let area = pointer(&app)
                .and_then(|(x, y)| app.monitor_from_point(x, y).ok().flatten())
                .or_else(|| app.primary_monitor().ok().flatten());
            let mut badge = WebviewWindowBuilder::new(
                &app,
                COUNTDOWN,
                WebviewUrl::App(format!("index.html?countdown={seconds}").into()),
            )
            .title("Capture timer")
            .decorations(false)
            .always_on_top(true)
            .resizable(false)
            .skip_taskbar(true)
            .inner_size(BADGE_SIZE, BADGE_SIZE)
            .focused(true);
            if let Some(m) = area {
                let s = m.scale_factor();
                let (p, z) = (m.position(), m.size());
                badge = badge.position(
                    (f64::from(p.x) + f64::from(z.width)) / s - BADGE_SIZE - BADGE_MARGIN,
                    (f64::from(p.y) + f64::from(z.height)) / s - BADGE_SIZE - BADGE_MARGIN,
                );
            }
            badge
                .build()
                .map_err(|e| format!("The capture timer could not be shown: {e}"))?;
            if let Ok(mut state) = lock(&app) {
                state.counting = true;
            }
            for _ in 0..u32::from(seconds) * 10 {
                if cancel.load(Ordering::SeqCst) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            // The badge closes normally from here; that is not a cancel.
            if let Ok(mut state) = lock(&app) {
                state.counting = false;
            }
            if let Some(window) = app.get_webview_window(COUNTDOWN) {
                let _ = window.close();
            }
            if cancel.load(Ordering::SeqCst) {
                return Ok(Response::new(Vec::new()));
            }
            std::thread::sleep(Duration::from_millis(HIDE_SETTLE_MS));
            capture_hidden(&app, CaptureMode::Region, hidden, busy)?
        };
        // An empty packet: overlays are open (the result follows `capture-ready`)
        // or the timer was cancelled.
        match image {
            Some(image) => finish(&app, image).map(Response::new),
            None => Ok(Response::new(Vec::new())),
        }
    })
    .await
    .map_err(|e| format!("The capture stopped: {e}"))?
}

#[tauri::command]
pub fn shell_overlay_info(app: AppHandle, display: u32) -> Result<OverlayInfo, String> {
    let mut state = lock(&app)?;
    state.contacted = true;
    let frozen = state
        .frozen
        .iter()
        .find(|f| f.display.id == display)
        .ok_or("This capture has ended.")?;
    let bounds = frozen.display.bounds;
    Ok(OverlayInfo {
        bounds: bounds.into(),
        windows: state
            .windows
            .iter()
            .filter(|w| w.bounds.intersect(&bounds).is_some())
            .map(WindowOutline::from)
            .collect(),
        mode: state.mode.ok_or("This capture has ended.")?,
    })
}

#[tauri::command]
pub async fn shell_overlay_frame(app: AppHandle, display: u32) -> Result<Response, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut state = lock(&app)?;
        state.contacted = true;
        let image = &state
            .frozen
            .iter()
            .find(|f| f.display.id == display)
            .ok_or("This capture has ended.")?
            .image;
        crate::shell::packet_from("", image.width, image.height, &image.pixels).map(Response::new)
    })
    .await
    .map_err(|e| format!("The capture stopped: {e}"))?
}

#[tauri::command]
pub async fn shell_overlay_select(app: AppHandle, display: u32, rect: Rect) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let rect = ScreenRect::from(rect);
        let cropped = {
            let mut state = lock(&app)?;
            require_active(state.mode)?;
            let cropped = state
                .frozen
                .iter()
                .find(|f| f.display.id == display)
                .ok_or_else(|| "This capture has ended.".to_string())
                .and_then(|f| crop(f, rect).map(|image| (image, f.display.clone())));
            if let Ok((image, shown_on)) = &cropped {
                if state.mode == Some(CaptureMode::Region) {
                    state.previous = Some(PreviousRegion {
                        display: shown_on.clone(),
                        rect,
                    });
                }
                state.result = Some(image.clone());
            }
            state.frozen.clear();
            state.windows.clear();
            state.mode = None;
            Busy::release(&state.busy);
            cropped
        };
        close_capture_windows(&app);
        show_editor(&app);
        match cropped {
            Ok(_) => {
                let _ = app.emit_to("main", "capture-ready", ());
                Ok(())
            }
            Err(e) => {
                let _ = app.emit_to("main", "capture-cancelled", ());
                Err(e)
            }
        }
    })
    .await
    .map_err(|e| format!("The capture stopped: {e}"))?
}

#[tauri::command]
pub async fn shell_capture_result(app: AppHandle) -> Result<Response, String> {
    let image = lock(&app)?
        .result
        .take()
        .ok_or("There is no capture to open.")?;
    tauri::async_runtime::spawn_blocking(move || finish(&app, image).map(Response::new))
        .await
        .map_err(|e| format!("The capture stopped: {e}"))?
}

fn cancel(app: &AppHandle) {
    if let Ok(mut state) = lock(app) {
        state.cancel.store(true, Ordering::SeqCst);
        state.counting = false;
        // Overlays own the busy flag; a running worker (timer) releases its own.
        if state.mode.is_some() {
            Busy::release(&state.busy);
        }
    }
    discard(app);
    close_capture_windows(app);
    show_editor(app);
    let _ = app.emit_to("main", "capture-cancelled", ());
}

#[tauri::command]
pub fn shell_capture_cancel(app: AppHandle) {
    cancel(&app);
}

/// Whether destroying the window with this label ends the capture: an overlay
/// while overlays are open, or the badge while the timer is still waiting.
fn destroy_cancels(label: &str, overlays_open: bool, counting: bool) -> bool {
    (label.starts_with(OVERLAY_PREFIX) && overlays_open) || (label == COUNTDOWN && counting)
}

/// An overlay closed by the system (not by a selection) cancels the capture,
/// so the editor never stays hidden.
pub fn window_destroyed(window: &tauri::Window) {
    let app = window.app_handle();
    let (overlays, counting) = lock(app).map_or((false, false), |s| (s.mode.is_some(), s.counting));
    if destroy_cancels(window.label(), overlays, counting) {
        cancel(app);
    }
}

#[cfg(test)]
mod select_tests {
    use super::*;

    #[test]
    fn a_select_with_no_active_capture_is_refused() {
        assert_eq!(
            require_active(None),
            Err("This capture has ended.".to_string())
        );
        assert_eq!(
            require_active(Some(CaptureMode::Region)),
            Ok(CaptureMode::Region)
        );
    }

    #[test]
    fn only_capture_windows_cancel_when_destroyed() {
        assert!(destroy_cancels("capture-overlay-1", true, false));
        assert!(!destroy_cancels("capture-overlay-1", false, false));
        assert!(destroy_cancels(COUNTDOWN, false, true));
        assert!(!destroy_cancels(COUNTDOWN, false, false)); // the timer ended normally
        assert!(!destroy_cancels("main", true, true));
    }
}
