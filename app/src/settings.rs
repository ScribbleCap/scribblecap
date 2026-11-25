//! Persistent app settings, stored as JSON in the app config directory.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

#[derive(Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub capture_folder: Option<PathBuf>,
    pub auto_save: bool,
}

pub struct SettingsState(pub Mutex<Settings>);

/// Reads settings; a missing or corrupt file yields the defaults, never an error.
pub fn load(path: &Path) -> Settings {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn store(path: &Path, settings: &Settings) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("The settings folder is unavailable: {e}"))?;
    }
    crate::output::atomic_save(path, &bytes)
}

/// A folder is usable when it exists, is a directory and accepts a new file.
pub fn usable_folder(folder: &Path) -> bool {
    folder.is_dir() && tempfile::NamedTempFile::new_in(folder).is_ok()
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|dir| dir.join("settings.json"))
        .map_err(|e| format!("The settings location is unavailable: {e}"))
}

/// Loads settings at startup (defaults when the location or file is unavailable).
pub fn initial(app: &AppHandle) -> SettingsState {
    SettingsState(Mutex::new(
        settings_path(app).map(|p| load(&p)).unwrap_or_default(),
    ))
}

fn update(
    app: &AppHandle,
    state: &SettingsState,
    change: impl FnOnce(&mut Settings),
) -> Result<Settings, String> {
    let mut settings = state
        .0
        .lock()
        .map_err(|_| "The settings are unavailable.".to_string())?;
    let mut next = settings.clone();
    change(&mut next);
    store(&settings_path(app)?, &next)?;
    *settings = next.clone();
    Ok(next)
}

fn pick_folder(app: &AppHandle) -> Result<Option<PathBuf>, String> {
    let Some(selected) = app
        .dialog()
        .file()
        .set_title("Choose where captures are saved")
        .blocking_pick_folder()
    else {
        return Ok(None);
    };
    selected
        .into_path()
        .map(Some)
        .map_err(|e| format!("The selected folder is unavailable: {e}"))
}

/// Keeps the saved folder if usable; otherwise asks `pick` until a usable folder
/// is chosen. Returns the folder and whether it is new, or `None` if cancelled.
fn resolve_folder(
    saved: Option<PathBuf>,
    usable: impl Fn(&Path) -> bool,
    mut pick: impl FnMut() -> Result<Option<PathBuf>, String>,
) -> Result<Option<(PathBuf, bool)>, String> {
    if let Some(folder) = saved.filter(|f| usable(f)) {
        return Ok(Some((folder, false)));
    }
    loop {
        match pick()? {
            None => return Ok(None),
            Some(folder) if usable(&folder) => return Ok(Some((folder, true))),
            Some(_) => {}
        }
    }
}

/// The saved capture folder if usable, otherwise a folder picker (asked again while
/// the choice is unusable) whose choice is stored. `Ok(None)` when the picker is
/// cancelled. Blocks on the dialog: call off the main thread.
pub fn ensure_capture_folder(app: &AppHandle) -> Result<Option<PathBuf>, String> {
    let state = app.state::<SettingsState>();
    let saved = state
        .0
        .lock()
        .map_err(|_| "The settings are unavailable.".to_string())?
        .capture_folder
        .clone();
    let Some((folder, changed)) = resolve_folder(saved, usable_folder, || pick_folder(app))? else {
        return Ok(None);
    };
    if changed {
        update(app, &state, |s| s.capture_folder = Some(folder.clone()))?;
    }
    Ok(Some(folder))
}

#[tauri::command]
pub fn shell_get_settings(state: State<'_, SettingsState>) -> Result<Settings, String> {
    state
        .0
        .lock()
        .map(|s| s.clone())
        .map_err(|_| "The settings are unavailable.".to_string())
}

#[tauri::command]
pub fn shell_set_auto_save(
    app: AppHandle,
    state: State<'_, SettingsState>,
    enabled: bool,
) -> Result<Settings, String> {
    update(&app, &state, |s| s.auto_save = enabled)
}

/// Always shows the folder picker; a cancelled picker leaves the settings unchanged.
#[tauri::command]
pub async fn shell_choose_capture_folder(app: AppHandle) -> Result<Settings, String> {
    let worker = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = worker.state::<SettingsState>();
        match pick_folder(&worker)? {
            Some(folder) if !usable_folder(&folder) => {
                Err("That folder cannot be written to. Choose another.".to_string())
            }
            Some(folder) => update(&worker, &state, |s| s.capture_folder = Some(folder)),
            None => state
                .0
                .lock()
                .map(|s| s.clone())
                .map_err(|_| "The settings are unavailable.".to_string()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unusable_picks_ask_again_until_usable_or_cancelled() {
        let usable = |p: &Path| p.ends_with("good");
        let mut picks = vec![
            Some("bad".into()),
            Some("worse".into()),
            Some("good".into()),
        ]
        .into_iter();
        let mut asked = 0;
        let got = resolve_folder(Some("stale".into()), usable, || {
            asked += 1;
            Ok(picks.next().unwrap())
        });
        assert_eq!(got, Ok(Some((PathBuf::from("good"), true))));
        assert_eq!(asked, 3);
        assert_eq!(
            resolve_folder(Some("good".into()), usable, || panic!("no picker")),
            Ok(Some((PathBuf::from("good"), false)))
        );
        let mut once = vec![Some("bad".into()), None].into_iter();
        assert_eq!(
            resolve_folder(None, usable, || Ok(once.next().unwrap())),
            Ok(None)
        );
    }

    #[test]
    fn settings_default_off_and_survive_a_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        assert_eq!(
            load(&path),
            Settings {
                capture_folder: None,
                auto_save: false
            }
        );
        let s = Settings {
            capture_folder: Some(dir.path().into()),
            auto_save: true,
        };
        store(&path, &s).unwrap();
        assert_eq!(load(&path), s);
    }

    #[test]
    fn corrupt_or_unknown_settings_fall_back_safely() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, b"{not json").unwrap();
        assert_eq!(load(&path), Settings::default());
        std::fs::write(&path, br#"{"autoSave":true,"theme":"dark"}"#).unwrap();
        assert!(load(&path).auto_save);
    }

    #[test]
    fn missing_or_read_only_folders_are_not_usable() {
        let dir = tempfile::tempdir().unwrap();
        assert!(usable_folder(dir.path()));
        assert!(!usable_folder(&dir.path().join("gone")));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o555)).unwrap();
            assert!(!usable_folder(dir.path()));
            std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
}
