//! Tray, shortcut and single-instance requests share one guarded editor route.
use crate::capture::session::CaptureMode;
use std::{
    collections::VecDeque,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tauri::{
    AppHandle, Emitter, Manager,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Action {
    Capture { mode: CaptureMode, delay: u8 },
    Settings,
    Quit,
    Show,
    Error { message: String },
}
#[derive(Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub tray_available: bool,
    pub hotkey_available: bool,
    pub hotkey_error: Option<String>,
    pub hotkey_description: Option<String>,
    pub tray_note: Option<String>,
    pub platform: String,
}
#[derive(Default)]
pub struct Runtime {
    ready: AtomicBool,
    pub exiting: AtomicBool,
    pending: Mutex<VecDeque<Action>>,
    pub status: Mutex<Status>,
    registered: Mutex<Option<Shortcut>>,
    previous: Mutex<Option<MenuItem<tauri::Wry>>>,
    #[cfg(target_os = "linux")]
    pub portal_task: Mutex<Option<crate::background_linux::Binding>>,
}

pub fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Option<Action>, String> {
    let args: Vec<_> = args
        .into_iter()
        .filter(|a| a != "--" && !a.starts_with("-psn_"))
        .collect();
    if args.is_empty() {
        return Ok(None);
    }
    let usage = "Usage: scribblecap-app capture [region|window|screen|previous] [--delay N], where N is 0–60 seconds";
    if args[0] != "capture" {
        return Err(usage.into());
    }
    let mut mode = CaptureMode::Region;
    let mut delay = 0;
    let mut index = 1;
    if let Some(name) = args.get(index).filter(|s| !s.starts_with('-')) {
        mode = match name.as_str() {
            "region" => CaptureMode::Region,
            "window" => CaptureMode::Window,
            "screen" => CaptureMode::Screen,
            "previous" => CaptureMode::Previous,
            _ => return Err(usage.into()),
        };
        index += 1;
    }
    if index < args.len() {
        if args[index] != "--delay" || index + 2 != args.len() {
            return Err(usage.into());
        }
        delay = args[index + 1]
            .parse::<u8>()
            .ok()
            .filter(|n| *n <= 60)
            .ok_or(usage)?;
    }
    Ok(Some(Action::Capture { mode, delay }))
}

pub fn show(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}
pub fn request(app: &AppHandle, action: Action) {
    let state = app.state::<Runtime>();
    if matches!(
        action,
        Action::Show | Action::Settings | Action::Quit | Action::Error { .. }
    ) {
        show(app);
    }
    if let Ok(mut pending) = state.pending.lock() {
        // Bound requests while the page starts; UI work is serialized thereafter.
        if pending.len() < 16 {
            pending.push_back(action);
        }
    }
    flush(app);
}
fn flush(app: &AppHandle) {
    let state = app.state::<Runtime>();
    if !state.ready.load(Ordering::SeqCst) {
        return;
    }
    if let Ok(mut pending) = state.pending.lock() {
        while let Some(action) = pending.pop_front() {
            if app.emit_to("main", "background-action", &action).is_err() {
                pending.push_front(action);
                break;
            }
        }
    }
}
#[tauri::command]
pub fn shell_background_ready(app: AppHandle) {
    app.state::<Runtime>().ready.store(true, Ordering::SeqCst);
    flush(&app);
}
#[tauri::command]
pub fn shell_background_status(app: AppHandle) -> Result<Status, String> {
    app.state::<Runtime>()
        .status
        .lock()
        .map(|s| s.clone())
        .map_err(|_| "Background status is unavailable".into())
}
#[tauri::command]
pub fn shell_show_editor(app: AppHandle) {
    show(&app);
}
#[tauri::command]
pub fn shell_finish_quit(app: AppHandle) {
    app.state::<Runtime>().exiting.store(true, Ordering::SeqCst);
    crate::capture::shell_capture_cancel(app.clone());
    app.exit(0);
}
#[tauri::command]
pub async fn shell_background_capture(
    app: AppHandle,
    mode: CaptureMode,
    delay: u8,
) -> Result<tauri::ipc::Response, String> {
    if delay > 60 {
        return Err("The delay must be between 0 and 60 seconds".into());
    }
    if delay == 0 {
        crate::capture::shell_capture(app, mode).await
    } else {
        crate::capture::capture_delayed(app, mode, delay).await
    }
}
pub fn close_to_tray(keep_running: bool, tray_available: bool) -> bool {
    keep_running && tray_available
}
pub fn close_requested(app: &AppHandle) {
    let keep = app
        .state::<crate::settings::SettingsState>()
        .0
        .lock()
        .map(|s| s.keep_running)
        .unwrap_or(false);
    let tray = app
        .state::<Runtime>()
        .status
        .lock()
        .map(|s| s.tray_available)
        .unwrap_or(false);
    if close_to_tray(keep, tray) {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.hide();
        }
    } else {
        request(app, Action::Quit);
    }
}
pub fn set_hotkey_error(app: &AppHandle, error: Option<String>, available: bool) {
    if let Ok(mut status) = app.state::<Runtime>().status.lock() {
        status.hotkey_available = available;
        status.hotkey_error = error;
        let _ = app.emit_to("main", "background-status", status.clone());
    }
}
pub async fn register_hotkey(app: &AppHandle, value: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    if scribblecap_capture::is_wayland() {
        return crate::background_linux::bind(app, value).await;
    }
    let shortcut = value
        .parse::<Shortcut>()
        .map_err(|e| format!("Invalid shortcut: {e}"))?;
    let state = app.state::<Runtime>();
    let mut active = state
        .registered
        .lock()
        .map_err(|_| "Shortcut state is unavailable")?;
    if active.as_ref() == Some(&shortcut) {
        set_hotkey_error(app, None, true);
        return Ok(());
    }
    // Register first: an occupied new shortcut never unregisters the working old one.
    app.global_shortcut()
        .on_shortcut(shortcut, |app, _, event| {
            if event.state == ShortcutState::Pressed {
                request(
                    app,
                    Action::Capture {
                        mode: CaptureMode::Region,
                        delay: 0,
                    },
                );
            }
        })
        .map_err(|e| {
            format!("The shortcut could not be registered (it may already be in use): {e}")
        })?;
    if let Some(old) = active.take() {
        let _ = app.global_shortcut().unregister(old);
    }
    *active = Some(shortcut);
    if let Ok(mut status) = state.status.lock() {
        status.hotkey_description = Some(value.into());
    }
    set_hotkey_error(app, None, true);
    Ok(())
}

pub fn refresh_previous(app: &AppHandle) {
    let available = crate::capture::shell_capture_state(app.clone()).ok();
    if let Ok(item) = app.state::<Runtime>().previous.lock()
        && let Some(item) = item.as_ref()
    {
        let _ = item.set_enabled(available.is_some_and(|a| a.previous && !a.wayland));
    }
}
fn tray_image(dark: bool) -> tauri::image::Image<'static> {
    // Original monochrome capture corners and loop-arrow, derived from our SVG.
    let lines: &[&[(f64, f64)]] = &[
        &[(5., 11.), (5., 5.), (11., 5.)],
        &[(21., 5.), (27., 5.), (27., 11.)],
        &[(27., 21.), (27., 27.), (21., 27.)],
        &[(11., 27.), (5., 27.), (5., 21.)],
        &[
            (10., 22.),
            (13., 22.),
            (17., 20.),
            (18., 17.),
            (17., 15.),
            (15., 15.),
            (14., 17.),
            (16., 19.),
            (19., 18.),
            (22., 12.),
        ],
        &[(18., 13.), (22., 11.), (23., 16.)],
    ];
    let mut pixels = vec![0; 32 * 32 * 4];
    for y in 0..32 {
        for x in 0..32 {
            let p = (x as f64 + 0.5, y as f64 + 0.5);
            let mut distance = f64::MAX;
            for line in lines {
                for pair in line.windows(2) {
                    let (a, b) = (pair[0], pair[1]);
                    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                    let t =
                        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / (dx * dx + dy * dy)).clamp(0., 1.);
                    distance = distance.min((p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy));
                }
            }
            let i = (y * 32 + x) * 4;
            let c = if dark { 255 } else { 0 };
            pixels[i..i + 3].fill(c);
            pixels[i + 3] = ((1.6 - distance).clamp(0., 1.) * 255.).round() as u8;
        }
    }
    tauri::image::Image::new_owned(pixels, 32, 32)
}
pub fn install_tray(app: &AppHandle, available: bool) -> Result<(), String> {
    if !available {
        return Ok(());
    }
    let item = |id: &str, label: &str, enabled: bool| {
        MenuItem::with_id(app, id, label, enabled, None::<&str>).map_err(|e| e.to_string())
    };
    let region = item("bg-region", "Capture region", true)?;
    let window = item("bg-window", "Window", !scribblecap_capture::is_wayland())?;
    let screen = item("bg-screen", "Full screen", true)?;
    let previous = item("bg-previous", "Previous region", false)?;
    let t3 = item("bg-timer-3", "Timer: 3 s", true)?;
    let t5 = item("bg-timer-5", "Timer: 5 s", true)?;
    let t10 = item("bg-timer-10", "Timer: 10 s", true)?;
    let show = item("bg-show", "Show editor", true)?;
    let settings = item("bg-settings", "Settings…", true)?;
    let quit = item("bg-quit", "Quit", true)?;
    let menu = Menu::with_items(
        app,
        &[
            &region, &window, &screen, &previous, &t3, &t5, &t10, &show, &settings, &quit,
        ],
    )
    .map_err(|e| e.to_string())?;
    let dark = app
        .get_webview_window("main")
        .and_then(|w| w.theme().ok())
        .is_some_and(|t| t == tauri::Theme::Dark);
    TrayIconBuilder::with_id("scribblecap-tray")
        .icon(tray_image(dark))
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("ScribbleCap")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| {
            let action = match event.id().as_ref() {
                "bg-region" => Action::Capture {
                    mode: CaptureMode::Region,
                    delay: 0,
                },
                "bg-window" => Action::Capture {
                    mode: CaptureMode::Window,
                    delay: 0,
                },
                "bg-screen" => Action::Capture {
                    mode: CaptureMode::Screen,
                    delay: 0,
                },
                "bg-previous" => Action::Capture {
                    mode: CaptureMode::Previous,
                    delay: 0,
                },
                "bg-timer-3" => Action::Capture {
                    mode: CaptureMode::Region,
                    delay: 3,
                },
                "bg-timer-5" => Action::Capture {
                    mode: CaptureMode::Region,
                    delay: 5,
                },
                "bg-timer-10" => Action::Capture {
                    mode: CaptureMode::Region,
                    delay: 10,
                },
                "bg-settings" => Action::Settings,
                "bg-quit" => Action::Quit,
                "bg-show" => Action::Show,
                _ => return,
            };
            request(app, action);
        })
        .build(app)
        .map_err(|e| e.to_string())?;
    *app.state::<Runtime>()
        .previous
        .lock()
        .map_err(|_| "Tray menu is unavailable")? = Some(previous);
    if let Ok(mut status) = app.state::<Runtime>().status.lock() {
        status.tray_available = true;
    }
    Ok(())
}
pub fn theme_changed(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id("scribblecap-tray") {
        let dark = app
            .get_webview_window("main")
            .and_then(|w| w.theme().ok())
            .is_some_and(|t| t == tauri::Theme::Dark);
        let _ = tray.set_icon(Some(tray_image(dark)));
    }
}
pub fn dock_policy(app: &AppHandle, hide: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        if hide
            && !app
                .state::<Runtime>()
                .status
                .lock()
                .map_err(|_| "Background status is unavailable")?
                .tray_available
        {
            return Err("The Dock icon cannot be hidden without a menu-bar icon".into());
        }
        app.set_activation_policy(if hide {
            tauri::ActivationPolicy::Accessory
        } else {
            tauri::ActivationPolicy::Regular
        })
        .map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (app, hide);
    Ok(())
}
pub fn initialize(app: &AppHandle) {
    if let Ok(mut status) = app.state::<Runtime>().status.lock() {
        status.platform = std::env::consts::OS.into();
        status.hotkey_available = true;
    }
    let worker = app.clone();
    tauri::async_runtime::spawn(async move {
        #[cfg(target_os = "linux")]
        let available = crate::background_linux::tray_available().await;
        #[cfg(not(target_os = "linux"))]
        let available = true;
        let handle = worker.clone();
        let _=worker.run_on_main_thread(move||{
            if let Err(error)=install_tray(&handle,available){if let Ok(mut status)=handle.state::<Runtime>().status.lock(){status.tray_note=Some(error);}}
            else if !available && let Ok(mut status)=handle.state::<Runtime>().status.lock(){status.tray_note=Some("This desktop has no StatusNotifier/AppIndicator tray. Closing the editor will quit; capture remains available in the editor.".into());}
            let hide=handle.state::<crate::settings::SettingsState>().0.lock().map(|s|s.hide_dock_icon).unwrap_or(false);let _=dock_policy(&handle,hide);
        });
        let hotkey = worker
            .state::<crate::settings::SettingsState>()
            .0
            .lock()
            .map(|s| s.capture_hotkey.clone())
            .unwrap_or_default();
        if let Err(error) = register_hotkey(&worker, &hotkey).await {
            let available = worker
                .state::<Runtime>()
                .status
                .lock()
                .map(|s| s.hotkey_available)
                .unwrap_or(false);
            set_hotkey_error(&worker, Some(error), available);
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cli_modes_delays_and_invalid_arguments() {
        let parse = |args: &[&str]| parse_args(args.iter().map(|s| (*s).to_owned()));
        assert_eq!(parse(&[]), Ok(None));
        assert_eq!(
            parse(&["capture"]),
            Ok(Some(Action::Capture {
                mode: CaptureMode::Region,
                delay: 0
            }))
        );
        for (name, mode) in [
            ("region", CaptureMode::Region),
            ("window", CaptureMode::Window),
            ("screen", CaptureMode::Screen),
            ("previous", CaptureMode::Previous),
        ] {
            assert_eq!(
                parse(&["capture", name, "--delay", "7"]),
                Ok(Some(Action::Capture { mode, delay: 7 }))
            );
        }
        for args in [
            &["capture", "bad"][..],
            &["capture", "--delay"][..],
            &["capture", "--delay", "61"][..],
            &["capture", "--delay", "-1"][..],
            &["capture", "region", "extra"][..],
        ] {
            assert!(parse(args).is_err());
        }
    }
    #[test]
    fn close_never_hides_without_an_available_tray() {
        assert!(close_to_tray(true, true));
        assert!(!close_to_tray(false, true));
        assert!(!close_to_tray(true, false));
        assert!(!close_to_tray(false, false));
    }
    #[test]
    fn icon_is_monochrome_and_has_transparency() {
        let image = tray_image(false);
        assert_eq!(image.width(), 32);
        assert!(image.rgba().as_chunks::<4>().0.iter().any(|p| p[3] > 0));
        assert!(image.rgba().as_chunks::<4>().0.iter().any(|p| p[3] == 0));
        assert!(
            image
                .rgba()
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| p[..3] == [0, 0, 0])
        );
    }
}
