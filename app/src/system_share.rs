//! Flat PNGs for native sharing. No editable document or undo pixels leave here.
#[cfg(any(target_os = "macos", target_os = "windows", test))]
use std::{io::Write, path::PathBuf};

#[cfg(target_os = "macos")]
#[path = "system_share_macos.rs"]
mod platform;
#[cfg(target_os = "windows")]
#[path = "system_share_windows.rs"]
mod platform;

#[cfg(any(target_os = "macos", target_os = "windows", test))]
pub(super) struct ShareFile {
    _directory: tempfile::TempDir,
    pub(super) path: PathBuf,
}
#[cfg(any(target_os = "macos", target_os = "windows", test))]
impl ShareFile {
    fn write(bytes: &[u8]) -> Result<Self, String> {
        let directory = tempfile::Builder::new()
            .prefix("scribblecap-system-share-")
            .tempdir()
            .map_err(|e| e.to_string())?;
        let path = directory.path().join("ScribbleCap.png");
        let mut file = std::fs::File::create_new(&path).map_err(|e| e.to_string())?;
        file.write_all(bytes).map_err(|e| e.to_string())?;
        Ok(Self {
            _directory: directory,
            path,
        })
    }
}

#[derive(Clone, Copy, serde::Deserialize)]
pub(super) struct Anchor {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl Anchor {
    fn validate(self) -> Result<Self, String> {
        if [self.x, self.y, self.width, self.height]
            .iter()
            .all(|v| v.is_finite() && *v >= 0.0 && *v <= 100_000.0)
            && self.width > 0.0
            && self.height > 0.0
        {
            Ok(self)
        } else {
            Err("The Share button position is invalid.".into())
        }
    }
}
#[derive(serde::Deserialize)]
struct Metadata {
    document: String,
    anchor: Anchor,
}

#[tauri::command]
pub async fn shell_share_image(
    app: tauri::AppHandle,
    request: tauri::ipc::Request<'_>,
) -> Result<(), String> {
    let (source, Metadata { document, anchor }) = crate::pixel_request::decode(&request)?;
    let anchor = anchor.validate()?;
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let file = tauri::async_runtime::spawn_blocking(move || render_file(source, &document))
            .await
            .map_err(|e| e.to_string())??;
        let (sender, receiver) = std::sync::mpsc::channel();
        let handle = app.clone();
        app.run_on_main_thread(move || platform::show(&handle, file, anchor, sender))
            .map_err(|e| e.to_string())?;
        tauri::async_runtime::spawn_blocking(move || {
            receiver
                .recv()
                .map_err(|_| "The share panel closed unexpectedly.".to_string())?
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (app, source, document, anchor);
        Err("System sharing is not supported on this platform. Use Copy, Save or drag out.".into())
    }
}

#[cfg(any(target_os = "macos", target_os = "windows", test))]
fn render_file(
    source: scribblecap_formats::RasterImage,
    document: &str,
) -> Result<ShareFile, String> {
    let prepared = crate::output::prepare(source, document, None, false)?;
    let bytes =
        scribblecap_formats::encode(&prepared.preview, scribblecap_formats::RasterFormat::Png)
            .map_err(|e| e.to_string())?;
    ShareFile::write(&bytes)
}

pub fn cleanup() {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    platform::cleanup();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_png_has_fixed_name_unique_directory_and_cleans_up() {
        let image = scribblecap_formats::RasterImage::new(1, 1, vec![2, 3, 4, 255]).unwrap();
        let bytes =
            scribblecap_formats::encode(&image, scribblecap_formats::RasterFormat::Png).unwrap();
        let first = ShareFile::write(&bytes).unwrap();
        let second = ShareFile::write(&bytes).unwrap();
        let path = first.path.clone();
        assert_eq!(path.file_name().unwrap(), "ScribbleCap.png");
        assert_ne!(path.parent(), second.path.parent());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        drop(first);
        assert!(!path.exists());
        assert!(!path.parent().unwrap().exists());
        assert!(second.path.exists());
    }
    #[test]
    fn shares_only_a_flat_png_and_rejects_a_stale_document() {
        let image = scribblecap_formats::RasterImage::new(1, 1, vec![0, 0, 0, 255]).unwrap();
        let document = scribblecap_core::Document::new(scribblecap_core::Size::new(1., 1.));
        let file = render_file(image.clone(), &document.to_json().unwrap()).unwrap();
        let opened = scribblecap_formats::open(&std::fs::read(file.path).unwrap()).unwrap();
        assert!(opened.editable.is_none());
        assert_eq!(opened.raster.image, image);
        let wrong = scribblecap_core::Document::new(scribblecap_core::Size::new(2., 1.));
        assert!(render_file(image, &wrong.to_json().unwrap()).is_err());
    }
    #[test]
    fn rejects_invalid_anchor() {
        for value in [f64::NAN, f64::INFINITY, -1.0, 100_001.0] {
            assert!(
                Anchor {
                    x: value,
                    y: 0.,
                    width: 1.,
                    height: 1.
                }
                .validate()
                .is_err()
            );
        }
    }
}
