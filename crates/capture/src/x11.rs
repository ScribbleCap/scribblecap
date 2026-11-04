//! Linux X11 backend: `x11rb` (pure-Rust X11 protocol, no libxcb), core `GetImage`,
//! RandR monitors and Composite for covered windows.
//!
//! X11 has no per-display scale factor, so desktop points are X11 pixels and every
//! image has scale 1. X11 lets any client read the screen: there is no permission to
//! ask for. Unlike macOS, the root window can't hide our own windows, so callers
//! should hide them before a screen or region capture.

use x11rb::connection::Connection;
use x11rb::protocol::composite::ConnectionExt as _;
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ConnectionExt as _, ImageFormat, ImageOrder, MapState, Visualid, Window,
};
use x11rb::rust_connection::RustConnection;

use crate::x11_pixels::{PixelLayout, is_wayland_session, pixel_rect, zpixmap_to_rgba};
use crate::{
    CaptureError, CapturedImage, Capturer, DisplayInfo, Result, ScreenRect, WindowInfo,
    region_on_display,
};

pub(crate) struct X11Capturer {
    conn: RustConnection,
    screen: usize,
    atoms: Atoms,
}

struct Atoms {
    client_list_stacking: Atom,
    client_list: Atom,
    wm_name: Atom,
    utf8_string: Atom,
    wm_pid: Atom,
    wm_window_type: Atom,
    type_normal: Atom,
    type_dialog: Atom,
}

fn os<E: std::fmt::Display>(e: E) -> CaptureError {
    CaptureError::Os(e.to_string())
}

impl X11Capturer {
    pub(crate) fn new() -> Result<Self> {
        let env = |k: &str| std::env::var(k).ok();
        if is_wayland_session(
            env("XDG_SESSION_TYPE").as_deref(),
            env("WAYLAND_DISPLAY").as_deref(),
        ) {
            return Err(CaptureError::Unsupported(
                "Wayland sessions need the xdg-desktop-portal backend (roadmap M4-4); \
                 XWayland can't see Wayland windows",
            ));
        }
        if env("DISPLAY").is_none_or(|d| d.is_empty()) {
            return Err(CaptureError::Unsupported(
                "no X11 display ($DISPLAY is not set)",
            ));
        }
        let (conn, screen) = x11rb::connect(None).map_err(os)?;
        let atoms = Atoms::intern(&conn)?;
        Ok(Self {
            conn,
            screen,
            atoms,
        })
    }

    fn root(&self) -> Window {
        self.conn.setup().roots[self.screen].root
    }

    /// The pixel layout of images read from a drawable of `depth` with `visual`.
    fn layout(&self, depth: u8, visual: Visualid) -> Result<PixelLayout> {
        let setup = self.conn.setup();
        let unsupported = || CaptureError::Os(format!("unsupported X11 pixel depth {depth}"));
        let format = setup
            .pixmap_formats
            .iter()
            .find(|f| f.depth == depth)
            .ok_or_else(unsupported)?;
        let vt = setup.roots[self.screen]
            .allowed_depths
            .iter()
            .flat_map(|d| &d.visuals)
            .find(|v| v.visual_id == visual)
            .ok_or_else(unsupported)?;
        Ok(PixelLayout::new(
            format.bits_per_pixel,
            format.scanline_pad,
            setup.image_byte_order == ImageOrder::MSB_FIRST,
            depth,
            [vt.red_mask, vt.green_mask, vt.blue_mask],
        ))
    }

    /// Read a rectangle of a drawable as RGBA. `visual` is the drawable's visual
    /// (pixmaps report none in the reply).
    fn read(
        &self,
        drawable: u32,
        (x, y, w, h): (i16, i16, u16, u16),
        visual: Visualid,
    ) -> Result<CapturedImage> {
        let reply = self
            .conn
            .get_image(ImageFormat::Z_PIXMAP, drawable, x, y, w, h, !0)
            .map_err(os)?
            .reply()
            .map_err(os)?;
        let visual = if reply.visual != 0 {
            reply.visual
        } else {
            visual
        };
        let layout = self.layout(reply.depth, visual)?;
        let pixels = zpixmap_to_rgba(&reply.data, usize::from(w), usize::from(h), &layout)
            .ok_or_else(|| CaptureError::Os("could not convert the captured image".into()))?;
        Ok(CapturedImage {
            width: u32::from(w),
            height: u32::from(h),
            pixels,
            scale: 1.0,
        })
    }

    fn property32(&self, window: Window, property: Atom, type_: Atom) -> Vec<u32> {
        self.conn
            .get_property(false, window, property, type_, 0, u32::MAX / 4)
            .ok()
            .and_then(|c| c.reply().ok())
            .and_then(|r| r.value32().map(Iterator::collect))
            .unwrap_or_default()
    }

    fn property_bytes(&self, window: Window, property: Atom, type_: Atom) -> Option<Vec<u8>> {
        self.conn
            .get_property(false, window, property, type_, 0, u32::MAX / 4)
            .ok()?
            .reply()
            .ok()
            .filter(|r| r.format == 8 && !r.value.is_empty())
            .map(|r| r.value)
    }

    fn title(&self, window: Window) -> String {
        self.property_bytes(window, self.atoms.wm_name, self.atoms.utf8_string)
            .or_else(|| self.property_bytes(window, AtomEnum::WM_NAME.into(), AtomEnum::ANY.into()))
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default()
    }

    /// The class part of `WM_CLASS` (`instance\0class\0`), e.g. "Firefox".
    fn app_name(&self, window: Window) -> String {
        self.property_bytes(window, AtomEnum::WM_CLASS.into(), AtomEnum::STRING.into())
            .map(|b| {
                let mut parts = b.split(|&c| c == 0).filter(|p| !p.is_empty());
                let first = parts.next().unwrap_or_default();
                String::from_utf8_lossy(parts.next().unwrap_or(first)).into_owned()
            })
            .unwrap_or_default()
    }

    /// A window's bounds in root coordinates (its client area, without frame).
    fn bounds(&self, window: Window) -> Option<ScreenRect> {
        let geo = self.conn.get_geometry(window).ok()?.reply().ok()?;
        let pos = self
            .conn
            .translate_coordinates(window, self.root(), 0, 0)
            .ok()?
            .reply()
            .ok()?;
        Some(ScreenRect::new(
            f64::from(pos.dst_x),
            f64::from(pos.dst_y),
            f64::from(geo.width),
            f64::from(geo.height),
        ))
    }

    /// Client windows the window manager knows, front to back.
    fn clients(&self) -> Vec<Window> {
        let root = self.root();
        let mut windows = self.property32(
            root,
            self.atoms.client_list_stacking,
            AtomEnum::WINDOW.into(),
        );
        if windows.is_empty() {
            windows = self.property32(root, self.atoms.client_list, AtomEnum::WINDOW.into());
        }
        // Stacking order is bottom to top.
        windows.reverse();
        windows
    }

    /// Normal, mapped application windows that aren't ours.
    fn is_listed(&self, window: Window) -> bool {
        let own = std::process::id();
        let Some(attrs) = self
            .conn
            .get_window_attributes(window)
            .ok()
            .and_then(|c| c.reply().ok())
        else {
            return false;
        };
        if attrs.map_state != MapState::VIEWABLE {
            return false;
        }
        if self.property32(window, self.atoms.wm_pid, AtomEnum::CARDINAL.into()) == [own] {
            return false;
        }
        let types = self.property32(window, self.atoms.wm_window_type, AtomEnum::ATOM.into());
        types.is_empty()
            || types
                .iter()
                .any(|&t| t == self.atoms.type_normal || t == self.atoms.type_dialog)
    }
}

impl Atoms {
    fn intern(conn: &RustConnection) -> Result<Self> {
        let atom = |name: &str| -> Result<Atom> {
            Ok(conn
                .intern_atom(false, name.as_bytes())
                .map_err(os)?
                .reply()
                .map_err(os)?
                .atom)
        };
        Ok(Self {
            client_list_stacking: atom("_NET_CLIENT_LIST_STACKING")?,
            client_list: atom("_NET_CLIENT_LIST")?,
            wm_name: atom("_NET_WM_NAME")?,
            utf8_string: atom("UTF8_STRING")?,
            wm_pid: atom("_NET_WM_PID")?,
            wm_window_type: atom("_NET_WM_WINDOW_TYPE")?,
            type_normal: atom("_NET_WM_WINDOW_TYPE_NORMAL")?,
            type_dialog: atom("_NET_WM_WINDOW_TYPE_DIALOG")?,
        })
    }
}

impl Capturer for X11Capturer {
    fn has_permission(&self) -> bool {
        true
    }

    fn request_permission(&self) -> bool {
        true
    }

    fn displays(&self) -> Result<Vec<DisplayInfo>> {
        let root = self.root();
        let monitors = self
            .conn
            .randr_get_monitors(root, true)
            .ok()
            .and_then(|c| c.reply().ok())
            .map(|r| r.monitors)
            .unwrap_or_default();
        if monitors.is_empty() {
            // No RandR 1.5: the whole screen is one display.
            let s = &self.conn.setup().roots[self.screen];
            return Ok(vec![DisplayInfo {
                id: 0,
                bounds: ScreenRect::new(
                    0.0,
                    0.0,
                    f64::from(s.width_in_pixels),
                    f64::from(s.height_in_pixels),
                ),
                scale: 1.0,
            }]);
        }
        // Primary first, so `capture_screen(None)` and ids stay predictable.
        let mut monitors = monitors;
        monitors.sort_by_key(|m| !m.primary);
        Ok(monitors
            .iter()
            .zip(0..)
            .map(|(m, id)| DisplayInfo {
                id,
                bounds: ScreenRect::new(
                    f64::from(m.x),
                    f64::from(m.y),
                    f64::from(m.width),
                    f64::from(m.height),
                ),
                scale: 1.0,
            })
            .collect())
    }

    fn list_windows(&self) -> Result<Vec<WindowInfo>> {
        Ok(self
            .clients()
            .into_iter()
            .filter(|&w| self.is_listed(w))
            .filter_map(|w| {
                let bounds = self.bounds(w)?;
                (bounds.width > 1.0 && bounds.height > 1.0).then(|| WindowInfo {
                    id: w,
                    title: self.title(w),
                    app_name: self.app_name(w),
                    bounds,
                })
            })
            .collect())
    }

    fn capture_screen(&self, display: Option<u32>) -> Result<CapturedImage> {
        let displays = self.displays()?;
        let info = match display {
            Some(id) => displays.iter().find(|d| d.id == id),
            None => displays.first(),
        }
        .ok_or(CaptureError::NotFound)?;
        self.capture_region(info.bounds)
    }

    fn capture_window(&self, id: u32) -> Result<CapturedImage> {
        if !self.clients().contains(&id) {
            return Err(CaptureError::NotFound);
        }
        let attrs = self
            .conn
            .get_window_attributes(id)
            .map_err(os)?
            .reply()
            .map_err(|_| CaptureError::NotFound)?;
        let geo = self
            .conn
            .get_geometry(id)
            .map_err(os)?
            .reply()
            .map_err(|_| CaptureError::NotFound)?;
        let rect = (0, 0, geo.width, geo.height);
        // Under a compositing manager the window's own pixmap holds all of it, even
        // the parts other windows cover.
        if let Some(image) = self.composited(id, rect, attrs.visual) {
            return Ok(image);
        }
        // Otherwise read the window itself: covered parts come back as what's on top.
        self.read(id, rect, attrs.visual)
    }

    fn capture_region(&self, region: ScreenRect) -> Result<CapturedImage> {
        let displays = self.displays()?;
        let (_, clipped) = region_on_display(&region, &displays).ok_or(CaptureError::NotFound)?;
        let rect = pixel_rect(&clipped).ok_or(CaptureError::NotFound)?;
        let root_visual = self.conn.setup().roots[self.screen].root_visual;
        self.read(self.root(), rect, root_visual)
    }
}

impl X11Capturer {
    /// Read a window through `XCompositeNameWindowPixmap`; `None` if Composite is
    /// missing or the window isn't redirected (no compositing manager).
    fn composited(
        &self,
        window: Window,
        rect: (i16, i16, u16, u16),
        visual: Visualid,
    ) -> Option<CapturedImage> {
        let c = &self.conn;
        c.composite_query_version(0, 2).ok()?.reply().ok()?;
        let pixmap = c.generate_id().ok()?;
        c.composite_name_window_pixmap(window, pixmap)
            .ok()?
            .check()
            .ok()?;
        let image = self.read(pixmap, rect, visual).ok();
        let _ = c.free_pixmap(pixmap);
        image
    }
}
