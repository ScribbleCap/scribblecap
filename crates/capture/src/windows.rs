//! Windows.Graphics.Capture, Windows 10 2004+. All native calls stay here.
#![allow(unsafe_code)]

use std::{
    mem::size_of,
    sync::Mutex,
    time::{Duration, Instant},
};

use crate::windows_image::{MAX_IMAGE_BYTES, PixelRect, bgra_to_rgba, crop_rect};
use crate::{
    CaptureError, CapturedImage, Capturer, DisplayInfo, Result, ScreenRect, WindowInfo,
    region_on_display,
};
use ::windows::{
    Foundation::Metadata::ApiInformation,
    Graphics::{
        Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession},
        DirectX::{Direct3D11::IDirect3DDevice, DirectXPixelFormat},
    },
    Win32::{
        Foundation::{CloseHandle, E_ACCESSDENIED, HMODULE, HWND, LPARAM, RECT},
        Graphics::{
            Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP},
            Direct3D11::*,
            Dwm::{DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute},
            Dxgi::{Common::DXGI_SAMPLE_DESC, IDXGIDevice},
            Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO},
        },
        System::{
            Threading::{
                OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
                QueryFullProcessImageNameW,
            },
            WinRT::{
                Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess},
                Graphics::Capture::IGraphicsCaptureItemInterop,
                RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize,
            },
        },
        UI::{
            HiDpi::{
                DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForSystem,
                SetThreadDpiAwarenessContext,
            },
            WindowsAndMessaging::*,
        },
    },
    core::{HSTRING, Interface, factory},
};

const TIMEOUT: Duration = Duration::from_secs(10);
// Affinity is process-wide window state, so serialize captures across instances.
static CAPTURE_LOCK: Mutex<()> = Mutex::new(());

pub(crate) struct WindowsCapturer;

/// Windows handles have 32 significant bits even on 64-bit Windows. Never
/// reconstruct a pointer from an ID: resolve it against a fresh enumeration.
/// https://learn.microsoft.com/windows/win32/winprog64/interprocess-communication
fn handle_id(handle: usize) -> u32 {
    handle as u32
}

impl WindowsCapturer {
    pub(crate) fn new() -> Result<Self> {
        worker(|| {
            if !ApiInformation::IsApiContractPresentByMajor(
                &HSTRING::from("Windows.Foundation.UniversalApiContract"),
                10,
            )
            .unwrap_or(false)
            {
                return Err(CaptureError::Unsupported(
                    "needs Windows 10 version 2004 or later",
                ));
            }
            if !GraphicsCaptureSession::IsSupported().unwrap_or(false) {
                return Err(CaptureError::Unsupported(
                    "Windows.Graphics.Capture is unavailable",
                ));
            }
            Ok(())
        })?;
        Ok(Self)
    }

    fn monitors(&self) -> Result<Vec<Monitor>> {
        let mut result = Vec::new();
        // SAFETY: callback is synchronous; lparam points to the live Vec exclusively
        // borrowed for this call. No pointer escapes the callback.
        unsafe {
            EnumDisplayMonitors(
                None,
                None,
                Some(monitor_callback),
                LPARAM((&mut result as *mut Vec<HMONITOR>) as isize),
            )
            .ok()?;
        }
        // A single desktop-point scale avoids overlaps/gaps between monitors at
        // different DPIs. PMv2 makes native rectangles physical pixels.
        // SAFETY: no pointers; executed on our isolated DPI-aware worker.
        let scale = f64::from(unsafe { GetDpiForSystem() }.max(96)) / 96.0;
        result
            .into_iter()
            .map(|handle| {
                let mut info = MONITORINFO {
                    cbSize: size_of::<MONITORINFO>() as u32,
                    ..Default::default()
                };
                // SAFETY: enumerated handle and correctly sized writable structure.
                unsafe {
                    GetMonitorInfoW(handle, &mut info).ok()?;
                }
                Ok(Monitor {
                    handle,
                    primary: info.dwFlags & MONITORINFOF_PRIMARY != 0,
                    info: DisplayInfo {
                        id: handle_id(handle.0 as usize),
                        bounds: rect(info.rcMonitor, scale),
                        scale,
                    },
                })
            })
            .collect()
    }

    fn windows(&self) -> Result<Vec<(HWND, WindowInfo)>> {
        let scale = desktop_scale();
        let mut result = Vec::new();
        for handle in window_handles()? {
            if !listed(handle) {
                continue;
            }
            let bounds = match window_bounds(handle) {
                Ok(r) => rect(r, scale),
                Err(_) => continue,
            };
            let title = window_title(handle);
            result.push((
                handle,
                WindowInfo {
                    id: handle_id(handle.0 as usize),
                    title,
                    app_name: app_name(handle),
                    bounds,
                },
            ));
        }
        Ok(result)
    }
}

impl Capturer for WindowsCapturer {
    // Win32 direct capture has no persistent permission prompt. Target-specific
    // denials are reported by CreateForWindow/Monitor or StartCapture.
    fn has_permission(&self) -> bool {
        true
    }
    fn request_permission(&self) -> bool {
        true
    }
    fn displays(&self) -> Result<Vec<DisplayInfo>> {
        worker(|| Ok(self.monitors()?.into_iter().map(|m| m.info).collect()))
    }
    fn list_windows(&self) -> Result<Vec<WindowInfo>> {
        worker(|| Ok(self.windows()?.into_iter().map(|(_, w)| w).collect()))
    }
    fn capture_screen(&self, display: Option<u32>) -> Result<CapturedImage> {
        worker(|| {
            let monitors = self.monitors()?;
            let monitor = monitors
                .iter()
                .find(|m| display.map_or(m.primary, |id| id == m.info.id))
                .ok_or(CaptureError::NotFound)?;
            capture_monitor(monitor, None)
        })
    }
    fn capture_window(&self, id: u32) -> Result<CapturedImage> {
        worker(|| {
            let windows = self.windows()?;
            let (handle, _) = windows
                .iter()
                .find(|(_, w)| w.id == id)
                .ok_or(CaptureError::NotFound)?;
            let _lock = CAPTURE_LOCK
                .lock()
                .map_err(|_| CaptureError::Os("capture lock poisoned".into()))?;
            let interop: IGraphicsCaptureItemInterop = factory::<GraphicsCaptureItem, _>()?;
            // SAFETY: HWND is from the current top-level window enumeration.
            let item = unsafe { interop.CreateForWindow(*handle)? };
            capture(item, desktop_scale(), None)
        })
    }
    fn capture_region(&self, region: ScreenRect) -> Result<CapturedImage> {
        worker(|| {
            if ![region.x, region.y, region.width, region.height]
                .iter()
                .all(|v| v.is_finite())
                || region.width <= 0.0
                || region.height <= 0.0
            {
                return Err(CaptureError::NotFound);
            }
            let monitors = self.monitors()?;
            let infos: Vec<_> = monitors.iter().map(|m| m.info.clone()).collect();
            let (info, clipped) =
                region_on_display(&region, &infos).ok_or(CaptureError::NotFound)?;
            let monitor = monitors
                .iter()
                .find(|m| m.info.id == info.id)
                .ok_or(CaptureError::NotFound)?;
            capture_monitor(monitor, Some(crop_rect(clipped, info.bounds, info.scale)?))
        })
    }
}

struct Monitor {
    handle: HMONITOR,
    info: DisplayInfo,
    primary: bool,
}

fn desktop_scale() -> f64 {
    // SAFETY: no pointers; worker has PMv2 DPI awareness.
    f64::from(unsafe { GetDpiForSystem() }.max(96)) / 96.0
}
fn rect(r: RECT, scale: f64) -> ScreenRect {
    ScreenRect::new(
        f64::from(r.left) / scale,
        f64::from(r.top) / scale,
        (f64::from(r.right) - f64::from(r.left)) / scale,
        (f64::from(r.bottom) - f64::from(r.top)) / scale,
    )
}

/// A fresh MTA thread avoids changing the caller's COM apartment or DPI context.
fn worker<T: Send>(f: impl FnOnce() -> Result<T> + Send) -> Result<T> {
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                // SAFETY: fresh thread, balanced by Apartment after all COM objects drop.
                unsafe {
                    RoInitialize(RO_INIT_MULTITHREADED)?;
                }
                let _apartment = Apartment;
                // SAFETY: only changes this disposable worker thread.
                if unsafe {
                    SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
                }
                .0
                .is_null()
                {
                    return Err(::windows::core::Error::from_thread().into());
                }
                f()
            })
            .join()
            .map_err(|_| CaptureError::Os("Windows capture worker panicked".into()))?
    })
}
struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: balances successful RoInitialize on this worker thread.
        unsafe {
            RoUninitialize();
        }
    }
}

unsafe extern "system" fn monitor_callback(
    m: HMONITOR,
    _: HDC,
    _: *mut RECT,
    data: LPARAM,
) -> ::windows::core::BOOL {
    // SAFETY: only called synchronously by monitors() with a live Vec pointer.
    unsafe {
        (&mut *(data.0 as *mut Vec<HMONITOR>)).push(m);
    }
    true.into()
}
unsafe extern "system" fn window_callback(w: HWND, data: LPARAM) -> ::windows::core::BOOL {
    // SAFETY: only called synchronously by window_handles() with a live Vec pointer.
    unsafe {
        (&mut *(data.0 as *mut Vec<HWND>)).push(w);
    }
    true.into()
}
fn window_handles() -> Result<Vec<HWND>> {
    let mut handles = Vec::new();
    // SAFETY: callback is synchronous and receives an exclusive live Vec pointer.
    unsafe {
        EnumWindows(
            Some(window_callback),
            LPARAM((&mut handles as *mut Vec<HWND>) as isize),
        )?;
    }
    Ok(handles)
}
fn process_id(w: HWND) -> u32 {
    let mut pid = 0;
    // SAFETY: writable PID output; Windows validates the potentially stale HWND.
    unsafe {
        GetWindowThreadProcessId(w, Some(&mut pid));
    }
    pid
}
fn listed(w: HWND) -> bool {
    // SAFETY: scalar queries; Windows validates HWND, which may have closed.
    unsafe {
        if !IsWindowVisible(w).as_bool()
            || IsIconic(w).as_bool()
            || process_id(w) == std::process::id()
            || GetWindowLongPtrW(w, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 != 0
        {
            return false;
        }
        let mut cloaked = 0u32;
        if DwmGetWindowAttribute(
            w,
            DWMWA_CLOAKED,
            (&mut cloaked as *mut u32).cast(),
            size_of::<u32>() as u32,
        )
        .is_err()
            || cloaked != 0
        {
            return false;
        }
    }
    window_bounds(w).is_ok_and(|r| r.right > r.left && r.bottom > r.top)
        && !window_title(w).is_empty()
}
fn window_bounds(w: HWND) -> Result<RECT> {
    let mut r = RECT::default();
    // SAFETY: correctly sized writable RECT; DWM excludes invisible resize borders.
    unsafe {
        DwmGetWindowAttribute(
            w,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&mut r as *mut RECT).cast(),
            size_of::<RECT>() as u32,
        )?;
    }
    Ok(r)
}
fn window_title(w: HWND) -> String {
    // SAFETY: bounded writable UTF-16 buffer; GetWindowTextW never exceeds it.
    unsafe {
        let mut text = vec![0u16; (GetWindowTextLengthW(w).max(0) as usize + 1).min(32768)];
        let n = GetWindowTextW(w, &mut text).max(0) as usize;
        String::from_utf16_lossy(&text[..n])
    }
}
fn app_name(w: HWND) -> String {
    // SAFETY: limited query access only; owned process handle closed before return.
    unsafe {
        let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id(w))
        else {
            return String::new();
        };
        let mut text = vec![0u16; 32768];
        let mut size = text.len() as u32;
        let query = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            ::windows::core::PWSTR(text.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(process);
        if query.is_err() {
            return String::new();
        }
        let path = String::from_utf16_lossy(&text[..size as usize]);
        std::path::Path::new(&path)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

/// Restore the original affinity even if setup or capture fails halfway through.
#[derive(Default)]
struct ExcludedWindows(Vec<(HWND, u32)>);
impl ExcludedWindows {
    fn new() -> Result<Self> {
        let mut guard = Self::default();
        for w in window_handles()? {
            if process_id(w) != std::process::id() {
                continue;
            }
            // SAFETY: own top-level windows only; remember original state first.
            unsafe {
                if !IsWindowVisible(w).as_bool() {
                    continue;
                }
                let mut original = 0;
                GetWindowDisplayAffinity(w, &mut original)?;
                SetWindowDisplayAffinity(w, WDA_EXCLUDEFROMCAPTURE)?;
                guard.0.push((w, original));
            }
        }
        Ok(guard)
    }
    fn restore(&mut self) -> Result<()> {
        let mut failure = None;
        for (w, affinity) in &self.0 {
            // SAFETY: restore saved state on own still-existing top-level windows.
            unsafe {
                if IsWindow(Some(*w)).as_bool()
                    && process_id(*w) == std::process::id()
                    && let Err(e) = SetWindowDisplayAffinity(*w, WINDOW_DISPLAY_AFFINITY(*affinity))
                {
                    failure = Some(e);
                }
            }
        }
        if let Some(e) = failure {
            Err(e.into())
        } else {
            self.0.clear();
            Ok(())
        }
    }
}
impl Drop for ExcludedWindows {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}
fn capture_monitor(monitor: &Monitor, crop: Option<PixelRect>) -> Result<CapturedImage> {
    let _lock = CAPTURE_LOCK
        .lock()
        .map_err(|_| CaptureError::Os("capture lock poisoned".into()))?;
    let mut excluded = ExcludedWindows::new()?;
    let interop: IGraphicsCaptureItemInterop = factory::<GraphicsCaptureItem, _>()?;
    // SAFETY: HMONITOR comes from current EnumDisplayMonitors enumeration.
    let item = unsafe { interop.CreateForMonitor(monitor.handle)? };
    let result = capture(item, monitor.info.scale, crop);
    excluded.restore()?;
    result
}

struct CaptureObjects {
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
}
impl Drop for CaptureObjects {
    fn drop(&mut self) {
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}
fn capture(
    item: GraphicsCaptureItem,
    scale: f64,
    crop: Option<PixelRect>,
) -> Result<CapturedImage> {
    let (device, context) = device()?;
    let dxgi: IDXGIDevice = device.cast()?;
    // SAFETY: valid owned D3D11 device with BGRA support.
    let direct: IDirect3DDevice = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi)? }.cast()?;
    let size = item.Size()?;
    if size.Width <= 0 || size.Height <= 0 {
        return Err(CaptureError::NotFound);
    }
    if (size.Width as usize)
        .checked_mul(size.Height as usize)
        .and_then(|n| n.checked_mul(4))
        .is_none_or(|n| n > MAX_IMAGE_BYTES)
    {
        return Err(CaptureError::Os("capture image exceeds 512 MiB".into()));
    }
    let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
        &direct,
        DirectXPixelFormat::B8G8R8A8UIntNormalized,
        1,
        size,
    )?;
    let session = match pool.CreateCaptureSession(&item) {
        Ok(s) => s,
        Err(e) => {
            let _ = pool.Close();
            return Err(e.into());
        }
    };
    let objects = CaptureObjects { pool, session };
    objects.session.SetIsCursorCaptureEnabled(false)?;
    objects.session.StartCapture()?;
    let start = Instant::now();
    loop {
        // FreeThreaded owns its dispatcher; no UI message loop is required.
        match objects.pool.TryGetNextFrame() {
            Ok(frame) => {
                let result = (|| {
                    let content = frame.ContentSize()?;
                    if content.Width != size.Width || content.Height != size.Height {
                        return Err(CaptureError::Os(
                            "capture target resized; retry capture".into(),
                        ));
                    }
                    let access: IDirect3DDxgiInterfaceAccess = frame.Surface()?.cast()?;
                    // SAFETY: surface is a D3D11 texture owned by the live frame.
                    let texture: ID3D11Texture2D = unsafe { access.GetInterface()? };
                    read_texture(
                        &device,
                        &context,
                        &texture,
                        size.Width as u32,
                        size.Height as u32,
                        scale,
                        crop,
                    )
                })();
                let closed = frame.Close();
                let image = result?;
                closed?;
                return Ok(image);
            }
            // A null frame is projected as E_POINTER while the pool is empty.
            Err(e) if e.code() == ::windows::Win32::Foundation::E_POINTER => {}
            Err(e) => return Err(e.into()),
        }
        if start.elapsed() >= TIMEOUT {
            return Err(CaptureError::Os(
                "Windows capture did not deliver a frame within 10 seconds".into(),
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn device() -> Result<(ID3D11Device, ID3D11DeviceContext)> {
    let mut last = None;
    for driver in [D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP] {
        let (mut device, mut context) = (None, None);
        // SAFETY: valid output pointers; all optional inputs are null/default.
        let result = unsafe {
            D3D11CreateDevice(
                None,
                driver,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
        };
        match result {
            Ok(()) => {
                let device: ID3D11Device =
                    device.ok_or_else(|| CaptureError::Os("missing D3D device".into()))?;
                let context: ID3D11DeviceContext =
                    context.ok_or_else(|| CaptureError::Os("missing D3D context".into()))?;
                let multithread: ID3D11Multithread = context.cast()?;
                // SAFETY: WGC and readback share the device; serialize native context access.
                unsafe {
                    let _ = multithread.SetMultithreadProtected(true);
                }
                return Ok((device, context));
            }
            Err(e) => last = Some(e),
        }
    }
    Err(last.expect("two device creation attempts").into())
}
fn read_texture(
    device: &ID3D11Device,
    context: &ID3D11DeviceContext,
    texture: &ID3D11Texture2D,
    width: u32,
    height: u32,
    scale: f64,
    crop: Option<PixelRect>,
) -> Result<CapturedImage> {
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    // SAFETY: live texture and writable descriptor.
    unsafe {
        texture.GetDesc(&mut desc);
    }
    if width > desc.Width || height > desc.Height {
        return Err(CaptureError::Os(
            "capture texture is smaller than its content".into(),
        ));
    }
    desc.Usage = D3D11_USAGE_STAGING;
    desc.BindFlags = 0;
    desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
    desc.MiscFlags = 0;
    desc.SampleDesc = DXGI_SAMPLE_DESC {
        Count: 1,
        Quality: 0,
    };
    let mut staging = None;
    // SAFETY: descriptor uses the source format/size and CPU-read staging flags.
    unsafe {
        device.CreateTexture2D(&desc, None, Some(&mut staging))?;
    }
    let staging = staging.ok_or_else(|| CaptureError::Os("missing staging texture".into()))?;
    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    // SAFETY: same-size/same-format resources on this device; Map waits for copy.
    unsafe {
        context.CopyResource(&staging, texture);
        context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
    }
    let result = (|| {
        let len = (mapped.RowPitch as usize)
            .checked_mul(height as usize)
            .filter(|n| *n <= MAX_IMAGE_BYTES)
            .ok_or_else(|| CaptureError::Os("capture buffer exceeds 512 MiB".into()))?;
        if mapped.pData.is_null() {
            return Err(CaptureError::Os("null capture buffer".into()));
        }
        // SAFETY: successful Map exposes RowPitch × texture-height bytes. Requested
        // height was bounded by texture height above; slice is consumed before Unmap.
        let data = unsafe { std::slice::from_raw_parts(mapped.pData.cast::<u8>(), len) };
        bgra_to_rgba(data, width, height, mapped.RowPitch as usize, crop, scale)
    })();
    // SAFETY: balances the successful Map, including conversion failures.
    unsafe {
        context.Unmap(&staging, 0);
    }
    result
}

impl From<::windows::core::Error> for CaptureError {
    fn from(error: ::windows::core::Error) -> Self {
        if error.code() == E_ACCESSDENIED {
            Self::PermissionDenied
        } else {
            Self::Os(error.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_sets_dpi_awareness_without_changing_the_callers_context() {
        use ::windows::Win32::UI::HiDpi::{
            AreDpiAwarenessContextsEqual, GetThreadDpiAwarenessContext,
        };
        // SAFETY: scalar context queries; no ownership transfer or pointers.
        let before = unsafe { GetThreadDpiAwarenessContext() };
        let aware = worker(|| {
            // SAFETY: query this worker's context against a valid constant.
            Ok(unsafe {
                AreDpiAwarenessContextsEqual(
                    GetThreadDpiAwarenessContext(),
                    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
                )
            }
            .as_bool())
        })
        .unwrap();
        assert!(aware);
        // SAFETY: scalar context query; the original context remains valid.
        assert!(
            unsafe { AreDpiAwarenessContextsEqual(before, GetThreadDpiAwarenessContext()) }
                .as_bool()
        );
    }

    #[test]
    fn native_ids_preserve_significant_bits_across_pointer_widths() {
        assert_eq!(handle_id(0x8000_0001), 0x8000_0001);
        #[cfg(target_pointer_width = "64")]
        assert_eq!(handle_id(0xffff_ffff_8000_0001), 0x8000_0001);
    }
    #[test]
    fn access_denied_maps_to_permission_error() {
        assert_eq!(
            CaptureError::from(::windows::core::Error::from_hresult(E_ACCESSDENIED)),
            CaptureError::PermissionDenied
        );
    }

    #[test]
    #[ignore = "requires an unlocked interactive Windows desktop and a visible application window"]
    fn interactive_capture_smoke_test() {
        let backend = WindowsCapturer::new().unwrap();
        let displays = backend.displays().unwrap();
        assert!(!displays.is_empty());
        let check = |image: CapturedImage| {
            assert!(image.width > 0 && image.height > 0);
            assert_eq!(
                image.pixels.len(),
                image.width as usize * image.height as usize * 4
            );
            assert!(image.scale.is_finite() && image.scale > 0.0);
        };
        check(backend.capture_screen(None).unwrap());
        for display in &displays {
            check(backend.capture_screen(Some(display.id)).unwrap());
            let region =
                ScreenRect::new(display.bounds.x + 10.0, display.bounds.y + 10.0, 32.0, 24.0);
            let image = backend.capture_region(region).unwrap();
            assert_eq!(image.width, (32.0 * display.scale).round() as u32);
            assert_eq!(image.height, (24.0 * display.scale).round() as u32);
            check(image);
        }
        let windows = backend.list_windows().unwrap();
        let window = windows
            .first()
            .expect("open an application window for this test");
        check(backend.capture_window(window.id).unwrap());
        assert_eq!(backend.capture_window(0), Err(CaptureError::NotFound));
        assert_eq!(
            backend.capture_region(ScreenRect::new(f64::NAN, 0.0, 1.0, 1.0)),
            Err(CaptureError::NotFound)
        );
    }
}
