//! ScribbleCap desktop app.

// Hide the extra console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod canvas;
mod capture;
mod menus;
mod output;
#[cfg(target_os = "linux")]
mod output_linux;
mod pixel_request;
mod privacy;
mod settings;
mod shell;

use tauri::{Emitter, Manager};

fn main() {
    tauri::Builder::default()
        .manage(shell::DroppedImage::default())
        .manage(shell::ImageSession::default())
        .manage(privacy::PrivacyHistory::default())
        .manage(output::ShareState::default())
        .manage(capture::session::CaptureState::default())
        .setup(|app| {
            app.manage(settings::initial(app.handle()));
            menus::install(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                capture::window_destroyed(window);
            }
            if let tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event
                && let Ok(mut pending) = window.state::<shell::DroppedImage>().0.lock()
            {
                *pending = paths.clone();
                let _ = window.emit("shell-file-drop", ());
            }
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            shell::shell_open_image,
            shell::shell_paste_image,
            shell::shell_drop_image,
            output::shell_copy_image,
            canvas::shell_transform_image,
            privacy::shell_privacy_image,
            privacy::shell_restore_privacy,
            output::shell_save_image,
            output::shell_drag_image,
            output::shell_begin_drag,
            output::shell_cancel_drag,
            settings::shell_get_settings,
            settings::shell_set_auto_save,
            settings::shell_choose_capture_folder,
            capture::shell_capture_state,
            capture::shell_capture,
            capture::shell_capture_timer,
            capture::shell_overlay_info,
            capture::shell_overlay_frame,
            capture::shell_overlay_select,
            capture::shell_capture_result,
            capture::shell_capture_cancel
        ])
        .build(tauri::generate_context!())
        .expect("error while building ScribbleCap")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                output::cleanup_share_files(app);
            }
        });
}
