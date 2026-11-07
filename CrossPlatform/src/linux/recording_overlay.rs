//! Optional GTK layer-shell feedback. The shim must precede libwayland in the
//! dynamic loader's symbol scope; late dlopen alone is insufficient. See
//! https://github.com/wmww/gtk4-layer-shell/blob/main/linking.md.
mod position;
use gtk4::{self as gtk, glib::translate::ToGlibPtr, prelude::*};
use libloading::Library;
use std::os::unix::process::CommandExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use std::{cell::RefCell, rc::Rc};

#[path = "timeline.rs"]
mod timeline;

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
pub(super) struct LayerShell(Library);
impl LayerShell {
    pub(super) fn load() -> Result<Self, Box<dyn std::error::Error>> {
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

    fn position(
        &self,
        window: &gtk::Window,
        monitor: &gtk::gdk::Monitor,
        x: i32,
        y: i32,
    ) -> Result<(), libloading::Error> {
        self.choose_monitor(window, Some(monitor))?;
        self.margins(window, x, y)
    }

    /// Moves a layer window within its current output. Changing the monitor
    /// remaps the surface, so a live drag uses this and never `position`.
    fn margins(&self, window: &gtk::Window, x: i32, y: i32) -> Result<(), libloading::Error> {
        type EdgeFn = unsafe extern "C" fn(*mut gtk::ffi::GtkWindow, i32, i32);
        // SAFETY: upstream C signatures; edges left=0 right=1 top=2 bottom=3.
        unsafe {
            let anchor = self.0.get::<EdgeFn>(b"gtk_layer_set_anchor\0")?;
            let margin = self.0.get::<EdgeFn>(b"gtk_layer_set_margin\0")?;
            let pointer = window.to_glib_none().0;
            for edge in 0..4 {
                anchor(pointer, edge, i32::from(edge == 0 || edge == 2));
                margin(pointer, edge, 0);
            }
            margin(pointer, 0, x);
            margin(pointer, 2, y);
        }
        Ok(())
    }

    /// Full-monitor overlay surface. Enums: overlay=3, keyboard none=0 or
    /// exclusive=1, edges left=0 right=1 top=2 bottom=3.
    pub(super) fn cover(
        &self,
        window: &gtk::Window,
        monitor: &gtk::gdk::Monitor,
        exclusive_keyboard: bool,
    ) -> Result<(), libloading::Error> {
        type WindowFn = unsafe extern "C" fn(*mut gtk::ffi::GtkWindow);
        type IntFn = unsafe extern "C" fn(*mut gtk::ffi::GtkWindow, i32);
        type EdgeFn = unsafe extern "C" fn(*mut gtk::ffi::GtkWindow, i32, i32);
        type MonitorFn =
            unsafe extern "C" fn(*mut gtk::ffi::GtkWindow, *mut gtk::gdk::ffi::GdkMonitor);
        type NameFn = unsafe extern "C" fn(*mut gtk::ffi::GtkWindow, *const std::ffi::c_char);
        // SAFETY: exact upstream C signatures, live library, live monitor and an
        // unrealized GTK window on the GTK thread.
        unsafe {
            let init = self.0.get::<WindowFn>(b"gtk_layer_init_for_window\0")?;
            let layer = self.0.get::<IntFn>(b"gtk_layer_set_layer\0")?;
            let keyboard = self.0.get::<IntFn>(b"gtk_layer_set_keyboard_mode\0")?;
            let anchor = self.0.get::<EdgeFn>(b"gtk_layer_set_anchor\0")?;
            let zone = self.0.get::<IntFn>(b"gtk_layer_set_exclusive_zone\0")?;
            let set_monitor = self.0.get::<MonitorFn>(b"gtk_layer_set_monitor\0")?;
            let namespace = self.0.get::<NameFn>(b"gtk_layer_set_namespace\0")?;
            let pointer = window.to_glib_none().0;
            init(pointer);
            namespace(pointer, c"koett-sketch".as_ptr());
            layer(pointer, 3);
            keyboard(pointer, i32::from(exclusive_keyboard));
            set_monitor(pointer, monitor.to_glib_none().0);
            for edge in 0..4 {
                anchor(pointer, edge, 1);
            }
            zone(pointer, -1);
        }
        super::output::track_layer_keyboard(window, exclusive_keyboard);
        Ok(())
    }

    fn choose_monitor(
        &self,
        window: &gtk::Window,
        monitor: Option<&gtk::gdk::Monitor>,
    ) -> Result<(), libloading::Error> {
        type MonitorFn =
            unsafe extern "C" fn(*mut gtk::ffi::GtkWindow, *mut gtk::gdk::ffi::GdkMonitor);
        // SAFETY: upstream C signature, live library and GTK objects on the UI thread.
        unsafe {
            let set_monitor = self.0.get::<MonitorFn>(b"gtk_layer_set_monitor\0")?;
            set_monitor(window.to_glib_none().0, monitor.to_glib_none().0);
        }
        Ok(())
    }

    /// Releases (0) or takes (1) exclusive keyboard focus on a layer window.
    pub(super) fn set_keyboard(
        &self,
        window: &gtk::Window,
        mode: i32,
    ) -> Result<(), libloading::Error> {
        type IntFn = unsafe extern "C" fn(*mut gtk::ffi::GtkWindow, i32);
        // SAFETY: upstream signature; the window was initialized by `cover`.
        unsafe {
            let keyboard = self.0.get::<IntFn>(b"gtk_layer_set_keyboard_mode\0")?;
            keyboard(window.to_glib_none().0, mode);
        }
        super::output::track_layer_keyboard(window, mode == 1);
        Ok(())
    }
}

/// The pencil follows a live stroke, and stays for the whole of a draw lock.
fn shows_pencil(pill: &Pill) -> bool {
    pill.timeline.drawing() || super::sketch::DRAW_LOCKED.load(std::sync::atomic::Ordering::Acquire)
}

enum PillLayer {
    Layer(Rc<LayerShell>),
    Gnome,
    X11,
}

impl PillLayer {
    fn load() -> Result<Self, Box<dyn std::error::Error>> {
        if super::x11_surface::active() {
            return Ok(Self::X11);
        }
        if super::gnome_pill::available() {
            return Ok(Self::Gnome);
        }
        Ok(Self::Layer(Rc::new(LayerShell::load()?)))
    }

    fn configure(&self, window: &gtk::Window) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Self::Layer(layer) => Ok(layer.configure(window)?),
            Self::Gnome => {
                super::gnome_pill::configure(window, None, position::saved_origin(window))
            }
            Self::X11 => Ok(super::x11_surface::pill(window, None)?),
        }
    }

    fn choose_monitor(
        &self,
        window: &gtk::Window,
        monitor: Option<&gtk::gdk::Monitor>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Self::Layer(layer) => Ok(layer.choose_monitor(window, monitor)?),
            Self::Gnome => {
                super::gnome_pill::configure(window, monitor, position::saved_origin(window))
            }
            Self::X11 => Ok(super::x11_surface::pill(window, monitor)?),
        }
    }
}

const GNOME_RETRY: std::time::Duration = std::time::Duration::from_secs(2);
const WIDTH: i32 = 300;
/// Every state is exactly this tall; nothing ever grows.
const ROW: i32 = 48;
/// Left edge of the strip; the glyph sits in the 44 px before it.
const STRIP_X: f64 = 44.0;
const STEP: f64 = 3.0;
/// Right-hand result buttons (Mac: right edge at width − 11, 6 px apart,
/// each as wide as its title plus 16 px).
const COPY_W: i32 = 48;
const COPY_X: f64 = (WIDTH - 11 - COPY_W) as f64;
const PASTE_W: i32 = 52;
const PASTE_X: f64 = COPY_X - 6.0 - PASTE_W as f64;

/// What the pill shows. Derived from engine state each UI tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    Hidden,
    Recording,
    Transcribing,
    Done,
    Recovery,
    Media,
    Message,
}

/// One short pill row for an engine error; the window keeps the full text.
pub(super) fn error_row(status: &str) -> &'static str {
    let error = status
        .strip_prefix("Error: ")
        .unwrap_or(status)
        .to_lowercase();
    if error.contains("media") || error.contains("browser document") {
        "Media failed"
    } else if error.contains("microphone") || error.contains("input device") {
        "Microphone couldn't start"
    } else if error.contains("download") {
        "Model download failed"
    } else if error.contains("paste") {
        "Paste failed"
    } else if error.contains("clipboard") || error.contains("copy") {
        "Copy failed"
    } else if error.contains("save") || error.contains("write") {
        "Save failed"
    } else {
        "Dictation failed"
    }
}

pub(super) fn phase(
    state: &str,
    recording: bool,
    recovering: bool,
    done: bool,
    media: bool,
    message: bool,
) -> Phase {
    let transcribing = state.starts_with("Transcribing")
        || state.starts_with("Fetching")
        || state.starts_with("Converting");
    if recording {
        Phase::Recording
    } else if transcribing {
        Phase::Transcribing
    } else if recovering && media {
        Phase::Media
    } else if recovering {
        Phase::Recovery
    } else if message {
        Phase::Message
    } else if done {
        Phase::Done
    } else {
        Phase::Hidden
    }
}

/// Recording and transcribing show actions on hover, in place of the strip
/// inside the same 48 px row (as on Mac). Finished states have their own
/// buttons on the right.
pub(super) fn shows_actions(phase: Phase, hovered: bool) -> bool {
    matches!(phase, Phase::Recording | Phase::Transcribing) && hovered
}

/// What the user did on a finished pill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Acted {
    /// Copy succeeded: close after 1 s, even while hovered.
    Copied,
    /// Paste (Not pasted): close now; the paste runs from the tray action.
    Pasted,
}

/// One-line result for the finished pill: whitespace collapsed, bounded.
pub(super) fn result_preview(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    match collapsed.char_indices().nth(96) {
        Some((end, _)) => collapsed[..end].to_string(),
        None => collapsed,
    }
}

/// Debug builds only: `KOETT_DEBUG_PILL_TIME_SCALE=N` runs the pill's clock N
/// times faster, so the lab can show a 20-minute strip in 5 real minutes.
/// Recording, decoding and the engine's limit keep real time.
fn time_scale() -> u64 {
    #[cfg(debug_assertions)]
    {
        static SCALE: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
        *SCALE.get_or_init(|| {
            std::env::var("KOETT_DEBUG_PILL_TIME_SCALE")
                .ok()
                .and_then(|value| value.parse().ok())
                .filter(|scale| (1..=64).contains(scale))
                .unwrap_or(1)
        })
    }
    #[cfg(not(debug_assertions))]
    {
        1
    }
}

pub(super) fn clock(milliseconds: u64) -> String {
    let seconds = milliseconds / 1_000;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// "NN%" once the engine reports a real decode plan; nothing otherwise.
pub(super) fn progress_label(done: u32, planned: u32) -> String {
    if planned > 0 {
        let percent = (f64::from(done.min(planned)) * 100.0 / f64::from(planned)).round();
        format!("{percent}%")
    } else {
        String::new()
    }
}

struct Palette {
    background: (f64, f64, f64, f64),
    stroke: (f64, f64, f64, f64),
    bar: (f64, f64, f64, f64),
    dim: (f64, f64, f64, f64),
    accent: (f64, f64, f64),
    band: (f64, f64, f64, f64),
}

const DARK: Palette = Palette {
    background: (0.075, 0.075, 0.08, 0.97),
    stroke: (1.0, 1.0, 1.0, 0.10),
    bar: (1.0, 1.0, 1.0, 0.82),
    dim: (1.0, 1.0, 1.0, 0.28),
    accent: (1.0, 0.31, 0.26),
    band: (1.0, 0.31, 0.26, 0.24),
};

const LIGHT: Palette = Palette {
    background: (1.0, 1.0, 1.0, 0.97),
    stroke: (0.11, 0.098, 0.09, 0.12),
    bar: (0.11, 0.098, 0.09, 0.72),
    dim: (0.11, 0.098, 0.09, 0.22),
    accent: (0.85, 0.267, 0.227),
    band: (0.85, 0.267, 0.227, 0.16),
};

struct Pill {
    phase: Phase,
    timeline: timeline::Timeline,
    started: Option<Instant>,
    duration_ms: u64,
    actions: bool,
    dark: bool,
    progress: (u32, u32),
    spin: f64,
    paused: bool,
    hovered: bool,
    levels: Option<crate::audio::AudioLevels>,
    last_draw: Option<Instant>,
    last_sample: Option<Instant>,
    /// Content being morphed away from, and when the morph began.
    morph_from: Option<Phase>,
    morph_started: Option<Instant>,
}

/// Meter sampling interval while recording (30 Hz, as on Mac).
const SAMPLE: std::time::Duration = std::time::Duration::from_millis(33);

/// Same-size content morph between states, as on the Mac.
const MORPH: std::time::Duration = std::time::Duration::from_millis(240);

impl Pill {
    /// Eased 0..1 while a morph runs; `None` once it has finished.
    fn morph_progress(&self) -> Option<f64> {
        let started = self.morph_started?;
        let t = started.elapsed().as_secs_f64() / MORPH.as_secs_f64();
        (t < 1.0).then(|| 1.0 - (1.0 - t).powi(3))
    }
}

/// Said once, then the pill fades like Done.
pub(super) const COPIED: &str = "Copied to clipboard";

pub struct RecordingOverlay {
    window: gtk::Window,
    drawing: gtk::DrawingArea,
    primary: gtk::Label,
    row: gtk::Box,
    cancel: gtk::Button,
    pause: gtk::Button,
    copy: gtk::Button,
    paste: gtk::Button,
    history: gtk::Button,
    latest: Rc<RefCell<Option<String>>>,
    /// Done after a paste that didn't happen, with the text on the clipboard.
    copied: std::cell::Cell<bool>,
    acted: Rc<std::cell::Cell<Option<Acted>>>,
    body: gtk::Box,
    warning: gtk::Label,
    state: Rc<RefCell<Pill>>,
    tick: Rc<RefCell<Option<gtk::TickCallbackId>>>,
    layer_shell: PillLayer,
    closed: Rc<std::cell::Cell<bool>>,
    gnome_monitor: RefCell<Option<gtk::gdk::Monitor>>,
    gnome_retry: std::cell::Cell<Option<Instant>>,
    layout: RefCell<super::monitor_layout::MonitorLayout>,
}

fn set_source(context: &gtk::cairo::Context, (r, g, b, a): (f64, f64, f64, f64)) {
    context.set_source_rgba(r, g, b, a);
}

fn rounded(context: &gtk::cairo::Context, x: f64, y: f64, width: f64, height: f64, radius: f64) {
    let radius = radius.min(width / 2.0).min(height / 2.0);
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

fn paint(context: &gtk::cairo::Context, pill: &Pill, height: f64) -> Result<(), gtk::cairo::Error> {
    if pill.phase == Phase::Hidden {
        return Ok(());
    }
    let palette = if pill.dark { &DARK } else { &LIGHT };
    let (ar, ag, ab) = palette.accent;
    let drawing = pill.phase == Phase::Recording && shows_pencil(pill);
    let radius = height / 2.0;
    rounded(
        context,
        1.0,
        1.0,
        f64::from(WIDTH) - 2.0,
        height - 2.0,
        radius,
    );
    set_source(context, palette.background);
    context.fill_preserve()?;
    if drawing {
        context.set_source_rgb(ar, ag, ab);
        context.set_line_width(2.0);
    } else {
        set_source(context, palette.stroke);
        context.set_line_width(1.0);
    }
    context.stroke()?;

    // Morphing content slides; keep it inside the pill.
    rounded(
        context,
        1.0,
        1.0,
        f64::from(WIDTH) - 2.0,
        height - 2.0,
        radius,
    );
    context.clip();
    let cy = height / 2.0;
    let morph = pill.morph_progress();
    match (pill.morph_from, morph) {
        // Same-size morph: the old content slides left and fades quickly while
        // the new content slides in from the right (matches the Mac pill).
        (Some(from), Some(m)) => {
            context.push_group();
            context.translate(-8.0 * m, 0.0);
            paint_body(context, pill, from, cy, palette)?;
            context.pop_group_to_source()?;
            context.paint_with_alpha((1.0 - m * 1.6).max(0.0))?;
            context.push_group();
            context.translate(8.0 * (1.0 - m), 0.0);
            paint_body(context, pill, pill.phase, cy, palette)?;
            context.pop_group_to_source()?;
            context.paint_with_alpha(m)?;
            Ok(())
        }
        _ => paint_body(context, pill, pill.phase, cy, palette),
    }
}

fn paint_body(
    context: &gtk::cairo::Context,
    pill: &Pill,
    phase: Phase,
    cy: f64,
    palette: &Palette,
) -> Result<(), gtk::cairo::Error> {
    let (ar, ag, ab) = palette.accent;
    let drawing = phase == Phase::Recording && shows_pencil(pill);
    context.set_source_rgb(ar, ag, ab);
    context.set_line_cap(gtk::cairo::LineCap::Round);
    context.set_line_join(gtk::cairo::LineJoin::Round);
    match phase {
        Phase::Recording if pill.paused => {
            context.set_line_width(2.5);
            for x in [20.5, 26.5] {
                context.move_to(x, cy - 5.0);
                context.line_to(x, cy + 5.0);
            }
            context.stroke()?;
        }
        Phase::Recording if drawing => {
            context.set_line_width(1.8);
            context.move_to(18.0, cy + 6.0);
            context.line_to(19.0, cy + 2.0);
            context.line_to(27.0, cy - 6.0);
            context.line_to(29.5, cy - 3.5);
            context.line_to(21.5, cy + 4.5);
            context.close_path();
            context.stroke()?;
        }
        Phase::Recording => {
            context.arc(23.0, cy, 4.0, 0.0, std::f64::consts::TAU);
            context.fill()?;
        }
        Phase::Transcribing => {
            set_source(context, palette.dim);
            context.set_line_width(2.0);
            context.arc(23.0, cy, 6.0, 0.0, std::f64::consts::TAU);
            context.stroke()?;
            context.set_source_rgb(ar, ag, ab);
            context.arc(23.0, cy, 6.0, pill.spin, pill.spin + 1.6);
            context.stroke()?;
        }
        Phase::Recovery | Phase::Message => {
            context.set_line_width(1.8);
            context.move_to(23.0, cy - 7.0);
            context.line_to(30.0, cy + 6.0);
            context.line_to(16.0, cy + 6.0);
            context.close_path();
            context.move_to(23.0, cy - 2.0);
            context.line_to(23.0, cy + 1.5);
            context.stroke()?;
            context.arc(23.0, cy + 3.8, 1.0, 0.0, std::f64::consts::TAU);
            context.fill()?;
        }
        Phase::Done | Phase::Media => {
            for (index, bar) in [7.0, 13.0, 19.0, 13.0, 7.0].into_iter().enumerate() {
                rounded(
                    context,
                    14.0 + index as f64 * 4.5,
                    cy - bar / 2.0,
                    2.5,
                    bar,
                    1.25,
                );
            }
            context.fill()?;
        }
        Phase::Hidden => {}
    }

    let strip = matches!(phase, Phase::Recording | Phase::Transcribing)
        && !pill.timeline.is_empty()
        && !pill.actions;
    if !strip {
        return Ok(());
    }
    let transcribing = phase == Phase::Transcribing;
    let x_of = |unit: f64| STRIP_X + unit * STEP;
    let bands: Vec<_> = pill
        .timeline
        .sketches()
        .iter()
        .filter_map(|sketch| pill.timeline.band(sketch).map(|band| (*sketch, band)))
        .collect();
    for (sketch, (start, end)) in &bands {
        rounded(
            context,
            x_of(*start) - 1.0,
            cy - 15.0,
            (end - start) * STEP + 1.0,
            30.0,
            3.0,
        );
        set_source(context, palette.band);
        if sketch.live {
            context.fill_preserve()?;
            context.set_source_rgb(ar, ag, ab);
            context.set_line_width(1.5);
            context.stroke()?;
        } else {
            context.fill()?;
        }
    }
    // Rolling window: bars slide left by elapsed time and fade out over the
    // left six columns. Time before the start is dim silence dots.
    const FADE: f64 = 6.0;
    for bar in pill.timeline.bars() {
        let alpha = ((bar.x + 1.0) / FADE).clamp(0.0, 1.0);
        let left = x_of(bar.x);
        match bar.level {
            Some(level) if level >= timeline::SILENT => {
                let (r, g, b, a) = if transcribing {
                    palette.dim
                } else {
                    palette.bar
                };
                context.set_source_rgba(r, g, b, a * alpha);
                let height = 4.0 + level * 22.0;
                rounded(context, left, cy - height / 2.0, 2.0, height, 1.0);
            }
            _ => {
                let (r, g, b, a) = palette.dim;
                context.set_source_rgba(r, g, b, a * alpha);
                rounded(context, left, cy - 1.0, 2.0, 2.0, 1.0);
            }
        }
        context.fill()?;
    }
    context.select_font_face(
        "Sans",
        gtk::cairo::FontSlant::Normal,
        gtk::cairo::FontWeight::Bold,
    );
    context.set_font_size(9.0);
    for (sketch, (start, _)) in &bands {
        let x = (x_of(*start) + 5.0).max(STRIP_X + 6.5);
        let y = cy - 15.0;
        context.set_source_rgb(ar, ag, ab);
        context.arc(x, y, 6.5, 0.0, std::f64::consts::TAU);
        context.fill()?;
        let label = sketch.index.to_string();
        let extents = context.text_extents(&label)?;
        context.set_source_rgb(1.0, 1.0, 1.0);
        context.move_to(
            x - extents.x_bearing() - extents.width() / 2.0,
            y - extents.y_bearing() - extents.height() / 2.0,
        );
        context.show_text(&label)?;
    }
    Ok(())
}

// The spinner is the only free-running motion and only while decoding with
// animations enabled. Strip data arrives from `update`, not from this clock.
fn sync_animation(
    drawing: &gtk::DrawingArea,
    state: &Rc<RefCell<Pill>>,
    tick: &Rc<RefCell<Option<gtk::TickCallbackId>>>,
) {
    let animate = (drawing.settings().is_gtk_enable_animations()
        && matches!(state.borrow().phase, Phase::Recording | Phase::Transcribing))
        || state.borrow().morph_progress().is_some();
    if !animate {
        if let Some(id) = tick.borrow_mut().take() {
            id.remove();
        }
    } else if tick.borrow().is_none() {
        let state = state.clone();
        *tick.borrow_mut() = Some(drawing.add_tick_callback(move |drawing, clock| {
            let seconds = clock.frame_time() as f64 / 1_000_000.0;
            let mut pill = state.borrow_mut();
            pill.spin = (seconds * 5.0) % std::f64::consts::TAU;
            // Recording: sample the meter at 30 Hz, silence included, so the
            // strip slides left at a constant rate (Mac meter timer).
            if pill.phase == Phase::Recording
                && let Some(started) = pill.started
                && pill.last_sample.is_none_or(|last| last.elapsed() >= SAMPLE)
            {
                let elapsed = u64::try_from(started.elapsed().as_millis())
                    .unwrap_or(u64::MAX)
                    .saturating_mul(time_scale());
                let rms = pill
                    .levels
                    .as_ref()
                    .map_or(0.0, |levels| levels.current().0);
                pill.timeline.push(elapsed, rms);
                pill.last_sample = Some(Instant::now());
            }
            drop(pill);
            drawing.queue_draw();
            // `sync_animation` removes this callback once nothing moves.
            gtk::glib::ControlFlow::Continue
        }));
    }
}

fn pill_button(label: &str) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.set_focusable(false);
    button.add_css_class("koett-pill-button");
    button.set_valign(gtk::Align::Center);
    button
}

impl RecordingOverlay {
    pub fn new(
        application: &gtk::Application,
        latest: Rc<RefCell<Option<String>>>,
        actions: std::sync::mpsc::Sender<super::TrayAction>,
    ) -> Option<Self> {
        let layer_shell = match PillLayer::load() {
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
            .default_width(WIDTH)
            .default_height(ROW)
            .build();
        if let Err(error) = layer_shell.configure(&window) {
            eprintln!("Recording overlay unavailable: {error}");
            window.destroy();
            return None;
        }
        let closed = Rc::new(std::cell::Cell::new(false));
        // GNOME turns the extension off during a screen lock, and turning it off
        // closes the pill. Keep the window so it can come back without a restart.
        let gnome = matches!(layer_shell, PillLayer::Gnome);
        window.connect_close_request({
            let closed = closed.clone();
            move |window| {
                if gnome && !closed.get() {
                    window.set_visible(false);
                    return gtk::glib::Propagation::Stop;
                }
                closed.set(true);
                gtk::glib::Propagation::Proceed
            }
        });
        window.add_css_class("koett-pill");
        window.set_focusable(false);
        window.set_can_target(false);
        window.connect_realize(|window| {
            if let Some(surface) = window.surface() {
                surface.set_input_region(Some(&gtk::cairo::Region::create()));
            }
        });
        let state = Rc::new(RefCell::new(Pill {
            phase: Phase::Hidden,
            timeline: timeline::Timeline::default(),
            started: None,
            duration_ms: 0,
            actions: false,
            dark: true,
            progress: (0, 0),
            spin: 0.0,
            paused: false,
            hovered: false,
            levels: None,
            last_draw: None,
            last_sample: None,
            morph_from: None,
            morph_started: None,
        }));
        let tick = Rc::new(RefCell::new(None));
        let drawing = gtk::DrawingArea::new();
        drawing.set_content_width(WIDTH);
        drawing.set_content_height(ROW);
        drawing.set_can_target(false);
        drawing.set_draw_func({
            let state = state.clone();
            move |_, context, _, height| {
                if let Err(error) = paint(context, &state.borrow(), f64::from(height)) {
                    eprintln!("Recording pill drawing failed: {error}");
                }
            }
        });

        let primary = gtk::Label::new(None);
        primary.add_css_class("koett-pill-text");
        primary.set_ellipsize(gtk::pango::EllipsizeMode::End);
        primary.set_can_target(false);
        let main = gtk::Fixed::new();
        main.set_size_request(WIDTH, ROW);
        main.put(&primary, 0.0, 0.0);

        let cancel = pill_button("Cancel");
        cancel.set_tooltip_text(Some("Discard this dictation"));
        let pause = pill_button("Pause");
        // Right-hand result buttons, as on Mac: Copy at the right edge, Paste
        // (Not pasted only) to its left.
        let copy = pill_button("Copy");
        copy.set_size_request(COPY_W, 28);
        main.put(&copy, COPY_X, 10.0);
        let paste = pill_button("Paste");
        paste.set_size_request(PASTE_W, 28);
        main.put(&paste, PASTE_X, 10.0);
        let history = pill_button("History");
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        row.set_size_request(-1, 28);
        for widget in [
            cancel.upcast_ref::<gtk::Widget>(),
            pause.upcast_ref(),
            history.upcast_ref(),
        ] {
            row.append(widget);
        }
        row.set_visible(false);
        // Hover actions replace the strip inside the same 48 px row.
        main.put(&row, 40.0, 10.0);

        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&drawing));
        overlay.add_overlay(&main);

        cancel.connect_clicked({
            let actions = actions.clone();
            move |_| {
                let _ = actions.send(super::TrayAction::Cancel);
            }
        });
        pause.connect_clicked({
            let state = state.clone();
            move |_| {
                if let Some(levels) = &state.borrow().levels {
                    levels.set_paused(!levels.paused());
                }
            }
        });
        history.connect_clicked({
            let actions = actions.clone();
            move |_| {
                let _ = actions.send(super::TrayAction::OpenTranscripts);
            }
        });
        let acted = Rc::new(std::cell::Cell::new(None));
        paste.connect_clicked({
            let actions = actions.clone();
            let acted = acted.clone();
            move |_| {
                acted.set(Some(Acted::Pasted));
                let _ = actions.send(super::TrayAction::Recovery("paste-last".into()));
            }
        });
        copy.connect_clicked({
            let latest = latest.clone();
            let acted = acted.clone();
            move |button| {
                if let Some(text) = latest.borrow().as_ref() {
                    match super::feedback::copy_transcript(&button.clipboard(), text) {
                        Ok(()) => {
                            button.set_label("Copied");
                            acted.set(Some(Acted::Copied));
                        }
                        Err(error) => {
                            button.set_label("Copy failed");
                            eprintln!("Copy transcript: {error}");
                        }
                    }
                }
            }
        });

        let motion = gtk::EventControllerMotion::new();
        motion.connect_enter({
            let state = state.clone();
            move |_, _, _| state.borrow_mut().hovered = true
        });
        motion.connect_motion({
            let state = state.clone();
            move |_, _, _| state.borrow_mut().hovered = true
        });
        motion.connect_leave({
            let state = state.clone();
            move |_| state.borrow_mut().hovered = false
        });
        window.add_controller(motion);

        let layout = gtk::Box::new(gtk::Orientation::Vertical, 4);
        layout.add_css_class("koett-pill-body");
        layout.append(&overlay);
        let warning = gtk::Label::new(None);
        warning.add_css_class("koett-feedback");
        warning.set_visible(false);
        layout.append(&warning);
        // Fading this parent hides the pill during a drag and leaves the live copy of
        // `layout` intact; GTK ignores opacity on the window itself.
        let shell = gtk::Box::new(gtk::Orientation::Vertical, 0);
        shell.append(&layout);
        window.set_child(Some(&shell));
        match &layer_shell {
            PillLayer::Layer(layer) => {
                position::apply(&window, layer);
                position::attach(&window, &shell, &layout, layer.clone());
            }
            PillLayer::Gnome => position::attach_follow(&window, &layout, true),
            PillLayer::X11 => position::attach_follow(&window, &layout, false),
        }
        Some(Self {
            window,
            drawing,
            primary,
            row,
            cancel,
            pause,
            copy,
            paste,
            history,
            latest,
            copied: std::cell::Cell::new(false),
            acted,
            body: layout,
            warning,
            state,
            tick,
            layer_shell,
            closed,
            gnome_monitor: RefCell::new(None),
            gnome_retry: std::cell::Cell::new(None),
            layout: RefCell::new(super::monitor_layout::MonitorLayout::new()),
        })
    }

    pub fn close(&self) {
        crate::sketch::set_recording_start(None);
        if let Some(id) = self.tick.borrow_mut().take() {
            id.remove();
        }
        if !self.closed.replace(true) {
            self.window.destroy();
        }
    }

    /// Keeps Done and recovery up while the pointer rests on the pill.
    /// On GNOME, a pill about to appear must be registered first: the extension
    /// may have been turned off and on since, which forgets it, and an unregistered
    /// window would map as an ordinary one. While the extension is off the pill stays
    /// hidden and asks again every 2 s.
    fn registered_to_show(&self, wanted: bool) -> bool {
        if !wanted || self.window.is_visible() || !matches!(self.layer_shell, PillLayer::Gnome) {
            return true;
        }
        let now = Instant::now();
        if self
            .gnome_retry
            .get()
            .is_some_and(|at| now.saturating_duration_since(at) < GNOME_RETRY)
        {
            return false;
        }
        let monitor = self.gnome_monitor.borrow().clone();
        match self
            .layer_shell
            .choose_monitor(&self.window, monitor.as_ref())
        {
            Ok(()) => {
                self.gnome_retry.set(None);
                true
            }
            Err(_) => {
                self.gnome_retry.set(Some(now));
                false
            }
        }
    }

    pub fn hovered(&self) -> bool {
        !self.closed.get() && self.state.borrow().hovered
    }

    /// Copy or Paste on a finished pill, reported once.
    pub fn take_acted(&self) -> Option<Acted> {
        self.acted.replace(None)
    }

    /// Fades the finished pill out (250 ms); the caller hides it after.
    pub fn fade(&self, out: bool) {
        if out {
            self.body.add_css_class("koett-pill-fade");
        } else {
            self.body.remove_css_class("koett-pill-fade");
        }
    }

    /// The next Done says the text was copied instead of showing it.
    pub fn set_copied(&self, copied: bool) {
        self.copied.set(copied);
    }

    /// Up on screen, including a result still waiting to be read.
    pub fn is_shown(&self) -> bool {
        self.window.is_visible()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &self,
        state: &str,
        started: Option<Instant>,
        levels: Option<&crate::audio::AudioLevels>,
        warning: Option<super::feedback::CaptureWarning>,
        recovering: bool,
        done: bool,
        media_preview: Option<&str>,
        message: Option<&str>,
    ) {
        if self.closed.get() {
            return;
        }
        if self.layout.borrow().changed() {
            let layout = super::monitor_layout::MonitorLayout::new();
            let previous = self
                .window
                .surface()
                .and_then(|surface| surface.display().monitor_at_surface(&surface))
                .filter(|monitor| layout.monitors.contains(monitor));
            self.window.set_visible(false);
            let chosen = previous.or_else(|| layout.monitors.first().cloned());
            if let Err(error) = self
                .layer_shell
                .choose_monitor(&self.window, chosen.as_ref())
            {
                eprintln!("Recording overlay monitor change failed: {error}");
            }
            self.gnome_monitor.replace(chosen);
            *self.layout.borrow_mut() = layout;
        }
        let recording = started.is_some();
        let phase = phase(
            state,
            recording,
            recovering,
            done,
            media_preview.is_some(),
            message.is_some(),
        );
        let (dark, _) = super::theme::appearance(&self.window.settings(), self.window.upcast_ref());
        let reduced = !self.drawing.settings().is_gtk_enable_animations();
        let mut pill = self.state.borrow_mut();
        if pill.phase != phase && matches!(phase, Phase::Done | Phase::Recovery | Phase::Media) {
            self.copy.set_label("Copy");
            self.acted.set(None);
        }
        if started.is_some() && pill.started != started {
            pill.timeline.reset();
            pill.last_draw = None;
            pill.last_sample = None;
            crate::sketch::set_recording_start(started);
        } else if started.is_none() && pill.started.is_some() {
            crate::sketch::set_recording_start(None);
        }
        if let Some(begin) = pill.started.filter(|_| started.is_none()) {
            pill.duration_ms = u64::try_from(begin.elapsed().as_millis()).unwrap_or(u64::MAX);
        }
        pill.started = started;
        if pill.phase != phase {
            // A surface that unmaps may never see its leave event; motion over
            // the pill sets this again.
            pill.hovered = false;
        }
        if pill.phase == Phase::Transcribing
            && matches!(phase, Phase::Done | Phase::Recovery)
            && !reduced
        {
            pill.morph_from = Some(Phase::Transcribing);
            pill.morph_started = Some(Instant::now());
            self.body.add_css_class("koett-pill-morph");
        } else if pill.phase != phase {
            pill.morph_from = None;
            pill.morph_started = None;
            self.body.remove_css_class("koett-pill-morph");
        }
        pill.phase = phase;
        pill.dark = dark;
        pill.levels = levels.cloned();
        pill.paused = recording && levels.is_some_and(crate::audio::AudioLevels::paused);
        for update in crate::sketch::drain() {
            match update {
                crate::sketch::SketchUpdate::Upsert(event) => {
                    pill.timeline.apply(event);
                }
                crate::sketch::SketchUpdate::Removed { index } => pill.timeline.remove(index),
            }
        }
        let elapsed_ms = started.map_or(pill.duration_ms, |started| {
            u64::try_from(started.elapsed().as_millis())
                .unwrap_or(u64::MAX)
                .saturating_mul(time_scale())
        });
        if recording {
            let rms = levels.map_or(0.0, |levels| levels.current().0);
            pill.timeline.push(elapsed_ms, rms);
        }
        pill.progress = if phase == Phase::Transcribing {
            crate::transcription::decode_progress()
        } else {
            (0, 0)
        };
        let actions = shows_actions(phase, pill.hovered);
        let height = ROW;
        pill.actions = actions;
        let strip = !pill.timeline.is_empty()
            && matches!(phase, Phase::Recording | Phase::Transcribing)
            && !actions;
        let redraw = !reduced
            || !recording
            || pill
                .last_draw
                .is_none_or(|last| last.elapsed() >= std::time::Duration::from_secs(1));
        if redraw {
            pill.last_draw = Some(Instant::now());
        }
        let progress = pill.progress;
        let paused = pill.paused;
        drop(pill);

        self.window.remove_css_class(if dark {
            "koett-pill-light"
        } else {
            "koett-pill-dark"
        });
        self.window.add_css_class(if dark {
            "koett-pill-dark"
        } else {
            "koett-pill-light"
        });

        let text = match phase {
            // Counts what's recorded, as the limit does, so a pause holds the clock.
            Phase::Recording => clock(elapsed_ms.saturating_sub(levels.map_or(0, |levels| {
                u64::try_from(levels.paused_time().as_millis())
                    .unwrap_or(u64::MAX)
                    .saturating_mul(time_scale())
            }))),
            Phase::Transcribing if strip || actions => progress_label(progress.0, progress.1),
            Phase::Transcribing => state.to_string(),
            Phase::Recovery => "Not pasted · saved".into(),
            Phase::Media => result_preview(media_preview.unwrap_or("Media transcript")),
            Phase::Done if self.copied.get() => COPIED.into(),
            Phase::Done => result_preview(self.latest.borrow().as_deref().unwrap_or("Done")),
            Phase::Message => message.unwrap_or_default().to_string(),
            Phase::Hidden => String::new(),
        };
        self.primary.set_text(&text);
        if strip || actions {
            self.primary.set_xalign(1.0);
            self.primary.set_size_request(48, ROW);
            if let Some(main) = self.primary.parent().and_downcast::<gtk::Fixed>() {
                main.move_(&self.primary, 238.0, 0.0);
            }
        } else {
            self.primary.set_xalign(0.0);
            // Finished rows start at the strip's edge and stop 10 px before
            // their right-hand buttons (Mac `resultRect`).
            let (x, width) = match phase {
                Phase::Recovery => (44.0, PASTE_X as i32 - 10 - 44),
                Phase::Done if self.copied.get() => (44.0, WIDTH - 44 - 16),
                Phase::Done | Phase::Media => (44.0, COPY_X as i32 - 10 - 44),
                Phase::Message => (44.0, WIDTH - 44 - 16),
                _ => (40.0, 246),
            };
            self.primary.set_size_request(width, ROW);
            if let Some(main) = self.primary.parent().and_downcast::<gtk::Fixed>() {
                main.move_(&self.primary, x, 0.0);
            }
        }
        self.primary.set_max_width_chars(match phase {
            _ if strip || actions => 6,
            Phase::Message => 36,
            _ => 22,
        });
        if !matches!(phase, Phase::Done) {
            self.fade(false);
        }

        self.row.set_visible(actions);
        self.cancel
            .set_visible(matches!(phase, Phase::Recording | Phase::Transcribing));
        self.pause.set_visible(phase == Phase::Recording);
        self.pause
            .set_label(if paused { "Resume" } else { "Pause" });
        // Copied has nothing left to do, so it's one line with no button.
        self.copy.set_visible(
            matches!(phase, Phase::Done | Phase::Recovery | Phase::Media)
                && !(phase == Phase::Done && self.copied.get()),
        );
        self.paste.set_visible(phase == Phase::Recovery);
        self.history
            .set_visible(matches!(phase, Phase::Recording | Phase::Transcribing));

        self.warning
            .set_visible((warning.is_some() || message.is_some()) && recording);
        self.warning.set_text(
            warning
                .map(|warning| warning.text())
                .or(message)
                .unwrap_or_default(),
        );
        self.drawing.set_content_height(height);
        self.window.set_default_size(WIDTH, height);
        if let PillLayer::Layer(layer) = &self.layer_shell {
            position::apply(&self.window, layer);
        }

        // States with buttons take the pointer (hover actions while recording
        // or transcribing); the pill never takes keyboard focus.
        let target = matches!(
            phase,
            Phase::Recording | Phase::Transcribing | Phase::Done | Phase::Recovery | Phase::Media
        );
        self.window.set_can_target(target);
        let visible = phase != Phase::Hidden && self.registered_to_show(phase != Phase::Hidden);
        if visible && !self.window.is_visible() && matches!(self.layer_shell, PillLayer::X11) {
            super::x11_surface::set_pill_origin(position::saved_origin(&self.window));
        }
        // Mapping a keyboard-none layer surface never requests activation.
        self.window.set_visible(visible);
        if let Some(surface) = self.window.surface() {
            if target {
                let region = gtk::cairo::Region::create_rectangle(&gtk::cairo::RectangleInt::new(
                    0, 0, WIDTH, height,
                ));
                surface.set_input_region(Some(&region));
            } else {
                surface.set_input_region(Some(&gtk::cairo::Region::create()));
            }
        }
        sync_animation(&self.drawing, &self.state, &self.tick);
        if redraw {
            self.drawing.queue_draw();
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_message_shows_with_the_window_closed_and_replaces_done() {
        use super::{Phase, phase};
        assert_eq!(
            phase("Error: no mic", false, false, false, false, true),
            Phase::Message
        );
        assert_eq!(
            phase("Ready", false, false, true, false, true),
            Phase::Message
        );
    }

    #[test]
    fn missing_microphone_has_a_short_row() {
        assert_eq!(
            super::error_row("Error: could not start the microphone: no device"),
            "Microphone couldn't start"
        );
        assert_eq!(
            super::error_row("Error: Microphone unavailable: USB. Choose another input"),
            "Microphone couldn't start"
        );
    }

    #[test]
    fn other_errors_have_a_short_row() {
        assert_eq!(
            super::error_row("Error: could not download the Koett speech model: io"),
            "Model download failed"
        );
        assert_eq!(super::error_row("Error: anything"), "Dictation failed");
        for media in [
            "Error: Media tool failed: unsupported, blocked or login-required source",
            "Error: Media downloader returned an invalid local file",
            "Error: invalid or oversized media audio",
            "Focus one browser document, or enter its URL",
        ] {
            assert_eq!(super::error_row(media), "Media failed");
        }
        assert_eq!(super::error_row("clipboard unavailable"), "Copy failed");
        assert_eq!(super::error_row("paste chord failed"), "Paste failed");
        assert_eq!(super::error_row("could not write history"), "Save failed");
    }

    use super::*;

    #[test]
    fn phases_follow_engine_state_with_recording_first() {
        assert_eq!(
            phase("Recording…", true, true, true, false, true),
            Phase::Recording
        );
        assert_eq!(
            phase("Transcribing…", false, true, true, false, true),
            Phase::Transcribing
        );
        assert_eq!(
            phase("Fetching audio…", false, false, false, false, false),
            Phase::Transcribing
        );
        assert_eq!(phase("Ready", false, true, true, true, false), Phase::Media);
        assert_eq!(
            phase("Ready", false, true, true, false, true),
            Phase::Recovery
        );
        assert_eq!(
            phase("Ready", false, false, true, false, false),
            Phase::Done
        );
        assert_eq!(
            phase("Ready", false, false, false, false, false),
            Phase::Hidden
        );
    }

    #[test]
    fn actions_row_needs_hover_only_while_recording() {
        assert!(!shows_actions(Phase::Recording, false));
        assert!(shows_actions(Phase::Recording, true));
        assert!(!shows_actions(Phase::Transcribing, false));
        assert!(shows_actions(Phase::Transcribing, true));
        for phase in [Phase::Done, Phase::Recovery, Phase::Media] {
            assert!(!shows_actions(phase, false));
            assert!(
                !shows_actions(phase, true),
                "finished rows: own buttons only"
            );
        }
        assert!(!shows_actions(Phase::Hidden, true));
    }

    #[test]
    fn labels_are_short_and_never_invent_progress() {
        assert_eq!(clock(0), "0:00");
        assert_eq!(clock(62_999), "1:02");
        assert_eq!(clock(600_000), "10:00");
        assert_eq!(progress_label(0, 0), "", "no plan, no number");
        assert_eq!(progress_label(0, 1), "0%");
        assert_eq!(progress_label(1, 1), "100%");
        assert_eq!(progress_label(3, 5), "60%");
        assert_eq!(progress_label(9, 5), "100%");
        assert_eq!(result_preview("  Hello\n  there  "), "Hello there");
        let long = "å".repeat(200);
        assert_eq!(result_preview(&long).chars().count(), 96);
    }
}
