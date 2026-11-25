//! One-shot, user-confirmed capture through the desktop Screenshot portal.
use ashpd::desktop::{ResponseError, screenshot::Screenshot};

use crate::{CaptureError, CapturedImage, Capturer, DisplayInfo, Result, ScreenRect, WindowInfo};

pub(crate) struct WaylandCapturer;

fn portal_error(error: ashpd::Error) -> CaptureError {
    match error {
        ashpd::Error::Response(ResponseError::Cancelled)
        | ashpd::Error::Portal(ashpd::PortalError::Cancelled(_))
        | ashpd::Error::Portal(ashpd::PortalError::NotAllowed(_)) => CaptureError::PermissionDenied,
        ashpd::Error::PortalNotFound(_) => CaptureError::Unsupported(
            "install xdg-desktop-portal and a Screenshot-capable desktop portal backend",
        ),
        other => CaptureError::Os(format!("Screenshot portal: {other}")),
    }
}

impl Capturer for WaylandCapturer {
    fn has_permission(&self) -> bool {
        false // There is no portable persistent permission query for Screenshot.
    }

    fn request_permission(&self) -> bool {
        true // The actual request/confirmation happens in capture_screen.
    }

    fn displays(&self) -> Result<Vec<DisplayInfo>> {
        Err(CaptureError::Unsupported(
            "the Wayland Screenshot portal does not enumerate displays; use screen without an ID",
        ))
    }

    fn list_windows(&self) -> Result<Vec<WindowInfo>> {
        Err(CaptureError::Unsupported(
            "the Wayland Screenshot portal does not enumerate windows; use its interactive picker",
        ))
    }

    fn capture_screen(&self, display: Option<u32>) -> Result<CapturedImage> {
        if display.is_some() {
            return Err(CaptureError::Unsupported(
                "Wayland display IDs are unavailable; use screen without an ID",
            ));
        }
        // The public capture trait is blocking; callers must use a worker thread.
        // A private current-thread runtime keeps portal I/O alive until response.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| CaptureError::Os(error.to_string()))?;
        let uri = runtime.block_on(async {
            let request = Screenshot::request()
                .interactive(true)
                .modal(false)
                .send()
                .await
                .map_err(portal_error)?;
            let response = request.response().map_err(portal_error)?;
            Ok::<_, CaptureError>(response.uri().as_str().to_owned())
        })?;
        crate::wayland_image::load(&uri)
    }

    fn capture_window(&self, _id: u32) -> Result<CapturedImage> {
        Err(CaptureError::Unsupported(
            "Wayland window IDs are unavailable; use screen and select in the portal if offered",
        ))
    }

    fn capture_region(&self, _rect: ScreenRect) -> Result<CapturedImage> {
        Err(CaptureError::Unsupported(
            "the Screenshot portal supplies no desktop coordinates; capture then crop in the editor",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_operations_do_not_start_a_portal_request() {
        let capturer = WaylandCapturer;
        assert!(!capturer.has_permission());
        assert!(capturer.request_permission());
        assert!(matches!(
            capturer.displays(),
            Err(CaptureError::Unsupported(_))
        ));
        assert!(matches!(
            capturer.list_windows(),
            Err(CaptureError::Unsupported(_))
        ));
        for result in [
            capturer.capture_screen(Some(1)),
            capturer.capture_window(1),
            capturer.capture_region(ScreenRect::new(0.0, 0.0, 10.0, 10.0)),
        ] {
            assert!(matches!(result, Err(CaptureError::Unsupported(_))));
        }
    }

    #[test]
    fn cancellation_and_denial_are_permission_errors() {
        for error in [
            ashpd::Error::Response(ResponseError::Cancelled),
            ashpd::Error::Portal(ashpd::PortalError::Cancelled("cancelled".into())),
            ashpd::Error::Portal(ashpd::PortalError::NotAllowed("denied".into())),
        ] {
            assert_eq!(portal_error(error), CaptureError::PermissionDenied);
        }
        assert!(matches!(
            portal_error(ashpd::Error::Response(ResponseError::Other)),
            CaptureError::Os(_)
        ));
    }
}
