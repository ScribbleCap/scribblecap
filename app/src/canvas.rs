//! Canvas operations on explicit RGBA input; no filesystem access or mutable global state.
use scribblecap_core::canvas::{CanvasOperation, transform};
use scribblecap_formats::RasterImage;
use tauri::{AppHandle, Manager, ipc::Response};

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TransformMetadata {
    name: String,
    operation: String,
    output_width: u32,
    output_height: u32,
    x: u32,
    y: u32,
}

#[tauri::command]
pub async fn shell_transform_image(
    app: AppHandle,
    request: tauri::ipc::Request<'_>,
) -> Result<Response, String> {
    let (
        source,
        TransformMetadata {
            name,
            operation,
            output_width,
            output_height,
            x,
            y,
        },
    ) = crate::pixel_request::decode(&request)?;
    tauri::async_runtime::spawn_blocking(move || {
        let session = app.state::<crate::shell::ImageSession>();
        let session = session.0.lock().map_err(|_| "Image state is unavailable".to_string())?;
        if session.editable.is_some() {
            return Err("Canvas transforms for editable annotations are not available yet. Open a flat image to transform it.".into());
        }
        drop(session);
        let operation = match operation.as_str() {
            "crop" => CanvasOperation::Crop {
                x,
                y,
                width: output_width,
                height: output_height,
            },
            "resize" => CanvasOperation::Resize {
                width: output_width,
                height: output_height,
            },
            "expand" => CanvasOperation::Expand {
                x,
                y,
                width: output_width,
                height: output_height,
            },
            "clockwise" => CanvasOperation::RotateClockwise,
            "counterclockwise" => CanvasOperation::RotateCounterclockwise,
            "horizontal" => CanvasOperation::FlipHorizontal,
            "vertical" => CanvasOperation::FlipVertical,
            _ => return Err("Unknown canvas operation".into()),
        };
        let image = transform(source.width(), source.height(), source.pixels(), operation).map_err(|e| e.to_string())?;
        let image =
            RasterImage::new(image.width, image.height, image.pixels).map_err(|e| e.to_string())?;
        let packet = crate::shell::packet(&name, image)?;
        crate::privacy::reset(&app)?;
        Ok(Response::new(packet))
    })
    .await
    .map_err(|e| format!("Canvas transform stopped: {e}"))?
}
