/// Every command registered in `main.rs`. Declaring them here makes Tauri generate
/// `allow-<name>` permissions, so each window gets only what its capability grants.
const COMMANDS: &[&str] = &[
    "shell_background_ready",
    "shell_background_status",
    "shell_show_editor",
    "shell_finish_quit",
    "shell_background_capture",
    "shell_set_background_settings",
    "shell_open_image",
    "shell_paste_image",
    "shell_drop_image",
    "shell_copy_image",
    "shell_transform_image",
    "shell_privacy_image",
    "shell_restore_privacy",
    "shell_save_image",
    "shell_drag_image",
    "shell_begin_drag",
    "shell_cancel_drag",
    "shell_get_settings",
    "shell_set_auto_save",
    "shell_choose_capture_folder",
    "shell_capture_state",
    "shell_capture",
    "shell_capture_timer",
    "shell_overlay_info",
    "shell_overlay_frame",
    "shell_overlay_select",
    "shell_capture_result",
    "shell_capture_cancel",
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("failed to run tauri-build");
}
