//! Optional GTK layer-shell feedback. The shim must precede libwayland in the
//! dynamic loader's symbol scope; late dlopen alone is insufficient. See
//! https://github.com/wmww/gtk4-layer-shell/blob/main/linking.md.
use gtk4::{self as gtk, glib::translate::ToGlibPtr, prelude::*};
use libloading::Library;
use std::os::unix::process::CommandExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

const LIBRARY: &str = "libgtk4-layer-shell.so.0";
const PREPARED: &str = "KOETT_LAYER_SHELL_PREPARED";
static PRELOADED: AtomicBool = AtomicBool::new(false);

pub fn prepare() -> Result<(), String> {
    // Called before Koett starts any threads. Restore the caller's loader env so
    // our optional preload does not propagate into portal/clipboard helpers.
    if std::env::var_os(PREPARED).is_some() {
        unsafe {
            match std::env::var_os("KOETT_ORIGINAL_LD_PRELOAD") {
                Some(value) => std::env::set_var("LD_PRELOAD", value),
                None => std::env::remove_var("LD_PRELOAD"),
            }
            std::env::remove_var("KOETT_ORIGINAL_LD_PRELOAD");
            std::env::remove_var(PREPARED);
        }
        PRELOADED.store(true, Ordering::Relaxed);
        return Ok(());
    }
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return Ok(());
    }
    // Probe without adding a DT_NEEDED entry. Missing/incompatible libraries
    // leave history, settings and in-window recording feedback fully usable.
    if let Err(error) = unsafe { Library::new(LIBRARY) } {
        eprintln!("Recording overlay unavailable: {error}");
        return Ok(());
    }
    let mut command =
        std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
    command.args(std::env::args_os().skip(1)).env(PREPARED, "1");
    // GTK may already have removed this from the process environment. Carry
    // its cached value across the loader re-exec for initial-launch activation.
    if let Some(token) = super::control::startup_token() {
        command.env("XDG_ACTIVATION_TOKEN", token);
    }
    let mut preload = std::ffi::OsString::from(LIBRARY);
    command.env_remove("KOETT_ORIGINAL_LD_PRELOAD");
    if let Some(original) = std::env::var_os("LD_PRELOAD") {
        command.env("KOETT_ORIGINAL_LD_PRELOAD", &original);
        preload.push(":");
        preload.push(original);
    }
    Err(command.env("LD_PRELOAD", preload).exec().to_string())
}

// The maintained Rust GTK layer-shell wrapper has a mandatory native link.
// Keep this optional ABI boundary limited to the upstream stable 1.0 C API.
// The handle outlives every function call and the layer window.
struct LayerShell(Library);
impl LayerShell {
    fn load() -> Result<Self, Box<dyn std::error::Error>> {
        if !PRELOADED.load(Ordering::Relaxed) {
            return Err("layer-shell was not preloaded".into());
        }
        let library = unsafe { Library::new(LIBRARY)? };
        // SAFETY: signature from gtk4-layer-shell.h; GTK is initialized here.
        let supported =
            unsafe { library.get::<unsafe extern "C" fn() -> i32>(b"gtk_layer_is_supported\0")?() };
        if supported == 0 {
            return Err("compositor does not support GTK layer-shell (including GNOME/X11)".into());
        }
        Ok(Self(library))
    }

    fn configure(&self, window: &gtk::Window) -> Result<(), libloading::Error> {
        type WindowFn = unsafe extern "C" fn(*mut gtk::ffi::GtkWindow);
        type IntFn = unsafe extern "C" fn(*mut gtk::ffi::GtkWindow, i32);
        type EdgeFn = unsafe extern "C" fn(*mut gtk::ffi::GtkWindow, i32, i32);
        // SAFETY: exact upstream C signatures, live library and unrealized GTK
        // window on the GTK thread. Enums: overlay=3, bottom=3, keyboard none=0.
        unsafe {
            let init = self.0.get::<WindowFn>(b"gtk_layer_init_for_window\0")?;
            let layer = self.0.get::<IntFn>(b"gtk_layer_set_layer\0")?;
            let keyboard = self.0.get::<IntFn>(b"gtk_layer_set_keyboard_mode\0")?;
            let anchor = self.0.get::<EdgeFn>(b"gtk_layer_set_anchor\0")?;
            let margin = self.0.get::<EdgeFn>(b"gtk_layer_set_margin\0")?;
            let zone = self.0.get::<IntFn>(b"gtk_layer_set_exclusive_zone\0")?;
            let pointer = window.to_glib_none().0;
            init(pointer);
            layer(pointer, 3);
            keyboard(pointer, 0);
            anchor(pointer, 3, 1);
            margin(pointer, 3, 24);
            zone(pointer, -1);
        }
        Ok(())
    }
}

pub struct RecordingOverlay {
    window: gtk::Window,
    label: gtk::Label,
    meter: gtk::LevelBar,
    _layer_shell: LayerShell,
}
impl RecordingOverlay {
    pub fn new(application: &gtk::Application) -> Option<Self> {
        let layer_shell = match LayerShell::load() {
            Ok(shell) => shell,
            Err(error) => {
                eprintln!("Recording overlay unavailable; using in-window feedback: {error}");
                return None;
            }
        };
        let window = gtk::Window::builder()
            .application(application)
            .title("Koett recording")
            .decorated(false)
            .resizable(false)
            .default_width(236)
            .default_height(48)
            .build();
        if let Err(error) = layer_shell.configure(&window) {
            eprintln!("Recording overlay unavailable: {error}");
            window.destroy();
            return None;
        }
        window.add_css_class("koett-pill");
        window.set_focusable(false);
        window.set_can_target(false);
        window.connect_realize(|window| {
            if let Some(surface) = window.surface() {
                surface.set_input_region(Some(&gtk::cairo::Region::create()));
            }
        });
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        content.set_halign(gtk::Align::Center);
        let label = gtk::Label::new(Some("Recording · 0:00"));
        let meter = gtk::LevelBar::for_interval(0.0, 1.0);
        meter.set_size_request(52, -1);
        meter.set_valign(gtk::Align::Center);
        meter.set_tooltip_text(Some("Microphone level"));
        content.append(&label);
        content.append(&meter);
        window.set_child(Some(&content));
        Some(Self {
            window,
            label,
            meter,
            _layer_shell: layer_shell,
        })
    }

    pub fn close(&self) {
        self.window.destroy();
    }

    pub fn update(
        &self,
        state: &str,
        started: Option<Instant>,
        levels: Option<&crate::audio::AudioLevels>,
    ) {
        let recording = started.is_some();
        let transcribing = state.starts_with("Transcribing");
        if let Some(started) = started {
            let seconds = started.elapsed().as_secs();
            self.label
                .set_text(&format!("Recording · {}:{:02}", seconds / 60, seconds % 60));
            self.meter.set_value(levels.map_or(0.0, |levels| {
                f64::from((levels.current().0 * 10.0).clamp(0.0, 1.0))
            }));
        } else if transcribing {
            self.label.set_text("Transcribing…");
        }
        self.meter.set_visible(recording);
        // Mapping a keyboard-none layer surface never requests activation.
        self.window.set_visible(recording || transcribing);
    }
}
