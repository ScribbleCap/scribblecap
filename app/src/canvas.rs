//! Canvas operations on explicit RGBA input; no filesystem access or mutable global state.
use scribblecap_core::canvas::{CanvasOperation, transform};
use scribblecap_formats::RasterImage;
use tauri::{AppHandle, Manager, ipc::Response};

#[allow(clippy::too_many_arguments)] // Explicit IPC fields keep the existing pixel packet boundary.
#[tauri::command]
pub async fn shell_transform_image(
    app: AppHandle,
    name: String,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    operation: String,
    output_width: u32,
    output_height: u32,
    x: u32,
    y: u32,
) -> Result<Response, String> {
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
        let image = transform(width, height, &pixels, operation).map_err(|e| e.to_string())?;
        let image =
            RasterImage::new(image.width, image.height, image.pixels).map_err(|e| e.to_string())?;
        crate::shell::packet(&name, image).map(Response::new)
    })
    .await
    .map_err(|e| format!("Canvas transform stopped: {e}"))?
}
