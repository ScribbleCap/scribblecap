//! Background-only privacy edits and session-only undo resources. Output commands
//! see only the current Session; historical resources never enter saved files.
use scribblecap_core::{
    Background, Point, Resource, ResourceId,
    render::{BackgroundImage, render},
};
use scribblecap_formats::{EditableDocument, RasterFormat, RasterImage, ResourceData};
use std::{collections::BTreeMap, sync::Mutex};
use tauri::{
    AppHandle, Manager,
    ipc::{Request, Response},
};

const MAX_HISTORY_BYTES: usize = 512 * 1024 * 1024;
#[derive(Default)]
pub struct PrivacyHistory(Mutex<History>);
#[derive(Default)]
struct History {
    snapshots: BTreeMap<u32, Snapshot>,
    current: u32,
    next: u32,
}
struct Snapshot {
    name: String,
    source: RasterImage,
    editable: Option<EditableDocument>,
}
impl Snapshot {
    fn weight(&self) -> usize {
        self.source.pixels().len()
            + self.editable.as_ref().map_or(0, |e| {
                e.resources.iter().map(|r| r.bytes.len()).sum::<usize>()
            })
    }
    fn session(&self) -> crate::shell::Session {
        crate::shell::Session {
            editable: self.editable.clone(),
            preview: self.editable.as_ref().map(|_| self.source.clone()),
            target: None,
            capture_name: None,
        }
    }
    fn packet(&self, revision: u32) -> Result<Vec<u8>, String> {
        let mut out = revision.to_le_bytes().to_vec();
        out.extend(crate::shell::loaded_packet(
            &self.name,
            self.source.clone(),
            self.editable.is_some(),
        )?);
        Ok(out)
    }
}
pub fn reset(app: &AppHandle) -> Result<(), String> {
    *app.state::<PrivacyHistory>()
        .0
        .lock()
        .map_err(|_| "Privacy history is unavailable")? = History::default();
    Ok(())
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Metadata {
    name: String,
    rect: [u32; 4],
    operation: String,
    current_revision: u32,
}

#[tauri::command]
pub async fn shell_privacy_image(app: AppHandle, request: Request<'_>) -> Result<Response, String> {
    let (source, metadata): (_, Metadata) = crate::pixel_request::decode(&request)?;
    tauri::async_runtime::spawn_blocking(move || {
        let history = app.state::<PrivacyHistory>();
        let mut history = history
            .0
            .lock()
            .map_err(|_| "Privacy history is unavailable")?;
        let state = app.state::<crate::shell::ImageSession>();
        let mut session = state.0.lock().map_err(|_| "Image state is unavailable")?;
        apply(&mut history, &mut session, source, metadata).map(Response::new)
    })
    .await
    .map_err(|e| format!("Privacy editing stopped: {e}"))?
}
#[tauri::command]
pub async fn shell_restore_privacy(app: AppHandle, revision: u32) -> Result<Response, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let history = app.state::<PrivacyHistory>();
        let mut history = history
            .0
            .lock()
            .map_err(|_| "Privacy history is unavailable")?;
        let state = app.state::<crate::shell::ImageSession>();
        let mut session = state.0.lock().map_err(|_| "Image state is unavailable")?;
        restore(&mut history, &mut session, revision).map(Response::new)
    })
    .await
    .map_err(|e| format!("Privacy history stopped: {e}"))?
}
fn restore(
    history: &mut History,
    session: &mut crate::shell::Session,
    revision: u32,
) -> Result<Vec<u8>, String> {
    let snapshot = history
        .snapshots
        .get(&revision)
        .ok_or("That background revision is unavailable")?;
    let packet = snapshot.packet(revision)?;
    *session = snapshot.session();
    history.current = revision;
    Ok(packet)
}
fn apply(
    history: &mut History,
    session: &mut crate::shell::Session,
    source: RasterImage,
    metadata: Metadata,
) -> Result<Vec<u8>, String> {
    apply_with_budget(history, session, source, metadata, MAX_HISTORY_BYTES)
}

fn apply_with_budget(
    history: &mut History,
    session: &mut crate::shell::Session,
    source: RasterImage,
    metadata: Metadata,
    budget: usize,
) -> Result<Vec<u8>, String> {
    if history.current != metadata.current_revision
        || session.editable.is_some() && session.preview.as_ref() != Some(&source)
        || history
            .snapshots
            .get(&history.current)
            .is_some_and(|s| s.source != source)
    {
        return Err("The background changed. Retry the privacy edit on the current image.".into());
    }
    let operation = match metadata.operation.as_str() {
        "redact" => scribblecap_core::privacy::PrivacyOperation::Redact,
        "pixelate" => scribblecap_core::privacy::PrivacyOperation::Pixelate { block: 12 },
        _ => return Err("Unknown privacy operation".into()),
    };
    let (background, mut imported) = if let Some(editable) = &session.editable {
        let mut document = editable.document.clone();
        document.elements.clear();
        let raster = match &document.background {
            Some(bg) => Some(
                scribblecap_formats::decode(
                    &editable
                        .resources
                        .iter()
                        .find(|r| r.id == bg.resource)
                        .ok_or("Missing background resource")?
                        .bytes,
                )
                .map_err(|e| e.to_string())?
                .image,
            ),
            None => None,
        };
        let bg = match (&document.background, &raster) {
            (Some(bg), Some(raster)) => Some(
                BackgroundImage::new(
                    &bg.resource,
                    raster.width(),
                    raster.height(),
                    raster.pixels(),
                )
                .map_err(|e| e.to_string())?,
            ),
            _ => None,
        };
        let rendered = render(&document, bg).map_err(|e| e.to_string())?;
        (
            RasterImage::new(rendered.width(), rendered.height(), rendered.into_pixels())
                .map_err(|e| e.to_string())?,
            Some(editable.document.clone()),
        )
    } else {
        (source.clone(), None)
    };
    let clean = scribblecap_core::privacy::apply(
        background.width(),
        background.height(),
        background.pixels(),
        metadata.rect,
        operation,
    )
    .map_err(|e| e.to_string())?;
    let clean =
        RasterImage::new(clean.width, clean.height, clean.pixels).map_err(|e| e.to_string())?;
    let (preview, editable) = if let Some(document) = &mut imported {
        let id = ResourceId("scribblecap-private-background".into());
        // Normalize the background onto the canvas and drop unused resources;
        // no old or off-canvas source pixels are carried into the current save.
        document.background = Some(Background {
            resource: id.clone(),
            origin: Point::new(0.0, 0.0),
        });
        document.resources = vec![Resource {
            id: id.clone(),
            media_type: "image/png".into(),
        }];
        let bytes =
            scribblecap_formats::encode(&clean, RasterFormat::Png).map_err(|e| e.to_string())?;
        let rendered = render(
            document,
            Some(
                BackgroundImage::new(&id, clean.width(), clean.height(), clean.pixels())
                    .map_err(|e| e.to_string())?,
            ),
        )
        .map_err(|e| e.to_string())?;
        let preview = RasterImage::new(rendered.width(), rendered.height(), rendered.into_pixels())
            .map_err(|e| e.to_string())?;
        let editable = EditableDocument::new(document.clone(), vec![ResourceData { id, bytes }])
            .map_err(|e| e.to_string())?;
        (preview, Some(editable))
    } else {
        (clean, None)
    };
    let snapshot = Snapshot {
        name: metadata.name.clone(),
        source: preview,
        editable,
    };
    let revision = history
        .next
        .checked_add(1)
        .ok_or("Background revision limit reached")?;
    let baseline_weight = if history.snapshots.is_empty() {
        source.pixels().len()
            + session.editable.as_ref().map_or(0, |e| {
                e.resources.iter().map(|r| r.bytes.len()).sum::<usize>()
            })
    } else {
        0
    };
    let bytes = history
        .snapshots
        .values()
        .map(Snapshot::weight)
        .sum::<usize>()
        + baseline_weight
        + snapshot.weight();
    if bytes > budget {
        return Err("Privacy undo history reached its 512 MiB resource limit. Save the current image, then reopen the saved file to clear session history and continue privacy editing.".into());
    }
    let packet = snapshot.packet(revision)?;
    if history.snapshots.is_empty() {
        history.snapshots.insert(
            0,
            Snapshot {
                name: metadata.name,
                source,
                editable: session.editable.clone(),
            },
        );
    }
    *session = snapshot.session();
    history.snapshots.insert(revision, snapshot);
    history.current = revision;
    history.next = revision;
    Ok(packet)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scribblecap_core::{Color, Document, Element, ElementId, Line, Shape, Size};
    fn metadata(revision: u32, operation: &str) -> Metadata {
        Metadata {
            name: "secret.png".into(),
            rect: [0, 0, 2, 1],
            operation: operation.into(),
            current_revision: revision,
        }
    }
    fn source() -> RasterImage {
        RasterImage::new(2, 1, vec![17, 31, 73, 255, 19, 37, 79, 255]).unwrap()
    }
    #[test]
    fn imported_annotations_survive_and_only_current_background_is_embedded_in_saves() {
        let mut doc = Document::new(Size::new(2.0, 1.0));
        doc.elements.push(Element {
            id: ElementId::from("line"),
            shape: Shape::Line(Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(2.0, 0.0),
                width: 1.0,
                color: Color::rgb(220, 0, 0),
            }),
        });
        let prepared =
            crate::output::prepare(source(), &doc.to_json().unwrap(), None, true).unwrap();
        let original = prepared.editable.clone().unwrap();
        let mut session = crate::shell::Session {
            editable: prepared.editable,
            preview: Some(prepared.preview.clone()),
            target: Some("old.png".into()),
            capture_name: None,
        };
        let mut history = History::default();
        apply(
            &mut history,
            &mut session,
            prepared.preview,
            metadata(0, "redact"),
        )
        .unwrap();
        assert!(session.target.is_none());
        let current = session.editable.as_ref().unwrap();
        assert_eq!(current.document.elements, original.document.elements);
        assert_eq!(history.snapshots.len(), 2);
        let clean = scribblecap_formats::decode(&current.resources[0].bytes)
            .unwrap()
            .image;
        assert_eq!(clean.pixels(), &[0, 0, 0, 255, 0, 0, 0, 255]);
        let overlay = Document::new(Size::new(2.0, 1.0)).to_json().unwrap();
        let output = crate::output::prepare(
            session.preview.clone().unwrap(),
            &overlay,
            session.editable.as_ref(),
            true,
        )
        .unwrap();
        let bytes = scribblecap_formats::encode_editable(
            &output.preview,
            RasterFormat::Png,
            &output.editable.unwrap(),
        )
        .unwrap();
        let saved = scribblecap_formats::extract_editable(&bytes)
            .unwrap()
            .unwrap();
        assert_eq!(saved.resources.len(), 1);
        assert_eq!(saved.document.elements, original.document.elements);
        assert_eq!(
            scribblecap_formats::decode(&saved.resources[0].bytes)
                .unwrap()
                .image
                .pixels(),
            clean.pixels()
        );
        restore(&mut history, &mut session, 0).unwrap();
        assert_eq!(
            session.editable.as_ref().unwrap().resources,
            original.resources
        );
        restore(&mut history, &mut session, 1).unwrap();
        assert_eq!(
            scribblecap_formats::decode(&session.editable.as_ref().unwrap().resources[0].bytes)
                .unwrap()
                .image
                .pixels(),
            clean.pixels()
        );
    }
    #[test]
    fn background_undo_redo_and_branching_use_distinct_revisions() {
        let mut history = History::default();
        let mut session = crate::shell::Session::default();
        apply(&mut history, &mut session, source(), metadata(0, "redact")).unwrap();
        assert_eq!(history.current, 1);
        restore(&mut history, &mut session, 0).unwrap();
        apply(
            &mut history,
            &mut session,
            source(),
            metadata(0, "pixelate"),
        )
        .unwrap();
        assert_eq!(history.current, 2);
        assert_ne!(history.snapshots[&2].source, history.snapshots[&1].source);
        let packet = restore(&mut history, &mut session, 2).unwrap();
        assert_eq!(&packet[..4], &2_u32.to_le_bytes());
    }
    #[test]
    fn privacy_edits_and_undo_make_a_capture_save_ask_for_a_destination() {
        use crate::output::{SaveTarget, save_target};
        let saved = |session: &crate::shell::Session| {
            save_target(
                false,
                false,
                session.target.as_deref(),
                session.capture_name.is_some(),
            )
        };
        let mut history = History::default();
        // An auto-saved capture: Save would overwrite its file in place.
        let mut session = crate::shell::Session {
            preview: Some(source()),
            target: Some("/c/Screenshot 2026-11-25 10.00.00.png".into()),
            capture_name: Some("Screenshot 2026-11-25 10.00.00".into()),
            ..Default::default()
        };
        assert!(matches!(saved(&session), SaveTarget::Existing(_)));
        apply(&mut history, &mut session, source(), metadata(0, "redact")).unwrap();
        assert_eq!(saved(&session), SaveTarget::Dialog);
        // Undo back to the original pixels must not resume silent capture saves either.
        restore(&mut history, &mut session, 0).unwrap();
        assert_eq!(saved(&session), SaveTarget::Dialog);
        restore(&mut history, &mut session, 1).unwrap();
        assert_eq!(saved(&session), SaveTarget::Dialog);
    }
    #[test]
    fn exhausted_history_budget_rejects_without_discarding_undo_or_current_pixels() {
        let mut history = History::default();
        let mut session = crate::shell::Session::default();
        apply(&mut history, &mut session, source(), metadata(0, "redact")).unwrap();
        let current = history.snapshots[&1].source.clone();
        let before = history.snapshots.len();
        let error = apply_with_budget(
            &mut history,
            &mut session,
            current,
            metadata(1, "pixelate"),
            1,
        )
        .unwrap_err();
        assert!(error.contains("Save the current image, then reopen the saved file"));
        assert_eq!(history.current, 1);
        assert_eq!(history.snapshots.len(), before);
        restore(&mut history, &mut session, 0).unwrap();
        assert_eq!(history.snapshots[&0].source, source());
    }

    #[test]
    fn invalid_request_and_invalid_restore_preserve_session_and_history() {
        let mut history = History::default();
        let mut session = crate::shell::Session {
            target: Some("keep.png".into()),
            ..Default::default()
        };
        let mut bad = metadata(0, "redact");
        bad.rect[2] = 3;
        assert!(apply(&mut history, &mut session, source(), bad).is_err());
        assert!(history.snapshots.is_empty());
        assert!(restore(&mut history, &mut session, 99).is_err());
        assert_eq!(
            session.target.unwrap(),
            std::path::PathBuf::from("keep.png")
        );
    }
}
