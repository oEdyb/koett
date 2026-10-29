use gtk4::{self as gtk, glib::object::ObjectType};
use libloading::Library;
use std::sync::OnceLock;

pub fn configure(window: &gtk::ApplicationWindow) -> Result<(), String> {
    static LIBRARY: OnceLock<Result<Library, String>> = OnceLock::new();
    let library = LIBRARY
        .get_or_init(|| unsafe {
            Library::new("libgtk4-layer-shell.so.0")
                .map_err(|_| "Floating Ask requires gtk4-layer-shell; using a normal window".into())
        })
        .as_ref()
        .map_err(Clone::clone)?;
    type Window = unsafe extern "C" fn(*mut gtk::ffi::GtkWindow);
    type Value = unsafe extern "C" fn(*mut gtk::ffi::GtkWindow, i32);
    type Edge = unsafe extern "C" fn(*mut gtk::ffi::GtkWindow, i32, i32);
    // Stable gtk4-layer-shell 1.0 ABI; the normal app preloads its Wayland shim.
    unsafe {
        let supported = library
            .get::<unsafe extern "C" fn() -> i32>(b"gtk_layer_is_supported\0")
            .map_err(|e| e.to_string())?;
        if supported() == 0 {
            return Err("Compositor cannot float Ask; using a normal window".into());
        }
        let pointer = window.as_ptr() as *mut gtk::ffi::GtkWindow;
        let init = library
            .get::<Window>(b"gtk_layer_init_for_window\0")
            .map_err(|e| e.to_string())?;
        let layer = library
            .get::<Value>(b"gtk_layer_set_layer\0")
            .map_err(|e| e.to_string())?;
        let keyboard = library
            .get::<Value>(b"gtk_layer_set_keyboard_mode\0")
            .map_err(|e| e.to_string())?;
        let anchor = library
            .get::<Edge>(b"gtk_layer_set_anchor\0")
            .map_err(|e| e.to_string())?;
        let margin = library
            .get::<Edge>(b"gtk_layer_set_margin\0")
            .map_err(|e| e.to_string())?;
        init(pointer);
        layer(pointer, 3);
        keyboard(pointer, 2);
        anchor(pointer, 1, 1);
        anchor(pointer, 2, 1);
        margin(pointer, 1, 22);
        margin(pointer, 2, 80);
    }
    Ok(())
}
