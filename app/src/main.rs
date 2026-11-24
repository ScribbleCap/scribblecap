//! ScribbleCap desktop app.

// Hide the extra console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod output;
#[cfg(target_os = "linux")]
mod output_linux;
mod shell;

use tauri::{Emitter, Manager};

fn main() {
    tauri::Builder::default()
        .manage(shell::DroppedImage::default())
        .manage(shell::ImageSession::default())
        .manage(output::ShareState::default())
        .on_window_event(|window, event| {
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
            output::shell_save_image,
            output::shell_drag_image,
            output::shell_cancel_drag
        ])
        .run(tauri::generate_context!())
        .expect("error while running ScribbleCap");
}
