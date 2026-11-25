//! A privacy commit flattens annotations, replaces the original resource and
//! clears the native editable session so future saves cannot embed old pixels.
use scribblecap_formats::RasterImage;
use tauri::{
    AppHandle, Manager,
    ipc::{Request, Response},
};

#[derive(serde::Deserialize)]
struct Metadata {
    name: String,
    document: String,
    rect: [u32; 4],
    operation: String,
}

#[tauri::command]
pub async fn shell_privacy_image(app: AppHandle, request: Request<'_>) -> Result<Response, String> {
    let (source, metadata): (_, Metadata) = crate::pixel_request::decode(&request)?;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<crate::shell::ImageSession>();
        let mut session = state.0.lock().map_err(|_| "Image state is unavailable")?;
        process(&mut session, source, metadata).map(Response::new)
    })
    .await
    .map_err(|e| format!("Privacy editing stopped: {e}"))?
}

fn process(
    session: &mut crate::shell::Session,
    source: RasterImage,
    metadata: Metadata,
) -> Result<Vec<u8>, String> {
    if session.editable.is_some() && session.preview.as_ref() != Some(&source) {
        return Err(
            "The editable preview changed. Reopen the image before applying privacy edits.".into(),
        );
    }
    let operation = match metadata.operation.as_str() {
        "redact" => scribblecap_core::privacy::PrivacyOperation::Redact,
        "pixelate" => scribblecap_core::privacy::PrivacyOperation::Pixelate { block: 12 },
        _ => return Err("Unknown privacy operation".into()),
    };
    let flat =
        crate::output::prepare(source, &metadata.document, session.editable.as_ref(), false)?
            .preview;
    let clean = scribblecap_core::privacy::apply(
        flat.width(),
        flat.height(),
        flat.pixels(),
        metadata.rect,
        operation,
    )
    .map_err(|e| e.to_string())?;
    let clean =
        RasterImage::new(clean.width, clean.height, clean.pixels).map_err(|e| e.to_string())?;
    let name = format!(
        "{}-private.png",
        std::path::Path::new(&metadata.name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Image")
    );
    let packet = crate::shell::packet(&name, clean)?;
    // Clear only after all processing and packet allocation succeeds. The old
    // on-disk file is untouched; the next save asks for a new destination.
    *session = crate::shell::Session::default();
    Ok(packet)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scribblecap_core::{Color, Document, Element, ElementId, Line, Point, Shape, Size};
    fn document() -> String {
        Document::new(Size::new(2.0, 1.0)).to_json().unwrap()
    }
    fn metadata() -> Metadata {
        Metadata {
            name: "secret.png".into(),
            document: document(),
            rect: [0, 0, 2, 1],
            operation: "redact".into(),
        }
    }
    #[test]
    fn privacy_commit_discards_imported_resources_and_subsequent_saves_embed_only_sanitized_pixels()
    {
        let source = RasterImage::new(2, 1, vec![17, 31, 73, 255, 19, 37, 79, 255]).unwrap();
        let mut doc = Document::new(Size::new(2.0, 1.0));
        doc.elements.push(Element {
            id: ElementId::from("line"),
            shape: Shape::Line(Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(1.0, 0.0),
                width: 1.0,
                color: Color::rgb(1, 2, 3),
            }),
        });
        let annotated = doc.to_json().unwrap();
        let prepared = crate::output::prepare(source, &annotated, None, true).unwrap();
        let mut session = crate::shell::Session {
            editable: prepared.editable,
            preview: Some(prepared.preview.clone()),
            target: Some("old.png".into()),
        };
        let packet = process(&mut session, prepared.preview, metadata()).unwrap();
        assert!(
            session.editable.is_none() && session.preview.is_none() && session.target.is_none()
        );
        let name_len = u32::from_le_bytes(packet[16..20].try_into().unwrap()) as usize;
        let clean = RasterImage::new(2, 1, packet[20 + name_len..].to_vec()).unwrap();
        assert_eq!(clean.pixels(), &[0, 0, 0, 255, 0, 0, 0, 255]);
        let saved =
            crate::output::prepare(clean, &annotated, session.editable.as_ref(), true).unwrap();
        let editable = saved.editable.unwrap();
        let bytes = scribblecap_formats::encode_editable(
            &saved.preview,
            scribblecap_formats::RasterFormat::Png,
            &editable,
        )
        .unwrap();
        let extracted = scribblecap_formats::extract_editable(&bytes)
            .unwrap()
            .unwrap();
        assert_eq!(extracted.resources.len(), 1);
        let embedded = scribblecap_formats::decode(&extracted.resources[0].bytes).unwrap();
        assert_eq!(embedded.image.pixels(), &[0, 0, 0, 255, 0, 0, 0, 255]);
    }
    #[test]
    fn invalid_privacy_request_keeps_existing_session() {
        let mut session = crate::shell::Session {
            target: Some("keep.png".into()),
            ..Default::default()
        };
        let source = RasterImage::new(2, 1, vec![0; 8]).unwrap();
        let mut bad = metadata();
        bad.rect[2] = 3;
        assert!(process(&mut session, source, bad).is_err());
        assert_eq!(
            session.target.unwrap(),
            std::path::PathBuf::from("keep.png")
        );
    }
}
