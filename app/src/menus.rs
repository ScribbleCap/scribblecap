//! Native menus route editor actions to the active webview.
use tauri::{
    App, Emitter,
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
};

pub fn install(app: &App) -> tauri::Result<()> {
    let item = |id: &str, label: &str, shortcut: Option<&str>| {
        MenuItem::with_id(app, id, label, true, shortcut)
    };
    let open = item("open", "Open image…", Some("CmdOrCtrl+O"))?;
    let save = item("save", "Save image", Some("CmdOrCtrl+S"))?;
    let save_as = item("save-as", "Save image as…", Some("CmdOrCtrl+Shift+S"))?;
    let export = item("export", "Export flat…", None)?;
    let file = Submenu::with_items(app, "File", true, &[&open, &save, &save_as, &export])?;
    let undo = item("undo", "Undo", Some("CmdOrCtrl+Z"))?;
    let redo = item("redo", "Redo", Some("CmdOrCtrl+Shift+Z"))?;
    let cut = PredefinedMenuItem::cut(app, None)?;
    let copy = item("copy", "Copy", Some("CmdOrCtrl+C"))?;
    let paste = item("paste", "Paste", Some("CmdOrCtrl+V"))?;
    let select_all = PredefinedMenuItem::select_all(app, None)?;
    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[&undo, &redo, &cut, &copy, &paste, &select_all],
    )?;
    let fit = item("fit", "Fit image", Some("CmdOrCtrl+0"))?;
    let actual = item("actual", "Actual size", Some("CmdOrCtrl+1"))?;
    let zoom_in = item("zoom-in", "Zoom in", Some("CmdOrCtrl+="))?;
    let zoom_out = item("zoom-out", "Zoom out", Some("CmdOrCtrl+-"))?;
    let view = Submenu::with_items(app, "View", true, &[&fit, &actual, &zoom_in, &zoom_out])?;
    #[cfg(target_os = "macos")]
    let application = Submenu::with_items(
        app,
        "ScribbleCap",
        true,
        &[
            &PredefinedMenuItem::about(app, None, None)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )?;
    let window = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;
    let menu = Menu::with_items(
        app,
        &[
            #[cfg(target_os = "macos")]
            &application,
            &file,
            &edit,
            &view,
            &window,
        ],
    )?;
    app.set_menu(menu)?;
    app.on_menu_event(|app, event| {
        let _ = app.emit("shell-menu", event.id().as_ref());
    });
    Ok(())
}
