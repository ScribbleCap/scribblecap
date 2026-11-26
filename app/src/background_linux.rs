//! Wayland portal shortcuts and conservative StatusNotifier tray discovery.
use ashpd::desktop::{
    CreateSessionOptions, Session,
    global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut},
};
use futures_util::StreamExt;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Modifiers, Shortcut};

pub struct Binding {
    session: Session<GlobalShortcuts>,
    task: tauri::async_runtime::JoinHandle<()>,
}

pub async fn tray_available() -> bool {
    async fn check() -> Result<bool, ashpd::zbus::Error> {
        let connection = ashpd::zbus::Connection::session().await?;
        let proxy = ashpd::zbus::fdo::DBusProxy::new(&connection).await?;
        proxy
            .name_has_owner("org.kde.StatusNotifierWatcher".try_into()?)
            .await
            .map_err(Into::into)
    }
    check().await.unwrap_or(false)
}
fn trigger(shortcut: &Shortcut) -> String {
    let mut result = String::new();
    for (flag, name) in [
        (Modifiers::CONTROL, "CTRL"),
        (Modifiers::SHIFT, "SHIFT"),
        (Modifiers::ALT, "ALT"),
        (Modifiers::SUPER, "LOGO"),
    ] {
        if shortcut.mods.contains(flag) {
            result.push_str(&format!("<{name}>"));
        }
    }
    let key = shortcut.key.to_string();
    result.push_str(
        key.strip_prefix("Digit")
            .or_else(|| key.strip_prefix("Key"))
            .unwrap_or(&key),
    );
    result
}
pub async fn bind(app: &AppHandle, value: &str) -> Result<(), String> {
    let shortcut = value
        .parse::<Shortcut>()
        .map_err(|e| format!("Invalid shortcut: {e}"))?;
    let proxy = match GlobalShortcuts::new().await {
        Ok(proxy) => proxy,
        Err(error) => {
            crate::background::set_hotkey_error(
                app,
                Some(format!(
                    "Global shortcuts are unavailable: this Wayland desktop does not provide the GlobalShortcuts portal ({error})."
                )),
                false,
            );
            return Err(
                "This Wayland desktop does not provide a usable GlobalShortcuts portal".into(),
            );
        }
    };
    crate::background::set_hotkey_error(app, None, true);
    let session = proxy
        .create_session(CreateSessionOptions::default())
        .await
        .map_err(|e| format!("The shortcut portal did not create a session: {e}"))?;
    let mut stream = proxy.receive_activated().await.map_err(|e| e.to_string())?;
    let request = proxy
        .bind_shortcuts(
            &session,
            &[
                NewShortcut::new("capture-region", "Capture a region in ScribbleCap")
                    .preferred_trigger(Some(trigger(&shortcut).as_str())),
            ],
            None,
            BindShortcutsOptions::default(),
        )
        .await
        .map_err(|e| format!("The shortcut portal refused registration: {e}"))?;
    let response = request
        .response()
        .map_err(|e| format!("Shortcut registration was cancelled or refused: {e}"))?;
    if !response
        .shortcuts()
        .iter()
        .any(|s| s.id() == "capture-region")
    {
        let _ = session.close().await;
        return Err("The portal did not register the capture shortcut".into());
    }
    if let Some(shortcut) = response
        .shortcuts()
        .iter()
        .find(|s| s.id() == "capture-region")
        && let Ok(mut status) = app.state::<crate::background::Runtime>().status.lock()
    {
        status.hotkey_description = Some(shortcut.trigger_description().to_owned());
    }
    let session_path = serde_json::to_value(&session)
        .map_err(|e| e.to_string())?
        .as_str()
        .ok_or("Invalid portal session identity")?
        .to_owned();
    let handle = app.clone();
    let task = tauri::async_runtime::spawn(async move {
        while let Some(event) = stream.next().await {
            if event.shortcut_id() == "capture-region"
                && event.session_handle().as_str() == session_path
            {
                crate::background::request(
                    &handle,
                    crate::background::Action::Capture {
                        mode: crate::capture::session::CaptureMode::Region,
                        delay: 0,
                    },
                );
            }
        }
    });
    let old = app
        .state::<crate::background::Runtime>()
        .portal_task
        .lock()
        .map_err(|_| "Shortcut state is unavailable")?
        .replace(Binding { session, task });
    if let Some(old) = old {
        let _ = old.session.close().await;
        old.task.abort();
    }
    crate::background::set_hotkey_error(app, None, true);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uses_xdg_trigger_not_plugin_spelling() {
        assert_eq!(
            trigger(&"Ctrl+Shift+Digit2".parse().unwrap()),
            "<CTRL><SHIFT>2"
        );
    }
}
