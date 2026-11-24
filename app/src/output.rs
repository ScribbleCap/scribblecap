//! Native output paths are chosen by the user; frontend input is bounded RGBA only.
use scribblecap_formats::{EditableDocument, RasterFormat, RasterImage};
use std::{
    io::Write,
    path::Path,
    sync::{
        Mutex,
        atomic::{AtomicU32, Ordering},
    },
};

use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

#[derive(Default)]
pub struct ShareState {
    cancelled: AtomicU32,
    files: Mutex<Vec<tempfile::TempDir>>,
}
#[tauri::command]
pub fn shell_cancel_drag(app: AppHandle, gesture: u32) {
    app.state::<ShareState>()
        .cancelled
        .fetch_max(gesture, Ordering::SeqCst);
}

fn image(width: u32, height: u32, pixels: Vec<u8>) -> Result<RasterImage, String> {
    if pixels.len() > 256 * 1024 * 1024 {
        return Err("This image is too large to save.".into());
    }
    RasterImage::new(width, height, pixels).map_err(|e| e.to_string())
}
struct Prepared {
    preview: RasterImage,
    editable: Option<EditableDocument>,
}

/// Render the current WASM document with the same core painter used by CLI export.
/// Imported annotations are already in the displayed preview; append only this
/// session's new elements to their retained original editable document.
fn prepare(
    source: RasterImage,
    json: &str,
    original: Option<&EditableDocument>,
    build_editable: bool,
) -> Result<Prepared, String> {
    use scribblecap_core::render::{BackgroundImage, render};
    use scribblecap_core::{Background, Document, ElementId, Point, Resource, ResourceId, Shape};
    if json.len() > 16 * 1024 * 1024 {
        return Err("The annotation document is too large to save.".into());
    }
    let mut overlay = Document::from_json(json).map_err(|e| e.to_string())?;
    if overlay.canvas.width != f64::from(source.width())
        || overlay.canvas.height != f64::from(source.height())
        || overlay.background.is_some()
        || !overlay.resources.is_empty()
    {
        return Err("The annotation canvas does not match the loaded image.".into());
    }
    let added = overlay.elements.clone();
    let id = ResourceId("scribblecap-output-background".into());
    overlay.background = Some(Background {
        resource: id.clone(),
        origin: Point::new(0.0, 0.0),
    });
    overlay.resources.push(Resource {
        id: id.clone(),
        media_type: "image/png".into(),
    });
    let background = BackgroundImage::new(&id, source.width(), source.height(), source.pixels())
        .map_err(|e| e.to_string())?;
    let rendered = render(&overlay, Some(background)).map_err(|e| e.to_string())?;
    let preview = RasterImage::new(rendered.width(), rendered.height(), rendered.into_pixels())
        .map_err(|e| e.to_string())?;
    if !build_editable {
        return Ok(Prepared {
            preview,
            editable: None,
        });
    }
    let editable = if let Some(original) = original {
        let mut document = original.document.clone();
        if document.canvas != overlay.canvas {
            return Err(
                "The imported document canvas changed. Reopen the image before saving.".into(),
            );
        }
        let mut occupied: std::collections::HashSet<_> =
            document.elements.iter().map(|e| e.id.clone()).collect();
        let mut mapping = std::collections::HashMap::new();
        for (index, element) in added.iter().enumerate() {
            let mut number = index;
            let target = loop {
                let target = ElementId(format!("scribblecap-addition-{number}"));
                if occupied.insert(target.clone()) {
                    break target;
                }
                number += 1;
            };
            mapping.insert(element.id.clone(), target);
        }
        for mut element in added {
            element.id = mapping[&element.id].clone();
            if let Shape::Text(text) = &mut element.shape
                && let Some(target) = &mut text.attached_to
            {
                *target = mapping
                    .get(target)
                    .ok_or("An added text label has an invalid attachment.")?
                    .clone();
            }
            document.elements.push(element);
        }
        Some(
            EditableDocument::new(document, original.resources.clone())
                .map_err(|e| e.to_string())?,
        )
    } else if added.is_empty() {
        None
    } else {
        let bytes =
            scribblecap_formats::encode(&source, RasterFormat::Png).map_err(|e| e.to_string())?;
        Some(
            EditableDocument::new(
                overlay,
                vec![scribblecap_formats::ResourceData { id, bytes }],
            )
            .map_err(|e| e.to_string())?,
        )
    };
    Ok(Prepared { preview, editable })
}

fn format(path: &Path) -> Result<RasterFormat, String> {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => Ok(RasterFormat::Png),
        "jpg" | "jpeg" => Ok(RasterFormat::Jpeg),
        "webp" => Ok(RasterFormat::WebP),
        "tif" | "tiff" => Ok(RasterFormat::Tiff),
        "bmp" => Ok(RasterFormat::Bmp),
        "gif" => Ok(RasterFormat::Gif),
        _ => Err("Choose a PNG, JPEG, WebP, TIFF, BMP or GIF filename extension.".into()),
    }
}
fn encode(
    image: &RasterImage,
    format: RasterFormat,
    editable: Option<&EditableDocument>,
) -> Result<Vec<u8>, String> {
    if let Some(editable) = editable {
        scribblecap_formats::encode_editable(image, format, editable).map_err(|e| format!("This format cannot preserve the editable image. Choose PNG, JPEG or WebP, or use Export flat. {e}"))
    } else {
        scribblecap_formats::encode(image, format).map_err(|e| e.to_string())
    }
}
fn atomic_save(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("The save folder is unavailable.")?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|e| format!("The save folder could not be written: {e}"))?;
    temporary
        .write_all(bytes)
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|e| format!("The image could not be saved: {e}"))?;
    temporary
        .persist(path)
        .map_err(|e| format!("The image could not replace the destination: {e}"))?;
    Ok(())
}

#[tauri::command]
pub async fn shell_copy_image(
    app: AppHandle,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    document: String,
) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    let source = image(width, height, pixels)?;
    tauri::async_runtime::spawn_blocking(move || {
        let prepared = prepare(source, &document, None, false)?;
        app.clipboard()
            .write_image(&tauri::image::Image::new_owned(
                prepared.preview.into_pixels(),
                width,
                height,
            ))
            .map_err(|e| format!("The image could not be copied: {e}"))
    })
    .await
    .map_err(|e| format!("Image copying stopped: {e}"))?
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn shell_save_image(
    app: AppHandle,
    name: String,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    document: String,
    save_as: bool,
    flat: bool,
) -> Result<Option<String>, String> {
    let image = image(width, height, pixels)?;
    tauri::async_runtime::spawn_blocking(move || {
        let (editable, preview, target) = {
            let session = app.state::<crate::shell::ImageSession>();
            let session = session
                .0
                .lock()
                .map_err(|_| "The image session is unavailable.")?;
            (
                session.editable.clone(),
                session.preview.clone(),
                session.target.clone(),
            )
        };
        if editable.is_some() && preview.as_ref() != Some(&image) {
            return Err("The imported preview changed. Reopen the image before saving.".into());
        }
        let prepared = prepare(image, &document, editable.as_ref(), !flat)?;
        let target = if save_as || flat || target.is_none() {
            let suggested = if Path::new(&name).extension().is_some() {
                name
            } else {
                format!("{name}.png")
            };
            let selected = app
                .dialog()
                .file()
                .set_title(if flat {
                    "Export a flat image"
                } else {
                    "Save image as"
                })
                .set_file_name(suggested)
                .add_filter(
                    "Images",
                    &["png", "jpg", "jpeg", "webp", "tif", "tiff", "bmp", "gif"],
                )
                .blocking_save_file();
            let Some(selected) = selected else {
                return Ok(None);
            };
            selected
                .into_path()
                .map_err(|e| format!("The destination is unavailable: {e}"))?
        } else {
            target.ok_or("The save destination is unavailable.")?
        };
        let bytes = encode(
            &prepared.preview,
            format(&target)?,
            prepared.editable.as_ref(),
        )?;
        // Export flat always creates a separate file for editable inputs, even when
        // the user chooses the original path in the save dialog.
        if flat && target.exists() {
            let metadata = std::fs::metadata(&target).map_err(|e| e.to_string())?;
            if metadata.len() > 512 * 1024 * 1024 {
                return Err("Choose a new filename for this export.".into());
            }
            let existing = std::fs::read(&target).map_err(|e| e.to_string())?;
            match scribblecap_formats::extract_editable(&existing) {
                Ok(None) => (),
                _ => {
                    return Err(
                        "Export to a different filename to preserve the editable destination."
                            .into(),
                    );
                }
            }
        }
        atomic_save(&target, &bytes)?;
        let name = target
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if !flat {
            app.state::<crate::shell::ImageSession>()
                .0
                .lock()
                .map_err(|_| "The image session is unavailable.")?
                .target = Some(target);
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_title(&format!("{name} — ScribbleCap"));
            }
        }
        Ok(Some(name))
    })
    .await
    .map_err(|e| format!("Image saving stopped: {e}"))?
}

#[tauri::command]
pub async fn shell_drag_image(
    app: AppHandle,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    document: String,
    gesture: u32,
) -> Result<(), String> {
    let image = image(width, height, pixels)?;
    let worker = app.clone();
    let temporary = tauri::async_runtime::spawn_blocking(move || {
        let session = worker.state::<crate::shell::ImageSession>();
        let session = session
            .0
            .lock()
            .map_err(|_| "The image session is unavailable.")?;
        if session.editable.is_some() && session.preview.as_ref() != Some(&image) {
            return Err(
                "The editable preview changed. Reopen the original before sharing.".to_string(),
            );
        }
        let prepared = prepare(image, &document, session.editable.as_ref(), true)?;
        let bytes = encode(
            &prepared.preview,
            RasterFormat::Png,
            prepared.editable.as_ref(),
        )?;
        let directory = tempfile::Builder::new()
            .prefix("scribblecap-share-")
            .tempdir()
            .map_err(|e| e.to_string())?;
        std::fs::write(directory.path().join("ScribbleCap.png"), bytes)
            .map_err(|e| e.to_string())?;
        Ok::<_, String>(directory)
    })
    .await
    .map_err(|e| format!("Image sharing stopped: {e}"))??;
    let window = app
        .get_webview_window("main")
        .ok_or("The editor window is unavailable.")?;
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let worker = app.clone();
    app.run_on_main_thread(move || {
        if worker
            .state::<ShareState>()
            .cancelled
            .load(Ordering::SeqCst)
            >= gesture
        {
            let _ = sender.send(Ok(()));
            return;
        }
        let path = temporary.path().join("ScribbleCap.png");
        // Keep real files until app shutdown: destinations can read asynchronously
        // after the OS reports a completed drop (especially GTK).
        let state = worker.state::<ShareState>();
        if let Ok(mut files) = state.files.lock() {
            files.push(temporary);
        } else {
            let _ = sender.send(Err("The share files are unavailable.".into()));
            return;
        }
        let start = || {
            #[cfg(target_os = "linux")]
            {
                crate::output_linux::start(&window.gtk_window().map_err(|e| e.to_string())?, &path)
            }
            #[cfg(not(target_os = "linux"))]
            {
                drag::start_drag(
                    &window,
                    drag::DragItem::Files(vec![path]),
                    drag::Image::Raw(include_bytes!("../icons/32x32.png").to_vec()),
                    |_, _| {},
                    drag::Options {
                        mode: drag::DragMode::Copy,
                        ..Default::default()
                    },
                )
                .map_err(|e| e.to_string())
            }
        };
        let _ = sender.send(start());
    })
    .map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || receiver.recv().map_err(|e| e.to_string())?)
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    fn annotated_document() -> scribblecap_core::Document {
        use scribblecap_core::{Color, Document, Element, ElementId, Line, Point, Shape, Size};
        let mut document = Document::new(Size::new(8.0, 8.0));
        document.elements.push(Element {
            id: ElementId("el-1".into()),
            shape: Shape::Line(Line {
                start: Point::new(1.0, 4.0),
                end: Point::new(7.0, 4.0),
                color: Color::rgb(255, 0, 0),
                width: 2.0,
            }),
        });
        document
    }
    #[test]
    fn current_annotations_are_rendered_and_saved_with_the_original_background() {
        let source = image(8, 8, vec![255; 8 * 8 * 4]).unwrap();
        let prepared = prepare(
            source.clone(),
            &annotated_document().to_json().unwrap(),
            None,
            true,
        )
        .unwrap();
        assert_ne!(prepared.preview, source);
        let editable = prepared.editable.unwrap();
        assert_eq!(editable.document.elements.len(), 1);
        assert_eq!(
            scribblecap_formats::decode(&editable.resources[0].bytes)
                .unwrap()
                .image,
            source
        );
        let saved = encode(&prepared.preview, RasterFormat::Png, Some(&editable)).unwrap();
        let opened = scribblecap_formats::open(&saved).unwrap();
        assert_eq!(opened.raster.image, prepared.preview);
        assert_eq!(opened.editable, Some(editable));
    }
    #[test]
    fn imported_annotations_and_resources_survive_added_overlays_without_id_collisions() {
        let source = image(8, 8, vec![255; 8 * 8 * 4]).unwrap();
        let original =
            prepare(source, &annotated_document().to_json().unwrap(), None, true).unwrap();
        let mut editable = original.editable.unwrap();
        editable.document.elements[0].id =
            scribblecap_core::ElementId("scribblecap-addition-0".into());
        let additions = annotated_document();
        let combined = prepare(
            original.preview,
            &additions.to_json().unwrap(),
            Some(&editable),
            true,
        )
        .unwrap();
        let combined = combined.editable.unwrap();
        assert_eq!(combined.document.elements.len(), 2);
        assert_eq!(combined.document.elements[0], editable.document.elements[0]);
        assert_ne!(
            combined.document.elements[0].id,
            combined.document.elements[1].id
        );
        assert_eq!(combined.resources, editable.resources);
        combined.document.validate().unwrap();
    }
    #[test]
    fn stale_canvas_requests_are_rejected_and_copy_does_not_build_hidden_resources() {
        let source = image(8, 8, vec![255; 8 * 8 * 4]).unwrap();
        let copied = prepare(
            source.clone(),
            &annotated_document().to_json().unwrap(),
            None,
            false,
        )
        .unwrap();
        assert_ne!(copied.preview, source);
        assert!(copied.editable.is_none());
        let wrong = scribblecap_core::Document::new(scribblecap_core::Size::new(9.0, 8.0));
        assert!(prepare(source, &wrong.to_json().unwrap(), None, true).is_err());
    }
    #[test]
    fn all_flat_formats_save_as_valid_images() {
        let directory = tempfile::tempdir().unwrap();
        let image = image(1, 1, vec![255, 0, 0, 255]).unwrap();
        for extension in ["png", "jpg", "webp", "tiff", "bmp", "gif"] {
            let path = directory.path().join(format!("test.{extension}"));
            atomic_save(
                &path,
                &encode(&image, format(&path).unwrap(), None).unwrap(),
            )
            .unwrap();
            assert_eq!(
                scribblecap_formats::open(&std::fs::read(&path).unwrap())
                    .unwrap()
                    .raster
                    .image
                    .width(),
                1
            );
            atomic_save(
                &path,
                &encode(&image, format(&path).unwrap(), None).unwrap(),
            )
            .unwrap();
        }
        assert!(format(Path::new("test.pdf")).is_err());
    }
    #[test]
    fn failed_atomic_save_preserves_an_existing_destination() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("destination");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("precious.txt"), b"keep me").unwrap();
        assert!(atomic_save(&target, b"replacement").is_err());
        assert_eq!(
            std::fs::read(target.join("precious.txt")).unwrap(),
            b"keep me"
        );
    }
    #[test]
    fn editable_payload_survives_save_and_flat_only_formats_are_rejected() {
        let image = image(1, 1, vec![255, 0, 0, 255]).unwrap();
        let editable = EditableDocument::new(
            scribblecap_core::Document::new(scribblecap_core::Size::new(1.0, 1.0)),
            vec![],
        )
        .unwrap();
        for format in [RasterFormat::Png, RasterFormat::Jpeg, RasterFormat::WebP] {
            let bytes = encode(&image, format, Some(&editable)).unwrap();
            assert_eq!(
                scribblecap_formats::open(&bytes).unwrap().editable,
                Some(editable.clone())
            );
        }
        assert!(encode(&image, RasterFormat::Tiff, Some(&editable)).is_err());
    }
}
