//! ScribbleCap desktop app.

// Hide the extra console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod shell;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            shell::shell_open_image,
            shell::shell_paste_image
        ])
        .run(tauri::generate_context!())
        .expect("error while running ScribbleCap");
}
