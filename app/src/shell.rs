//! Native image input. No arbitrary filesystem commands are exposed to the UI.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use scribblecap_formats::{EditableDocument, RasterImage};
use tauri::{AppHandle, Manager, ipc::Response};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;

const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_IMAGE_BYTES: usize = 256 * 1024 * 1024;

fn read_limited(reader: impl Read, limit: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("The image file could not be read: {e}"))?;
    if bytes.len() as u64 > limit {
        return Err("This image file is too large to open (maximum 512 MiB).".into());
    }
    Ok(bytes)
}

fn packet(name: &str, image: RasterImage) -> Result<Vec<u8>, String> {
    if image.pixels().len() > MAX_IMAGE_BYTES {
        return Err("This image is too large to display (about 67 million pixels maximum).".into());
    }
    if name.len() > 65536 {
        return Err("The image filename is too long.".into());
    }
    let mut out = Vec::new();
    out.try_reserve_exact(20 + name.len() + image.pixels().len())
        .map_err(|_| "There is not enough memory to open this image.".to_string())?;
    out.extend_from_slice(b"SCIM");
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&image.width().to_le_bytes());
    out.extend_from_slice(&image.height().to_le_bytes());
    out.extend_from_slice(&(name.len() as u32).to_le_bytes());
    out.extend_from_slice(name.as_bytes());
    out.extend_from_slice(image.pixels());
    Ok(out)
}

fn loaded_packet(name: &str, image: RasterImage, editable: bool) -> Result<Vec<u8>, String> {
    let mut bytes = packet(name, image)?;
    if editable {
        bytes[4..8].copy_from_slice(&2u32.to_le_bytes());
        bytes.splice(20..20, 1u32.to_le_bytes());
    }
    Ok(bytes)
}

#[derive(Default)]
pub struct Session {
    pub editable: Option<EditableDocument>,
    pub preview: Option<RasterImage>,
    pub target: Option<PathBuf>,
}
#[derive(Default)]
pub struct ImageSession(pub std::sync::Mutex<Session>);

fn remember(
    app: &AppHandle,
    editable: Option<EditableDocument>,
    preview: RasterImage,
    target: Option<PathBuf>,
) -> Result<(), String> {
    *app.state::<ImageSession>()
        .0
        .lock()
        .map_err(|_| "The image session is unavailable.")? = Session {
        editable,
        preview: Some(preview),
        target,
    };
    Ok(())
}

type OpenedInput = (String, Vec<u8>, Option<EditableDocument>, RasterImage);
fn read_image(path: &Path) -> Result<OpenedInput, String> {
    let file = std::fs::File::open(path)
        .map_err(|e| format!("The image file could not be opened: {e}"))?;
    let bytes = read_limited(file, MAX_FILE_BYTES)?;
    // Damaged editable metadata is an error, never silently flattened on save.
    let opened = scribblecap_formats::open(&bytes)
        .map_err(|e| format!("This image could not be opened. {e}"))?;
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let preview = opened.raster.image;
    let bytes = loaded_packet(&name, preview.clone(), opened.editable.is_some())?;
    Ok((name, bytes, opened.editable, preview))
}

fn set_title(app: &AppHandle, name: &str) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_title(&format!("{name} — ScribbleCap"));
    }
}

/// Only paths delivered by the native OS drop event are accepted.
#[derive(Default)]
pub struct DroppedImage(pub Mutex<Vec<PathBuf>>);

/// `None` when there is nothing left to open; an error for a multi-file drop.
fn single_drop(mut paths: Vec<PathBuf>) -> Result<Option<PathBuf>, String> {
    match paths.len() {
        0 => Ok(None),
        1 => Ok(paths.pop()),
        _ => Err("Drop one image at a time.".into()),
    }
}

#[tauri::command]
pub async fn shell_drop_image(app: AppHandle) -> Result<Response, String> {
    let paths = std::mem::take(
        &mut *app
            .state::<DroppedImage>()
            .0
            .lock()
            .map_err(|_| "The file drop is unavailable.")?,
    );
    let Some(path) = single_drop(paths)? else {
        // An earlier request already consumed this drop (two drops arrived before
        // the UI asked). An empty packet means "nothing to open".
        return Ok(Response::new(Vec::new()));
    };
    let target = path.clone();
    let (name, bytes, editable, preview) =
        tauri::async_runtime::spawn_blocking(move || read_image(&path))
            .await
            .map_err(|e| format!("Image opening stopped: {e}"))??;
    remember(&app, editable, preview, Some(target))?;
    set_title(&app, &name);
    Ok(Response::new(bytes))
}

#[tauri::command]
pub async fn shell_open_image(app: AppHandle) -> Result<Response, String> {
    let worker = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let selected = worker
            .dialog()
            .file()
            .set_title("Open an image")
            .add_filter(
                "Images",
                &["png", "jpg", "jpeg", "webp", "tif", "tiff", "bmp", "gif"],
            )
            .blocking_pick_file();
        let Some(selected) = selected else {
            return Ok(None);
        };
        let path = selected
            .into_path()
            .map_err(|e| format!("The selected file is unavailable: {e}"))?;
        read_image(&path)
            .map(|(name, bytes, editable, preview)| Some((name, bytes, editable, preview, path)))
    })
    .await
    .map_err(|e| format!("Image opening stopped: {e}"))?;
    match result? {
        Some((name, bytes, editable, preview, target)) => {
            remember(&app, editable, preview, Some(target))?;
            set_title(&app, &name);
            Ok(Response::new(bytes))
        }
        None => Ok(Response::new(Vec::new())),
    }
}

#[tauri::command]
pub async fn shell_paste_image(app: AppHandle) -> Result<Response, String> {
    let worker = app.clone();
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        let image = worker.clipboard().read_image().map_err(|_| {
            "No image could be read from the clipboard. Copy an image and try again.".to_string()
        })?;
        let image = RasterImage::new(image.width(), image.height(), image.rgba().to_vec())
            .map_err(|e| e.to_string())?;
        let bytes = packet("Clipboard image", image.clone())?;
        remember(&worker, None, image, None)?;
        Ok::<_, String>(bytes)
    })
    .await
    .map_err(|e| format!("Image paste stopped: {e}"))??;
    set_title(&app, "Clipboard image");
    Ok(Response::new(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use scribblecap_formats::{RasterFormat, encode};

    #[test]
    fn binary_packet_has_exact_unicode_name_dimensions_and_straight_pixels() {
        let bytes = packet(
            "résumé.png",
            RasterImage::new(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 128]).unwrap(),
        )
        .unwrap();
        assert_eq!(&bytes[..8], b"SCIM\x01\0\0\0");
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 2);
        assert_eq!(u32::from_le_bytes(bytes[12..16].try_into().unwrap()), 1);
        let n = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;
        assert_eq!(
            std::str::from_utf8(&bytes[20..20 + n]).unwrap(),
            "résumé.png"
        );
        assert_eq!(&bytes[20 + n..], &[255, 0, 0, 255, 0, 0, 255, 128]);
    }
    #[test]
    fn bounded_reads_reject_excess_data() {
        assert_eq!(read_limited(&b"1234"[..], 4).unwrap(), b"1234");
        assert!(read_limited(&b"12345"[..], 4).is_err());
    }
    #[test]
    fn native_file_pipeline_uses_formats_codecs_and_rejects_invalid_inputs() {
        let path =
            std::env::temp_dir().join(format!("scribblecap-shell-test-{}.png", std::process::id()));
        let image = RasterImage::new(1, 1, vec![255, 0, 0, 255]).unwrap();
        std::fs::write(&path, encode(&image, RasterFormat::Png).unwrap()).unwrap();
        let result = read_image(&path);
        std::fs::remove_file(&path).unwrap();
        assert_eq!(result.unwrap().1.last(), Some(&255));
        assert!(read_image(&path).is_err());
    }

    #[test]
    fn a_consumed_drop_opens_nothing_and_multiple_files_are_rejected() {
        assert_eq!(single_drop(vec![]), Ok(None));
        let one = PathBuf::from("one.png");
        assert_eq!(single_drop(vec![one.clone()]), Ok(Some(one)));
        assert!(single_drop(vec!["a.png".into(), "b.png".into()]).is_err());
    }
}
