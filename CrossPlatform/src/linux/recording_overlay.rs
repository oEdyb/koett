//! Optional GTK layer-shell feedback. The shim must precede libwayland in the
//! dynamic loader's symbol scope; late dlopen alone is insufficient. See
//! https://github.com/wmww/gtk4-layer-shell/blob/main/linking.md.
mod meeting;
mod paint;
mod position;
mod stage;
use super::meeting_confirm::{Choice, Press, Shown};
use crate::meeting::capture::TrackLevel;
use crate::meeting::{MeetingScope, Tracks};
use gtk4::{self as gtk, glib::translate::ToGlibPtr, prelude::*};
use libloading::Library;
pub(super) use meeting::MeetingPill;
use stage::{Light, Stage};
use std::os::unix::process::CommandExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
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
        // The surface sits on the bottom edge; the pill rests 24 px above it.
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
            margin(pointer, 3, 0);
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
    pill.timeline.drawing()
        || super::sketch::DRAW_LOCKED.load(std::sync::atomic::Ordering::Acquire)
        || (matches!(pill.phase, Phase::Meeting | Phase::MeetingConfirm)
            && super::sketch::MEETING_INK.load(std::sync::atomic::Ordering::Acquire))
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

    fn configure(
        &self,
        window: &gtk::Window,
        stage: &Stage,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Self::Layer(layer) => Ok(layer.configure(window)?),
            Self::Gnome => {
                super::gnome_pill::configure(window, None, position::origin(window, None, stage))
            }
            Self::X11 => Ok(super::x11_surface::pill(window, None)?),
        }
    }

    fn choose_monitor(
        &self,
        window: &gtk::Window,
        monitor: Option<&gtk::gdk::Monitor>,
        stage: &Stage,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Self::Layer(layer) => Ok(layer.choose_monitor(window, monitor)?),
            Self::Gnome => super::gnome_pill::configure(
                window,
                monitor,
                position::origin(window, monitor, stage),
            ),
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
/// The clock ends `CLOCK_INSET` inside the pill in a box that fits 10:00:05;
/// the strip keeps at least `CLOCK_GAP` clear of its text.
const CLOCK_W: i32 = 64;
const CLOCK_INSET: i32 = 14;
const CLOCK_GAP: f64 = 6.0;
/// Most a long clock may widen the pill.
const CLOCK_GROWTH: i32 = 64;
/// Saving stays up at least this long once a meeting closes, then fades.
const SAVING_HOLD: Duration = Duration::from_millis(1_200);
const SAVING_FADE: Duration = Duration::from_millis(250);

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
    Meeting,
    MeetingConfirm,
    MeetingSaving,
}

impl Phase {
    /// Audio is being captured: the recording dot, or the pencil while drawing.
    fn records(self) -> bool {
        matches!(self, Self::Recording | Self::Meeting | Self::MeetingConfirm)
    }
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

/// A live meeting owns the pill. Saving yields to any dictation, and stays
/// until `saving_until` once the meeting has moved on to the tray. Returns the
/// phase and the new `saving_until`.
fn with_meeting(
    meeting: Option<&MeetingPill>,
    dictation: Phase,
    saving_until: Option<Instant>,
    now: Instant,
) -> (Phase, Option<Instant>) {
    match meeting {
        Some(pill) if pill.confirming() => (Phase::MeetingConfirm, None),
        Some(pill) if !pill.saving() => (Phase::Meeting, None),
        _ if dictation != Phase::Hidden => (dictation, None),
        Some(_) => {
            let until = saving_until
                .unwrap_or(now + SAVING_HOLD)
                .max(now + SAVING_FADE);
            (Phase::MeetingSaving, Some(until))
        }
        None => match saving_until {
            Some(until) if now < until => (Phase::MeetingSaving, Some(until)),
            _ => (dictation, None),
        },
    }
}

/// Saving's hold, started the moment a meeting closes into Saving. The pill
/// polls views, so a close and the view after it can land in one poll; the
/// hold must not wait to be seen.
fn saving_hold(
    meeting: Option<&MeetingPill>,
    saving_until: Option<Instant>,
    now: Instant,
) -> Option<Instant> {
    match meeting {
        Some(pill) if pill.saving() => saving_until.or(Some(now + SAVING_HOLD)),
        _ => saving_until,
    }
}

/// What the pill is working on, for its light.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Task {
    Dictation,
    Ask,
    Media,
}

/// Media while a media transcription runs; Ask while an Ask question records.
fn task(state: &str, asking: bool) -> Task {
    if matches!(
        state,
        "Fetching audio…" | "Converting audio…" | "Transcribing locally…"
    ) {
        Task::Media
    } else if asking {
        Task::Ask
    } else {
        Task::Dictation
    }
}

/// The assistant is recording an Ask question.
fn ask_listening() -> bool {
    #[cfg(feature = "ask")]
    return crate::assistant::status()["phase"] == "Listening…";
    #[cfg(not(feature = "ask"))]
    false
}

/// The light around the pill: the task's colour while it captures or
/// transcribes, red while drawing, the meeting colour in a meeting, amber while
/// a meeting has a problem, none on a result.
fn light(phase: Phase, task: Task, drawing: bool, issue: bool) -> Light {
    match phase {
        Phase::Recording if drawing => Light::Drawing,
        Phase::Meeting | Phase::MeetingConfirm if drawing => Light::MeetingDrawing,
        Phase::Meeting | Phase::MeetingConfirm if issue => Light::Issue,
        Phase::Meeting => Light::Meeting,
        Phase::MeetingConfirm => Light::Confirm,
        Phase::Recording | Phase::Transcribing => match task {
            Task::Dictation => Light::Dictation,
            Task::Ask => Light::Ask,
            Task::Media => Light::Media,
        },
        _ => Light::Calm,
    }
}

/// Recording and transcribing show ✕ above the pill on hover.
/// Finished states have their own buttons on the right.
pub(super) fn shows_actions(phase: Phase, hovered: bool) -> bool {
    matches!(phase, Phase::Recording | Phase::Transcribing) && hovered
}

/// What the user did on a finished pill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Acted {
    /// Copy succeeded: the pill becomes Copied to clipboard and fades, even while hovered.
    Copied,
    /// Copy failed: the row says so; its buttons keep their size and place.
    CopyFailed,
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
/// times faster, so the lab can show a 20-minute strip or an hour-long meeting
/// clock in minutes. Recording, decoding and the engine's limit keep real time.
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

/// The pill's width for a clock `text` px wide, where m:ss is `short` px: a
/// longer clock widens the pill by the difference, so the strip, the gap to
/// the clock and the clock's right edge all stay where they are at 0:12.
fn clock_width(text: i32, short: i32) -> i32 {
    WIDTH + (text - short).clamp(0, CLOCK_GROWTH)
}

/// m:ss, and h:mm:ss from the first hour on.
pub(super) fn clock(milliseconds: u64) -> String {
    let seconds = milliseconds / 1_000;
    let (hours, minutes, seconds) = (seconds / 3_600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
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

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// A live meeting's two sides, as the strip samples them.
struct MeetingFeed {
    zero: Instant,
    levels: Tracks<TrackLevel>,
    lost: Tracks<bool>,
}

impl MeetingFeed {
    /// Meeting time and each side's level now; a reconnecting side is `None`.
    fn sample(&self, now: Instant) -> (u64, Tracks<Option<f32>>) {
        let side = |lost: bool, level: &TrackLevel| (!lost).then(|| level.rms(now));
        (
            millis(now.saturating_duration_since(self.zero)),
            Tracks {
                you: side(self.lost.you, &self.levels.you),
                other: side(self.lost.other, &self.levels.other),
            },
        )
    }
}

struct Pill {
    phase: Phase,
    timeline: timeline::Timeline,
    started: Option<Instant>,
    /// Whose clock the strip and the sketch registry follow: a dictation's
    /// start or a live meeting's audio zero.
    owner: Option<Instant>,
    meeting: Option<MeetingFeed>,
    duration_ms: u64,
    dark: bool,
    /// Done is saying Cancelled: the recording dot, as on Mac, not the result bars.
    cancelled: bool,
    progress: (u32, u32),
    spin: f64,
    hovered: bool,
    levels: Option<crate::audio::AudioLevels>,
    last_draw: Option<Instant>,
    last_sample: Option<Instant>,
    strip: paint::Strip,
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

    /// Adds one meter sample to the strip while audio is being captured, silence
    /// included, so the strip slides left at a constant rate (Mac meter timer).
    fn sample(&mut self) {
        let now = Instant::now();
        match self.phase {
            Phase::Recording => {
                let Some(started) = self.started else {
                    return;
                };
                let elapsed =
                    millis(now.saturating_duration_since(started)).saturating_mul(time_scale());
                let rms = self
                    .levels
                    .as_ref()
                    .map_or(0.0, |levels| levels.current().0);
                self.timeline.push(elapsed, rms);
            }
            Phase::Meeting | Phase::MeetingConfirm => {
                let Some(feed) = &self.meeting else {
                    return;
                };
                let (elapsed, sides) = feed.sample(now);
                self.timeline.push_sides(elapsed, sides);
            }
            _ => return,
        }
        self.last_sample = Some(now);
    }
}

/// Said once, then the pill fades like Done.
pub(super) const COPIED: &str = "Copied to clipboard";
pub(super) const CANCELLED: &str = "Cancelled";
pub(super) const TEXT_DRAWING_COPIED: &str = "Text pasted · drawing copied";
pub(super) const TEXT_DRAWING_FAILED: &str = "Text pasted · drawing failed";
pub(super) const DRAWING_TEXT_COPIED: &str = "Drawing pasted · text copied";
pub(super) const DRAWING_TEXT_FAILED: &str = "Drawing pasted · text failed";

pub struct RecordingOverlay {
    window: gtk::Window,
    drawing: gtk::DrawingArea,
    primary: gtk::Label,
    stage: Rc<Stage>,
    stop: gtk::Button,
    confirm: Vec<gtk::Button>,
    meeting: Rc<RefCell<Option<MeetingPill>>>,
    /// The confirmation the row answers, and when its key first showed.
    shown: Rc<std::cell::Cell<Option<Shown>>>,
    cancel: gtk::Button,
    copy: gtk::Button,
    paste: gtk::Button,
    latest: Rc<RefCell<Option<String>>>,
    /// Done says this instead of the text: Copied to clipboard, or Cancelled.
    note: std::cell::Cell<Option<&'static str>>,
    /// Copy was pressed and failed; said in the row until the pill changes.
    copy_failed: std::cell::Cell<bool>,
    acted: Rc<std::cell::Cell<Option<Acted>>>,
    /// The meeting clock as last shown; Saving keeps it still.
    meeting_ms: std::cell::Cell<u64>,
    saving_until: std::cell::Cell<Option<Instant>>,
    state: Rc<RefCell<Pill>>,
    tick: Rc<RefCell<Option<gtk::TickCallbackId>>>,
    layer_shell: PillLayer,
    closed: Rc<std::cell::Cell<bool>>,
    gnome_monitor: RefCell<Option<gtk::gdk::Monitor>>,
    gnome_retry: std::cell::Cell<Option<Instant>>,
    layout: RefCell<super::monitor_layout::MonitorLayout>,
}

// The spinner is the only free-running motion and only while decoding with
// animations enabled. Strip data arrives from `update`, not from this clock.
fn sync_animation(
    drawing: &gtk::DrawingArea,
    state: &Rc<RefCell<Pill>>,
    tick: &Rc<RefCell<Option<gtk::TickCallbackId>>>,
) {
    let moving = matches!(
        state.borrow().phase,
        Phase::Recording
            | Phase::Transcribing
            | Phase::Meeting
            | Phase::MeetingConfirm
            | Phase::MeetingSaving
    );
    let animate = (drawing.settings().is_gtk_enable_animations() && moving)
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
            if pill.last_sample.is_none_or(|last| last.elapsed() >= SAMPLE) {
                pill.sample();
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

fn chip(label: &str) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.set_focusable(false);
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
            .default_width(stage::surface(WIDTH).0)
            .default_height(stage::surface(WIDTH).1)
            .build();
        let state = Rc::new(RefCell::new(Pill {
            phase: Phase::Hidden,
            timeline: timeline::Timeline::default(),
            started: None,
            owner: None,
            meeting: None,
            duration_ms: 0,
            dark: true,
            cancelled: false,
            progress: (0, 0),
            spin: 0.0,
            hovered: false,
            levels: None,
            last_draw: None,
            last_sample: None,
            strip: paint::Strip::default(),
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
            move |_, context, width, height| {
                let size = (f64::from(width), f64::from(height));
                if let Err(error) = paint::paint(context, &state.borrow(), size) {
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
        // Right-hand result buttons, as on Mac: Copy at the right edge, Paste
        // (Not pasted only) to its left.
        let copy = pill_button("Copy");
        copy.set_size_request(COPY_W, 28);
        main.put(&copy, COPY_X, 10.0);
        let paste = pill_button("Paste");
        paste.set_size_request(PASTE_W, 28);
        main.put(&paste, PASTE_X, 10.0);
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&drawing));
        overlay.add_overlay(&main);
        let stage = Rc::new(Stage::new(&overlay));

        // The lane above the pill, left to right: ✕ for a dictation, Stop for
        // a meeting, then the stop confirmation with Continue where Stop was.
        let cancel = chip("✕");
        cancel.set_tooltip_text(Some("Cancel this dictation"));
        super::menu::set_accessible_label(cancel.upcast_ref(), "Cancel");
        cancel.add_css_class("koett-chip-icon");
        stage.add_chip(&cancel);
        let stop = chip("Stop");
        stage.add_chip(&stop);
        let confirm: Vec<gtk::Button> = Choice::ROW
            .iter()
            .map(|choice| {
                let button = chip(meeting::label(*choice));
                if *choice == Choice::Finish {
                    button.add_css_class("koett-chip-primary");
                }
                stage.add_chip(&button);
                button
            })
            .collect();

        if let Err(error) = layer_shell.configure(&window, &stage) {
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

        cancel.connect_clicked({
            let actions = actions.clone();
            move |_| {
                let _ = actions.send(super::TrayAction::Cancel);
            }
        });
        let meeting = Rc::new(RefCell::new(None::<MeetingPill>));
        let shown = Rc::new(std::cell::Cell::new(None::<Shown>));
        let press = Rc::new(std::cell::Cell::new(None::<Press>));
        let send_meeting = {
            let actions = actions.clone();
            move |request| {
                let _ = actions.send(super::TrayAction::Meeting(
                    crate::meeting::MeetingCall::desktop(request),
                ));
            }
        };
        stop.connect_clicked({
            let meeting = meeting.clone();
            let send = send_meeting.clone();
            move |_| {
                if let Some(request) = meeting.borrow().as_ref().and_then(MeetingPill::stop) {
                    send(request);
                }
            }
        });
        for (button, choice) in confirm.iter().zip(Choice::ROW) {
            // A click lands on release; it answers what was on screen at the press.
            let pressed = gtk::GestureClick::new();
            pressed.set_propagation_phase(gtk::PropagationPhase::Capture);
            pressed.connect_pressed({
                let (shown, press) = (shown.clone(), press.clone());
                move |_, _, _, _| {
                    press.set(Some(Press {
                        key: shown.get().map(Shown::key),
                        at: Instant::now(),
                    }));
                }
            });
            button.add_controller(pressed);
            button.connect_clicked({
                let (shown, press) = (shown.clone(), press.clone());
                let send = send_meeting.clone();
                move |_| {
                    let begun = press.take().unwrap_or(Press {
                        key: shown.get().map(Shown::key),
                        at: Instant::now(),
                    });
                    if let Some(request) = choice.request(begun, shown.get()) {
                        send(request);
                    }
                }
            });
        }
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
                // Never relabel the button: a longer label grows past the pill's edge.
                if let Some(text) = latest.borrow().as_ref() {
                    match super::feedback::copy_transcript(&button.clipboard(), text) {
                        Ok(()) => acted.set(Some(Acted::Copied)),
                        Err(error) => {
                            acted.set(Some(Acted::CopyFailed));
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

        stage.root().add_css_class("koett-pill-body");
        // Fading this parent hides the pill during a drag and leaves the live copy of
        // the stage intact; GTK ignores opacity on the window itself.
        let shell = gtk::Box::new(gtk::Orientation::Vertical, 0);
        shell.append(stage.root());
        window.set_child(Some(&shell));
        match &layer_shell {
            PillLayer::Layer(layer) => {
                position::apply(&window, layer, &stage);
                position::attach(&window, &shell, stage.clone(), layer.clone());
            }
            PillLayer::Gnome => position::attach_follow(&window, stage.clone(), true),
            PillLayer::X11 => position::attach_follow(&window, stage.clone(), false),
        }
        Some(Self {
            window,
            drawing,
            primary,
            stage,
            stop,
            confirm,
            meeting,
            shown,
            cancel,
            copy,
            paste,
            latest,
            note: std::cell::Cell::new(None),
            copy_failed: std::cell::Cell::new(false),
            acted,
            meeting_ms: std::cell::Cell::new(0),
            saving_until: std::cell::Cell::new(None),
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
            .choose_monitor(&self.window, monitor.as_ref(), &self.stage)
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

    /// Takes each meeting view as it arrives, before the next can replace it.
    pub fn meeting_view(&self, meeting: Option<&MeetingPill>) {
        self.saving_until.set(saving_hold(
            meeting,
            self.saving_until.get(),
            Instant::now(),
        ));
    }

    /// Pixel width of `text` in the pill label's font.
    fn text_width(&self, text: &str) -> i32 {
        let layout = self.primary.layout().copy();
        layout.set_width(-1);
        layout.set_text(text);
        layout.pixel_size().0
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
        let body = self.stage.root();
        if out {
            body.add_css_class("koett-pill-fade");
        } else {
            body.remove_css_class("koett-pill-fade");
        }
    }

    /// The next Done says the text was copied instead of showing it.
    pub fn set_copied(&self, copied: bool) {
        self.note.set(copied.then_some(COPIED));
    }

    /// The next Done says which half of a drawing result landed, and whether the other is
    /// on the clipboard.
    pub fn set_partial(&self, text_landed: bool, other_copied: bool) {
        self.note.set(Some(match (text_landed, other_copied) {
            (true, true) => TEXT_DRAWING_COPIED,
            (true, false) => TEXT_DRAWING_FAILED,
            (false, true) => DRAWING_TEXT_COPIED,
            (false, false) => DRAWING_TEXT_FAILED,
        }));
    }

    /// The next Done says Cancelled: the ✕ was pressed or a cancel arrived.
    pub fn set_cancelled(&self) {
        self.note.set(Some(CANCELLED));
    }

    /// Copy failed on this pill: the row says so until the pill changes.
    pub fn set_copy_failed(&self) {
        self.copy_failed.set(true);
    }

    /// Off hides the light around the pill in every mode.
    pub fn set_glow(&self, on: bool) {
        self.stage.set_glow(on);
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
        meeting: Option<&MeetingPill>,
        scope: Option<&MeetingScope>,
    ) {
        if self.closed.get() {
            return;
        }
        self.meeting.replace(meeting.cloned());
        if self.layout.borrow().changed() {
            let layout = super::monitor_layout::MonitorLayout::new();
            let previous = self
                .window
                .surface()
                .and_then(|surface| surface.display().monitor_at_surface(&surface))
                .filter(|monitor| layout.monitors.contains(monitor));
            self.window.set_visible(false);
            let chosen = previous.or_else(|| layout.monitors.first().cloned());
            if let Err(error) =
                self.layer_shell
                    .choose_monitor(&self.window, chosen.as_ref(), &self.stage)
            {
                eprintln!("Recording overlay monitor change failed: {error}");
            }
            self.gnome_monitor.replace(chosen);
            *self.layout.borrow_mut() = layout;
        }
        let recording = started.is_some();
        let now = Instant::now();
        let dictation = phase(
            state,
            recording,
            recovering,
            done,
            media_preview.is_some(),
            message.is_some(),
        );
        let (phase, saving_until) = with_meeting(meeting, dictation, self.saving_until.get(), now);
        self.saving_until.set(saving_until);
        let (dark, _) = super::theme::appearance(&self.window.settings(), self.window.upcast_ref());
        let reduced = !self.drawing.settings().is_gtk_enable_animations();
        let mut pill = self.state.borrow_mut();
        pill.cancelled = self.note.get() == Some(CANCELLED);
        if pill.phase != phase && matches!(phase, Phase::Done | Phase::Recovery | Phase::Media) {
            self.copy.set_label("Copy");
            self.acted.set(None);
            self.copy_failed.set(false);
        }
        let owner = started.or(scope.map(MeetingScope::zero));
        if owner.is_some() && pill.owner != owner {
            pill.timeline.reset();
            pill.last_draw = None;
            pill.last_sample = None;
            crate::sketch::set_recording_start(owner);
        } else if owner.is_none() && pill.owner.is_some() {
            crate::sketch::set_recording_start(None);
        }
        pill.owner = owner;
        pill.meeting = scope.map(|scope| MeetingFeed {
            zero: scope.zero(),
            levels: scope.levels().clone(),
            lost: meeting.map(MeetingPill::lost).unwrap_or_default(),
        });
        if let Some(begin) = pill.started.filter(|_| started.is_none()) {
            pill.duration_ms = millis(begin.elapsed());
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
            pill.morph_started = Some(now);
            self.stage.root().add_css_class("koett-pill-morph");
        } else if pill.phase != phase {
            pill.morph_from = None;
            pill.morph_started = None;
            self.stage.root().remove_css_class("koett-pill-morph");
        }
        self.shown.set(Shown::follow(
            self.shown.get(),
            meeting.and_then(MeetingPill::key),
            now,
        ));
        pill.phase = phase;
        pill.dark = dark;
        pill.levels = levels.cloned();
        for update in crate::sketch::drain() {
            match update {
                crate::sketch::SketchUpdate::Upsert(event) => {
                    pill.timeline.apply(event);
                }
                crate::sketch::SketchUpdate::Removed { index } => pill.timeline.remove(index),
            }
        }
        let elapsed_ms = started.map_or(pill.duration_ms, |started| {
            millis(started.elapsed()).saturating_mul(time_scale())
        });
        pill.sample();
        pill.progress = if phase == Phase::Transcribing {
            crate::transcription::decode_progress()
        } else {
            (0, 0)
        };
        let hovered = pill.hovered;
        let strip = !pill.timeline.is_empty()
            && matches!(
                phase,
                Phase::Recording
                    | Phase::Transcribing
                    | Phase::Meeting
                    | Phase::MeetingConfirm
                    | Phase::MeetingSaving
            );
        let live = matches!(
            phase,
            Phase::Recording | Phase::Meeting | Phase::MeetingConfirm
        );
        let redraw = !reduced
            || !live
            || pill
                .last_draw
                .is_none_or(|last| last.elapsed() >= std::time::Duration::from_secs(1));
        if redraw {
            pill.last_draw = Some(now);
        }
        let inked = phase.records() && shows_pencil(&pill);
        let progress = pill.progress;
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

        let meeting_ms = match phase {
            Phase::Meeting | Phase::MeetingConfirm => {
                let shown = meeting
                    .map_or(0, MeetingPill::elapsed_now)
                    .saturating_mul(time_scale());
                self.meeting_ms.set(shown);
                shown
            }
            _ => self.meeting_ms.get(),
        };
        let text = match phase {
            Phase::Recording => clock(elapsed_ms),
            Phase::Meeting | Phase::MeetingConfirm | Phase::MeetingSaving => clock(meeting_ms),
            Phase::Transcribing if strip => progress_label(progress.0, progress.1),
            Phase::Transcribing => state.to_string(),
            Phase::Recovery if self.copy_failed.get() => "Copy failed · saved".into(),
            Phase::Recovery => "Not pasted · saved".into(),
            Phase::Media => result_preview(media_preview.unwrap_or("Media transcript")),
            Phase::Done => match self.note.get() {
                Some(note) => note.into(),
                None if self.copy_failed.get() => "Copy failed".into(),
                None => result_preview(self.latest.borrow().as_deref().unwrap_or("Done")),
            },
            Phase::Message => message.unwrap_or_default().to_string(),
            Phase::Hidden => String::new(),
        };
        self.primary.set_text(&text);
        if phase == Phase::MeetingSaving {
            self.primary.add_css_class("koett-pill-quiet");
        } else {
            self.primary.remove_css_class("koett-pill-quiet");
        }
        let timed = matches!(
            phase,
            Phase::Recording | Phase::Meeting | Phase::MeetingConfirm | Phase::MeetingSaving
        );
        let clocked = strip || timed;
        let label = self.text_width(&text);
        let width = if timed {
            clock_width(label, self.text_width(&clock(0)))
        } else {
            WIDTH
        };
        let clock_right = width - CLOCK_INSET;
        let clock_left = clocked.then(|| f64::from(clock_right - label));
        self.state.borrow_mut().strip =
            paint::Strip::ending_at(clock_left.map_or(f64::MAX, |left| left - CLOCK_GAP));
        self.stage.set_width(width);
        if clocked {
            self.primary.set_xalign(1.0);
            self.primary.set_size_request(CLOCK_W, ROW);
            if let Some(main) = self.primary.parent().and_downcast::<gtk::Fixed>() {
                main.move_(&self.primary, f64::from(clock_right - CLOCK_W), 0.0);
            }
        } else {
            self.primary.set_xalign(0.0);
            // Finished rows start at the strip's edge and stop 10 px before
            // their right-hand buttons (Mac `resultRect`).
            let (x, width) = match phase {
                Phase::Recovery => (44.0, PASTE_X as i32 - 10 - 44),
                Phase::Done if self.note.get().is_some() => (44.0, WIDTH - 44 - 16),
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
            _ if clocked => 8,
            Phase::Message => 36,
            _ => 22,
        });
        match phase {
            Phase::Done => {}
            Phase::MeetingSaving => self.fade(
                meeting.is_none() && saving_until.is_some_and(|until| until <= now + SAVING_FADE),
            ),
            _ => self.fade(false),
        }
        self.primary.set_visible(phase != Phase::Hidden);
        // Copied and Cancelled have nothing left to do: one line, no button.
        self.copy.set_visible(
            matches!(phase, Phase::Done | Phase::Recovery | Phase::Media)
                && !(phase == Phase::Done && self.note.get().is_some()),
        );
        self.paste.set_visible(phase == Phase::Recovery);

        let issue = meeting.and_then(MeetingPill::issue);
        let notice = issue.or_else(|| {
            recording
                .then(|| warning.map(|warning| warning.text()).or(message))
                .flatten()
        });
        let chips: Vec<&gtk::Button> = match phase {
            Phase::MeetingConfirm => self.confirm.iter().collect(),
            Phase::Meeting if hovered => vec![&self.stop],
            _ if shows_actions(phase, hovered) => vec![&self.cancel],
            _ => Vec::new(),
        };
        let armed = self.shown.get().is_some_and(|shown| shown.armed(now));
        for (button, choice) in self.confirm.iter().zip(Choice::ROW) {
            if choice != Choice::Continue && !armed {
                button.add_css_class("koett-chip-waiting");
            } else {
                button.remove_css_class("koett-chip-waiting");
            }
        }
        self.stage.show_lane(notice, &chips);
        self.stage.set_light(
            light(phase, task(state, ask_listening()), inked, issue.is_some()),
            !reduced && phase.records(),
        );
        self.drawing.set_content_height(ROW);
        let (surface_width, surface_height) = self.stage.surface();
        self.window.set_default_size(surface_width, surface_height);
        if let PillLayer::Layer(layer) = &self.layer_shell {
            position::apply(&self.window, layer, &self.stage);
        }

        // States with buttons take the pointer (hover actions while recording
        // or transcribing); the pill never takes keyboard focus.
        let target = matches!(
            phase,
            Phase::Recording
                | Phase::Transcribing
                | Phase::Done
                | Phase::Recovery
                | Phase::Media
                | Phase::Meeting
                | Phase::MeetingConfirm
        );
        self.window.set_can_target(target);
        let visible = phase != Phase::Hidden && self.registered_to_show(phase != Phase::Hidden);
        if visible && !self.window.is_visible() && matches!(self.layer_shell, PillLayer::X11) {
            super::x11_surface::set_pill_origin(position::origin(
                &self.window,
                self.gnome_monitor.borrow().as_ref(),
                &self.stage,
            ));
        }
        // Mapping a keyboard-none layer surface never requests activation.
        self.window.set_visible(visible);
        if let Some(surface) = self.window.surface() {
            if target {
                surface.set_input_region(Some(&self.stage.input_region(&chips)));
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
    fn a_long_clock_widens_the_pill_and_keeps_the_strip_and_gap() {
        // 13 px tabular figures: 0:12, 10:00, 1:16:35 and 10:00:05.
        let short = 27;
        let start = |text: i32| clock_width(text, short) - CLOCK_INSET - text;
        assert_eq!(clock_width(short, short), WIDTH);
        assert_eq!(clock_width(short - 3, short), WIDTH, "never narrower");
        for text in [35, 48, 56] {
            assert_eq!(clock_width(text, short), WIDTH + text - short);
            assert_eq!(start(text), start(short), "same gap after the strip");
            assert!(text <= CLOCK_W, "the clock box fits it");
        }
        let strip = paint::Strip::ending_at(f64::from(start(48)) - CLOCK_GAP);
        assert_eq!(
            strip,
            paint::Strip::default(),
            "the strip is not pushed left"
        );
        assert_eq!(clock_width(short + 500, short), WIDTH + CLOCK_GROWTH);
    }

    #[test]
    fn labels_are_short_and_never_invent_progress() {
        assert_eq!(clock(0), "0:00");
        assert_eq!(clock(62_999), "1:02");
        assert_eq!(clock(600_000), "10:00");
        assert_eq!(clock(3_599_999), "59:59");
        assert_eq!(clock(3_600_000), "1:00:00");
        assert_eq!(clock(4_360_000), "1:12:40", "not 72:40");
        assert_eq!(clock(36_005_000), "10:00:05");
        assert!(
            clock(4_360_000).chars().count() <= 7,
            "fits the clock label"
        );
        assert_eq!(progress_label(0, 0), "", "no plan, no number");
        assert_eq!(progress_label(0, 1), "0%");
        assert_eq!(progress_label(1, 1), "100%");
        assert_eq!(progress_label(3, 5), "60%");
        assert_eq!(progress_label(9, 5), "100%");
        assert_eq!(result_preview("  Hello\n  there  "), "Hello there");
        let long = "å".repeat(200);
        assert_eq!(result_preview(&long).chars().count(), 96);
    }

    fn meeting(view: crate::meeting::MeetingView) -> MeetingPill {
        MeetingPill::from_view(&view).unwrap()
    }

    fn info() -> crate::meeting::SessionInfo {
        crate::meeting::SessionInfo {
            id: crate::meeting::MeetingId::new(),
            started_at_ms: 0,
        }
    }

    #[test]
    fn saving_hands_off_after_a_short_hold_and_yields_to_dictation() {
        use crate::meeting::{CaptureHealth, CloseGoal, MeetingView};
        let now = Instant::now();
        let live = meeting(MeetingView::Recording {
            info: info(),
            health: CaptureHealth::default(),
        });
        let saving = meeting(MeetingView::Closing {
            info: info(),
            goal: CloseGoal::Finish,
        });

        assert_eq!(
            with_meeting(Some(&live), Phase::Hidden, None, now),
            (Phase::Meeting, None)
        );
        let (phase, until) = with_meeting(Some(&saving), Phase::Hidden, None, now);
        assert_eq!(
            (phase, until),
            (Phase::MeetingSaving, Some(now + SAVING_HOLD))
        );
        let later = now + Duration::from_secs(5);
        assert_eq!(
            with_meeting(Some(&saving), Phase::Hidden, until, later),
            (Phase::MeetingSaving, Some(later + SAVING_FADE)),
            "a slow close keeps Saving up"
        );
        let after = now + Duration::from_millis(400);
        assert_eq!(
            with_meeting(None, Phase::Hidden, until, after),
            (Phase::MeetingSaving, until),
            "the hold outlives the close"
        );
        assert_eq!(
            with_meeting(None, Phase::Hidden, until, now + SAVING_HOLD),
            (Phase::Hidden, None)
        );
        assert_eq!(
            with_meeting(Some(&saving), Phase::Recording, until, after),
            (Phase::Recording, None),
            "a new dictation takes the pill at once"
        );
        assert_eq!(
            with_meeting(None, Phase::Done, until, after),
            (Phase::Done, None)
        );
    }

    #[test]
    fn a_close_replaced_within_one_poll_still_shows_saving() {
        use crate::meeting::{CaptureHealth, CloseGoal, MeetingView};
        let now = Instant::now();
        let live = meeting(MeetingView::Recording {
            info: info(),
            health: CaptureHealth::default(),
        });
        let closing = meeting(MeetingView::Closing {
            info: info(),
            goal: CloseGoal::Finish,
        });
        assert_eq!(saving_hold(Some(&live), None, now), None);
        let held = saving_hold(Some(&closing), None, now);
        assert_eq!(held, Some(now + SAVING_HOLD));
        let poll = now + Duration::from_millis(80);
        assert_eq!(
            with_meeting(None, Phase::Hidden, saving_hold(None, held, poll), poll),
            (Phase::MeetingSaving, held),
            "the view after Closing arrived in the same poll"
        );
        assert_eq!(
            saving_hold(Some(&closing), held, poll),
            held,
            "a repeated Closing keeps the hold it started"
        );
    }

    #[test]
    fn each_mode_has_its_own_light() {
        let dictation = task("Recording", false);
        assert_eq!(dictation, Task::Dictation);
        assert_eq!(task("Recording", true), Task::Ask);
        assert_eq!(task("Transcribing locally…", true), Task::Media);
        for state in [
            "Fetching audio…",
            "Converting audio…",
            "Transcribing locally…",
        ] {
            assert_eq!(task(state, false), Task::Media, "{state}");
        }
        assert_eq!(task("Transcribing…", false), Task::Dictation);
        let lit = |phase, drawing, issue| light(phase, dictation, drawing, issue);
        assert_eq!(lit(Phase::Recording, false, false), Light::Dictation);
        assert_eq!(lit(Phase::Recording, true, false), Light::Drawing);
        assert_eq!(lit(Phase::Transcribing, false, false), Light::Dictation);
        assert_eq!(lit(Phase::Meeting, false, false), Light::Meeting);
        assert_eq!(lit(Phase::Meeting, true, true), Light::MeetingDrawing);
        assert_eq!(lit(Phase::Meeting, false, true), Light::Issue);
        assert_eq!(lit(Phase::MeetingConfirm, false, false), Light::Confirm);
        assert_eq!(
            lit(Phase::MeetingConfirm, true, false),
            Light::MeetingDrawing
        );
        assert_eq!(lit(Phase::MeetingSaving, false, true), Light::Calm);
        assert_eq!(lit(Phase::Done, false, false), Light::Calm);
        assert_eq!(lit(Phase::Media, false, false), Light::Calm);
        assert_eq!(light(Phase::Recording, Task::Ask, false, false), Light::Ask);
        assert_eq!(
            light(Phase::Recording, Task::Ask, true, false),
            Light::Drawing
        );
        assert_eq!(
            light(Phase::Transcribing, Task::Media, false, false),
            Light::Media
        );
    }
}
