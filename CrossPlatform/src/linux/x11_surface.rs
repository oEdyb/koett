//! The recording pill and drawing surfaces on X11, which has no layer-shell. A GTK
//! window is made override-redirect, so no window manager decorates, moves or focuses
//! it. GDK's input region is an X shape, which applies at once.
use gtk4::{self as gtk, gdk, prelude::*};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ChangeWindowAttributesAux, ConfigureWindowAux, ConnectionExt as _, ImageFormat,
    StackMode,
};
use x11rb::rust_connection::RustConnection;

pub fn active() -> bool {
    gdk::Display::default().is_some_and(|display| display.type_().name() == "GdkX11Display")
}

/// Without a compositor a transparent window shows black.
pub fn composited() -> bool {
    gdk::Display::default().is_some_and(|display| display.is_composited())
}

fn connect() -> Result<(RustConnection, usize), String> {
    x11rb::connect(None).map_err(|error| format!("could not connect to X11: {error}"))
}

fn atom(connection: &RustConnection, name: &str) -> Result<u32, String> {
    connection
        .intern_atom(false, name.as_bytes())
        .map_err(|error| error.to_string())?
        .reply()
        .map(|reply| reply.atom)
        .map_err(|error| error.to_string())
}

fn xid(window: &gtk::Window) -> Result<u32, String> {
    use gtk4::glib::translate::ToGlibPtr;
    type XidFn = unsafe extern "C" fn(*mut gdk::ffi::GdkSurface) -> std::ffi::c_ulong;
    gtk::prelude::WidgetExt::realize(window);
    let surface = window.surface().ok_or("the window has no surface")?;
    let library = libloading::os::unix::Library::this();
    // SAFETY: upstream signature of a symbol in the loaded GTK, on an X11 surface.
    unsafe {
        let get = library
            .get::<XidFn>(b"gdk_x11_surface_get_xid\0")
            .map_err(|error| error.to_string())?;
        u32::try_from(get(surface.to_glib_none().0)).map_err(|error| error.to_string())
    }
}

/// GDK's scale on X11, which is one value for the whole display. Rectangles from GTK
/// are logical and the X server works in physical pixels.
static SCALE: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(1);

fn scale() -> i32 {
    SCALE.load(std::sync::atomic::Ordering::Relaxed).max(1)
}

/// Physical pixels of a monitor, which is what X11 positions in.
fn physical(monitor: &gdk::Monitor) -> (i32, i32, i32, i32) {
    let area = monitor.geometry();
    let scale = monitor.scale_factor();
    SCALE.store(scale, std::sync::atomic::Ordering::Relaxed);
    (
        area.x() * scale,
        area.y() * scale,
        area.width() * scale,
        area.height() * scale,
    )
}

/// Takes a realized window out of the window manager's hands. Only then is it sure
/// to stay undecorated and above, and never to be given focus: GTK rewrites the focus
/// hint when it shows a window, and a window manager gives a new window focus anyway.
fn float(window: &gtk::Window) -> Result<u32, String> {
    let xid = xid(window)?;
    let (connection, _) = connect()?;
    connection
        .change_window_attributes(xid, &ChangeWindowAttributesAux::new().override_redirect(1))
        .map_err(|error| error.to_string())?;
    connection.flush().map_err(|error| error.to_string())?;
    Ok(xid)
}

/// Back on top of anything a window manager has raised since.
pub fn raise(window: &gtk::Window) {
    let Ok(xid) = self::xid(window) else {
        return;
    };
    if let Ok((connection, _)) = connect() {
        let _ = connection
            .configure_window(xid, &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE));
        let _ = connection.flush();
    }
}

fn move_window(xid: u32, x: i32, y: i32, size: Option<(i32, i32)>) -> Result<(), String> {
    let (connection, _) = connect()?;
    let mut change = ConfigureWindowAux::new()
        .x(x)
        .y(y)
        .stack_mode(StackMode::ABOVE);
    if let Some((width, height)) = size {
        change = change
            .width(u32::try_from(width).map_err(|error| error.to_string())?)
            .height(u32::try_from(height).map_err(|error| error.to_string())?);
    }
    connection
        .configure_window(xid, &change)
        .map_err(|error| error.to_string())?;
    connection.flush().map_err(|error| error.to_string())
}

/// Covers one monitor. Applied again when mapped, because the window manager places
/// a new window itself.
pub fn cover(window: &gtk::Window, monitor: &gdk::Monitor) -> Result<(), String> {
    if !composited() {
        return Err("drawing on X11 needs a compositor".into());
    }
    let area = monitor.geometry();
    window.set_default_size(area.width(), area.height());
    let xid = float(window)?;
    let (x, y, width, height) = physical(monitor);
    window.connect_map(move |_| {
        if let Err(error) = move_window(xid, x, y, Some((width, height))) {
            eprintln!("Sketch overlay placement failed: {error}");
        }
    });
    Ok(())
}

thread_local! {
    /// Where the user dragged the pill, in logical desktop coordinates.
    static PILL_ORIGIN: std::cell::Cell<Option<(i32, i32)>> = const { std::cell::Cell::new(None) };
}

pub fn set_pill_origin(origin: Option<(i32, i32)>) {
    PILL_ORIGIN.with(|place| place.set(origin));
}

/// Moves the pill with the pointer until the button comes up, then reports where it
/// landed in logical coordinates, or `None` if it stayed put.
pub fn follow_pointer(window: &gtk::Window, finished: impl FnOnce(Option<(i32, i32)>) + 'static) {
    const BUTTON1: u16 = 1 << 8;
    let Ok(xid) = xid(window) else {
        return;
    };
    let Ok((connection, screen)) = connect() else {
        return;
    };
    let root = connection.setup().roots[screen].root;
    let scale = window.scale_factor();
    let pointer =
        move |connection: &RustConnection| connection.query_pointer(root).ok()?.reply().ok();
    let origin = connection
        .translate_coordinates(xid, root, 0, 0)
        .ok()
        .and_then(|cookie| cookie.reply().ok());
    let (Some(start), Some(origin)) = (pointer(&connection), origin) else {
        return;
    };
    let grab = (
        i32::from(start.root_x) - i32::from(origin.dst_x),
        i32::from(start.root_y) - i32::from(origin.dst_y),
    );
    let start = (i32::from(start.root_x), i32::from(start.root_y));
    let mut moved = false;
    let mut last = (i32::from(origin.dst_x), i32::from(origin.dst_y));
    let finished = std::cell::Cell::new(Some(finished));
    gtk::glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
        let Some(now) = pointer(&connection) else {
            return gtk::glib::ControlFlow::Break;
        };
        let at = (i32::from(now.root_x), i32::from(now.root_y));
        moved = moved || (at.0 - start.0).abs().max((at.1 - start.1).abs()) >= 4;
        if moved && u16::from(now.mask) & BUTTON1 != 0 {
            last = (at.0 - grab.0, at.1 - grab.1);
            let _ =
                connection.configure_window(xid, &ConfigureWindowAux::new().x(last.0).y(last.1));
            let _ = connection.flush();
        }
        if u16::from(now.mask) & BUTTON1 == 0 {
            let place = moved.then_some((last.0 / scale, last.1 / scale));
            if place.is_some() {
                set_pill_origin(place);
            }
            if let Some(finished) = finished.take() {
                finished(place);
            }
            return gtk::glib::ControlFlow::Break;
        }
        gtk::glib::ControlFlow::Continue
    });
}

/// x, y, width, height and scale of a monitor, in physical pixels.
type PillMonitor = (i32, i32, i32, i32, i32);

thread_local! {
    /// The monitor the pill sits on: a monitor change only replaces this.
    static PILL_MONITOR: std::cell::Cell<Option<PillMonitor>> =
        const { std::cell::Cell::new(None) };
}

/// Keeps the pill centred at the bottom of a monitor, whatever its height.
pub fn pill(window: &gtk::Window, monitor: Option<&gdk::Monitor>) -> Result<(), String> {
    let display = gtk::prelude::WidgetExt::display(window);
    let monitors = display.monitors();
    let chosen = monitor
        .cloned()
        .or_else(|| monitors.item(0).and_downcast::<gdk::Monitor>())
        .ok_or("no monitor for the recording pill")?;
    let (x, y, width, height) = physical(&chosen);
    let first = PILL_MONITOR
        .with(|place| place.replace(Some((x, y, width, height, chosen.scale_factor()))))
        .is_none();
    if !first {
        return Ok(());
    }
    let xid = float(window)?;
    let place = move |surface_width: i32, surface_height: i32| {
        let Some((x, y, width, height, scale)) = PILL_MONITOR.with(std::cell::Cell::get) else {
            return;
        };
        let (w, h) = (surface_width * scale, surface_height * scale);
        let (left, top) = PILL_ORIGIN.with(std::cell::Cell::get).map_or(
            (x + (width - w) / 2, y + height - h - 24 * scale),
            |(left, top)| (left * scale, top * scale),
        );
        if let Err(error) = move_window(xid, left, top, None) {
            eprintln!("Recording pill placement failed: {error}");
        }
    };
    window.connect_map(move |window| {
        place(
            window.default_width().max(window.width()),
            window.height().max(1),
        );
    });
    if let Some(surface) = window.surface() {
        surface.connect_layout(move |_, surface_width, surface_height| {
            place(surface_width, surface_height);
        });
    }
    Ok(())
}

/// One monitor from the root window as a PNG, ink included.
pub fn capture(rect: (i32, i32, i32, i32)) -> Result<Vec<u8>, String> {
    let (x, y, width, height) = physical_rect(rect, scale());
    let (connection, screen) = connect()?;
    let root = connection.setup().roots[screen].root;
    let reply = connection
        .get_image(
            ImageFormat::Z_PIXMAP,
            root,
            i16::try_from(x).map_err(|error| error.to_string())?,
            i16::try_from(y).map_err(|error| error.to_string())?,
            u16::try_from(width).map_err(|error| error.to_string())?,
            u16::try_from(height).map_err(|error| error.to_string())?,
            u32::MAX,
        )
        .map_err(|error| error.to_string())?
        .reply()
        .map_err(|error| format!("X11 screen capture failed: {error}"))?;
    let (width, height) = (width as usize, height as usize);
    if reply.data.len() != width * height * 4 {
        return Err("X11 screen capture has an unexpected format".into());
    }
    png(&reply.data, width, height)
}

fn physical_rect((x, y, width, height): (i32, i32, i32, i32), scale: i32) -> (i32, i32, i32, i32) {
    (x * scale, y * scale, width * scale, height * scale)
}

fn png(bgrx: &[u8], width: usize, height: usize) -> Result<Vec<u8>, String> {
    let mut rgba = Vec::with_capacity(bgrx.len());
    for pixel in bgrx.as_chunks::<4>().0 {
        rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
    }
    let texture = gdk::MemoryTexture::new(
        i32::try_from(width).map_err(|error| error.to_string())?,
        i32::try_from(height).map_err(|error| error.to_string())?,
        gdk::MemoryFormat::R8g8b8a8,
        &gtk::glib::Bytes::from_owned(rgba),
        width * 4,
    );
    Ok(texture.save_to_png_bytes().to_vec())
}

/// Offers a PNG on the clipboard through GTK, which serves it on the main thread.
pub fn set_clipboard_png(png: Vec<u8>) -> Result<(), String> {
    let (sender, receiver) = std::sync::mpsc::channel();
    gtk::glib::MainContext::default().invoke(move || {
        let result = gdk::Texture::from_bytes(&gtk::glib::Bytes::from_owned(png))
            .map_err(|error| error.to_string())
            .and_then(|texture| {
                let display = gdk::Display::default().ok_or("no display")?;
                display.clipboard().set_texture(&texture);
                Ok(())
            });
        let _ = sender.send(result);
    });
    receiver
        .recv_timeout(std::time::Duration::from_secs(2))
        .map_err(|_| "the clipboard did not answer".to_string())?
}

fn property_text(connection: &RustConnection, window: u32, property: u32) -> Option<Vec<u8>> {
    let reply = connection
        .get_property(false, window, property, AtomEnum::ANY, 0, 1024)
        .ok()?
        .reply()
        .ok()?;
    (reply.format == 8).then_some(reply.value)
}

/// The names `image_paste_shortcuts` can use for a window: its WM_CLASS class, then
/// its instance.
pub fn window_classes(window: u32) -> Vec<String> {
    let Ok((connection, _)) = connect() else {
        return Vec::new();
    };
    let Some(class) = property_text(&connection, window, AtomEnum::WM_CLASS.into()) else {
        return Vec::new();
    };
    class_names(&class)
}

fn class_names(class: &[u8]) -> Vec<String> {
    class
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .rev()
        .map(|part| String::from_utf8_lossy(part).into_owned())
        .collect()
}

/// The topmost other client window at a desktop point, for a pin.
pub fn window_at(point: (f64, f64)) -> Option<super::sketch::GnomeWindow> {
    if !point.0.is_finite() || !point.1.is_finite() {
        return None;
    }
    let scale = scale();
    let (px, py) = (
        (point.0 * f64::from(scale)).round() as i32,
        (point.1 * f64::from(scale)).round() as i32,
    );
    let (connection, screen) = connect().ok()?;
    let root = connection.setup().roots[screen].root;
    let stacking = atom(&connection, "_NET_CLIENT_LIST_STACKING").ok()?;
    let list = connection
        .get_property(false, root, stacking, AtomEnum::ANY, 0, 4096)
        .ok()?
        .reply()
        .ok()?;
    let pid_atom = atom(&connection, "_NET_WM_PID").ok()?;
    let name_atom = atom(&connection, "_NET_WM_NAME").ok()?;
    let own = std::process::id();
    for window in list.value32()?.collect::<Vec<_>>().into_iter().rev() {
        let attributes = connection.get_window_attributes(window).ok()?.reply().ok();
        if attributes
            .is_none_or(|value| value.map_state != x11rb::protocol::xproto::MapState::VIEWABLE)
        {
            continue;
        }
        let pid = connection
            .get_property(false, window, pid_atom, AtomEnum::CARDINAL, 0, 1)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .and_then(|reply| reply.value32().and_then(|mut value| value.next()));
        if pid == Some(own) {
            continue;
        }
        let geometry = connection.get_geometry(window).ok()?.reply().ok()?;
        let origin = connection
            .translate_coordinates(window, root, 0, 0)
            .ok()?
            .reply()
            .ok()?;
        let (x, y) = (i32::from(origin.dst_x), i32::from(origin.dst_y));
        if px < x
            || py < y
            || px >= x + i32::from(geometry.width)
            || py >= y + i32::from(geometry.height)
        {
            continue;
        }
        let title = property_text(&connection, window, name_atom)
            .or_else(|| property_text(&connection, window, AtomEnum::WM_NAME.into()))?;
        let class = property_text(&connection, window, AtomEnum::WM_CLASS.into())?;
        let app = class
            .split(|byte| *byte == 0)
            .rfind(|part| !part.is_empty())?;
        return Some(super::sketch::GnomeWindow {
            title: String::from_utf8_lossy(&title).into_owned(),
            app: String::from_utf8_lossy(app).into_owned(),
            x: f64::from(x) / f64::from(scale),
            y: f64::from(y) / f64::from(scale),
            width: f64::from(geometry.width) / f64::from(scale),
            height: f64::from(geometry.height) / f64::from(scale),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use gtk4::prelude::*;

    #[test]
    fn logical_rectangles_become_physical_at_scale_two() {
        assert_eq!(
            super::physical_rect((640, 0, 640, 400), 2),
            (1280, 0, 1280, 800)
        );
    }

    #[test]
    fn a_window_class_is_tried_before_its_instance() {
        assert_eq!(
            super::class_names(b"kitty-one\0kitty\0"),
            ["kitty", "kitty-one"]
        );
        assert!(super::class_names(b"").is_empty());
    }

    #[test]
    fn a_capture_becomes_a_png_with_red_and_blue_swapped_back() {
        let png = super::png(&[10, 20, 30, 0, 40, 50, 60, 0], 2, 1).unwrap();
        let texture = super::gdk::Texture::from_bytes(&gtk4::glib::Bytes::from_owned(png)).unwrap();
        assert_eq!((texture.width(), texture.height()), (2, 1));
    }
}
