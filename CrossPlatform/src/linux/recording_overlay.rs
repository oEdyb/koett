//! Optional GTK layer-shell feedback. The shim must precede libwayland in the
//! dynamic loader's symbol scope; late dlopen alone is insufficient. See
//! https://github.com/wmww/gtk4-layer-shell/blob/main/linking.md.
use gtk4::{self as gtk, glib::translate::ToGlibPtr, prelude::*};
use libloading::Library;
use std::os::unix::process::CommandExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use std::{cell::RefCell, rc::Rc};

#[path = "waveform.rs"]
mod waveform;

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

struct MeterState {
    waveform: waveform::Waveform,
    started: Option<Instant>,
    audio: Option<crate::audio::AudioLevels>,
    last_frame: Option<i64>,
}

pub struct RecordingOverlay {
    window: gtk::Window,
    label: gtk::Label,
    status: gtk::Label,
    drawing: gtk::DrawingArea,
    warning: gtk::Label,
    recovery: gtk::Box,
    copy: gtk::Button,
    media_copy: gtk::Button,
    state: Rc<RefCell<MeterState>>,
    tick: Rc<RefCell<Option<gtk::TickCallbackId>>>,
    _layer_shell: LayerShell,
}

// GTK ticks stop when unmapped. Install only during animated capture, and
// remove on stop/reduced motion, so there is no hidden waveform polling loop.
fn sync_animation(
    drawing: &gtk::DrawingArea,
    state: &Rc<RefCell<MeterState>>,
    tick: &Rc<RefCell<Option<gtk::TickCallbackId>>>,
) {
    let reduced = !drawing.settings().is_gtk_enable_animations();
    let mut meter = state.borrow_mut();
    meter.waveform.set_reduced_motion(reduced);
    let animate = meter.started.is_some() && !reduced;
    drop(meter);
    if !animate {
        if let Some(id) = tick.borrow_mut().take() {
            id.remove();
        }
        state.borrow_mut().last_frame = None;
    } else if tick.borrow().is_none() {
        let state = state.clone();
        *tick.borrow_mut() = Some(drawing.add_tick_callback(move |drawing, clock| {
            let mut meter = state.borrow_mut();
            let now = clock.frame_time();
            if meter.last_frame.is_none_or(|last| now - last >= 33_000) {
                let rms = meter.audio.as_ref().map_or(0.0, |audio| audio.current().0);
                meter.waveform.push(rms);
                meter.last_frame = Some(now);
                drawing.queue_draw();
            }
            gtk::glib::ControlFlow::Continue
        }));
    }
    drawing.queue_draw();
}

fn capsule(context: &gtk::cairo::Context, x: f64, y: f64, width: f64, height: f64) {
    let radius = width.min(height) / 2.0;
    context.new_sub_path();
    for (cx, cy, angle) in [
        (x + width - radius, y + radius, -std::f64::consts::FRAC_PI_2),
        (x + width - radius, y + height - radius, 0.0),
        (x + radius, y + height - radius, std::f64::consts::FRAC_PI_2),
        (x + radius, y + radius, std::f64::consts::PI),
    ] {
        context.arc(cx, cy, radius, angle, angle + std::f64::consts::FRAC_PI_2);
    }
    context.close_path();
}

fn paint(
    context: &gtk::cairo::Context,
    meter: &MeterState,
    width: f64,
) -> Result<(), gtk::cairo::Error> {
    capsule(context, 1.0, 1.0, width - 2.0, 46.0);
    context.set_source_rgba(0.075, 0.075, 0.08, 0.97);
    context.fill_preserve()?;
    context.set_source_rgba(1.0, 1.0, 1.0, 0.10);
    context.set_line_width(1.0);
    context.stroke()?;
    context.set_source_rgb(1.0, 0.31, 0.26);
    context.arc(
        if meter.started.is_some() { 20.0 } else { 21.0 },
        24.0,
        4.0,
        0.0,
        std::f64::consts::TAU,
    );
    context.fill()?;
    if meter.started.is_some() {
        for (index, level) in meter.waveform.levels.iter().enumerate() {
            let height = 3.0 + level * 24.0;
            capsule(
                context,
                39.0 + index as f64 * 6.0,
                24.0 - height / 2.0,
                3.0,
                height,
            );
            if (6..=10).contains(&index) {
                context.set_source_rgb(1.0, 0.31, 0.26);
            } else {
                context.set_source_rgba(1.0, 1.0, 1.0, 0.82);
            }
            context.fill()?;
        }
        context.set_source_rgba(1.0, 1.0, 1.0, 0.13);
        context.rectangle(153.0, 12.0, 1.0, 24.0);
        context.fill()?;
    }
    Ok(())
}

impl RecordingOverlay {
    pub fn new(
        application: &gtk::Application,
        latest: Rc<RefCell<Option<String>>>,
    ) -> Option<Self> {
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
        let state = Rc::new(RefCell::new(MeterState {
            waveform: waveform::Waveform::default(),
            started: None,
            audio: None,
            last_frame: None,
        }));
        let tick = Rc::new(RefCell::new(None));
        let drawing = gtk::DrawingArea::new();
        drawing.set_content_width(236);
        drawing.set_content_height(48);
        drawing.set_draw_func({
            let state = state.clone();
            move |_, context, width, _| {
                if let Err(error) = paint(context, &state.borrow(), f64::from(width)) {
                    eprintln!("Recording waveform drawing failed: {error}");
                }
            }
        });
        drawing.settings().connect_gtk_enable_animations_notify({
            let drawing = drawing.downgrade();
            let state = Rc::downgrade(&state);
            let tick = Rc::downgrade(&tick);
            move |_| {
                if let (Some(drawing), Some(state), Some(tick)) =
                    (drawing.upgrade(), state.upgrade(), tick.upgrade())
                {
                    sync_animation(&drawing, &state, &tick);
                }
            }
        });
        let content = gtk::Fixed::new();
        content.put(&drawing, 0.0, 0.0);
        let label = gtk::Label::new(Some("0:00"));
        label.add_css_class("koett-pill-timer");
        label.set_tooltip_text(Some("Recording elapsed time"));
        label.set_size_request(84, 48);
        content.put(&label, 152.0, 0.0);
        let status = gtk::Label::new(Some("Transcribing…"));
        status.add_css_class("koett-pill-status");
        status.set_xalign(0.0);
        status.set_size_request(184, 48);
        content.put(&status, 36.0, 0.0);
        let layout = gtk::Box::new(gtk::Orientation::Vertical, 4);
        layout.append(&content);
        let warning = gtk::Label::new(None);
        warning.add_css_class("koett-feedback");
        warning.set_visible(false);
        layout.append(&warning);
        let recovery = gtk::Box::new(gtk::Orientation::Vertical, 4);
        recovery.add_css_class("koett-feedback");
        let copy = gtk::Button::with_label("Copy transcript");
        copy.set_focusable(false);
        copy.connect_clicked(move |button| {
            if let Some(text) = latest.borrow().as_ref() {
                match super::feedback::copy_transcript(&button.clipboard(), text) {
                    Ok(()) => button.set_label("Copied"),
                    Err(error) => {
                        button.set_label("Copy failed · retry");
                        eprintln!("Copy transcript: {error}");
                    }
                }
            }
        });
        let media_copy = gtk::Button::with_label("Copy");
        media_copy.set_focusable(false);
        media_copy.add_css_class("koett-media-copy");
        let css = gtk::CssProvider::new();
        css.load_from_data(".koett-media-copy { min-width: 0; padding: 4px 10px; }");
        gtk::style_context_add_provider_for_display(
            &media_copy.display(),
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        media_copy.connect_clicked({
            let copy = copy.clone();
            move |_| copy.emit_clicked()
        });
        content.put(&media_copy, 186.0, 8.0);
        media_copy.set_visible(false);
        recovery.append(&copy);
        recovery.set_visible(false);
        layout.append(&recovery);
        window.set_child(Some(&layout));
        Some(Self {
            window,
            label,
            status,
            drawing,
            warning,
            recovery,
            copy,
            media_copy,
            state,
            tick,
            _layer_shell: layer_shell,
        })
    }

    pub fn close(&self) {
        if let Some(id) = self.tick.borrow_mut().take() {
            id.remove();
        }
        self.window.destroy();
    }

    pub fn update(
        &self,
        state: &str,
        started: Option<Instant>,
        levels: Option<&crate::audio::AudioLevels>,
        warning: Option<super::feedback::CaptureWarning>,
        recovering: bool,
        media_preview: Option<&str>,
    ) {
        let recording = started.is_some();
        let transcribing = state.starts_with("Transcribing")
            || state.starts_with("Fetching")
            || state.starts_with("Converting");
        let recovering = recovering && !recording && !transcribing;
        if recovering != self.recovery.is_visible() {
            self.copy.set_label("Copy transcript");
        }
        let media = media_preview.is_some() && recovering;
        self.recovery.set_visible(recovering && !media);
        self.media_copy.set_visible(media);
        self.window
            .set_default_size(if media { 250 } else { 236 }, 48);
        self.drawing
            .set_content_width(if media { 250 } else { 236 });
        self.status
            .set_size_request(if media { 144 } else { 184 }, 48);
        self.status.set_ellipsize(gtk::pango::EllipsizeMode::End);
        self.status.set_max_width_chars(if media { 20 } else { 26 });
        self.warning.set_visible(warning.is_some());
        self.warning
            .set_text(warning.map_or("", |warning| warning.text()));
        self.window.set_can_target(recovering);
        if let Some(surface) = self.window.surface() {
            if recovering {
                let region = gtk::cairo::Region::create_rectangle(&gtk::cairo::RectangleInt::new(
                    186, 8, 64, 40,
                ));
                surface.set_input_region(if media { Some(&region) } else { None });
            } else {
                surface.set_input_region(Some(&gtk::cairo::Region::create()));
            }
        }
        {
            let mut meter = self.state.borrow_mut();
            if meter.started != started {
                meter.waveform.reset();
                meter.last_frame = None;
            }
            meter.started = started;
            meter.audio = levels.cloned();
        }
        if let Some(started) = started {
            let seconds = started.elapsed().as_secs();
            let elapsed = format!("{}:{:02}", seconds / 60, seconds % 60);
            self.label.set_text(&elapsed);
        }
        self.label.set_visible(recording);
        self.status.set_text(if media {
            media_preview.unwrap_or("")
        } else if recording {
            "Recording"
        } else if recovering {
            if state == "Media result" {
                "Media transcript"
            } else {
                "Paste unavailable"
            }
        } else {
            state
        });
        self.status.set_opacity(if recording { 0.0 } else { 1.0 });
        self.status
            .set_visible(recording || transcribing || recovering);
        sync_animation(&self.drawing, &self.state, &self.tick);
        // Mapping a keyboard-none layer surface never requests activation.
        self.window
            .set_visible(recording || transcribing || recovering);
        if recovering && let Some(surface) = self.window.surface() {
            let region = gtk::cairo::Region::create_rectangle(&gtk::cairo::RectangleInt::new(
                186, 8, 64, 40,
            ));
            surface.set_input_region(if media { Some(&region) } else { None });
        }
    }
}
