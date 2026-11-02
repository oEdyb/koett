//! Draw while dictating. While a dictation records, a transparent layer-shell
//! overlay covers every monitor. Holding the draw key (Shift by default) and
//! moving the pointer leaves red ink; otherwise clicks pass through. Strokes on
//! one monitor close into a sketch shortly after the key is released; that
//! monitor is then captured with the ink still visible, and the ink fades.
//! Delivery turns the captures into one numbered contact sheet.
//!
//! The overlay holds exclusive keyboard focus during recording so it can see
//! the draw key (Wayland gives no global key state). It drops focus and input
//! when recording stops, before any paste.
use crate::settings::DrawKey;
use gtk4::{self as gtk, gdk, glib, prelude::*};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::{cell::RefCell, rc::Rc};

mod capture;
mod frames;
pub mod keys;
pub mod sheet;

const INK: (f64, f64, f64) = (1.0, 59.0 / 255.0, 48.0 / 255.0);
const INK_WIDTH: f64 = 5.0;
const FADE_AFTER: Duration = Duration::from_millis(1500);
const FADE_FOR: Duration = Duration::from_millis(400);
/// Matches the pill timeline's cap (`timeline::MAX_SKETCHES`, 256 per dictation).
const MAX_SKETCHES: usize = 256;

pub use frames::{Keyframe, begin_recording, discard_recording, take as take_keyframes};
#[cfg(test)]
pub(crate) use frames::{TEST_LOCK as FRAMES_TEST_LOCK, push as push_frame};

/// Longest wait for presentation feedback before capturing anyway (for a
/// compositor without wp_presentation, or a surface it never presents).
const PRESENT_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, PartialEq)]
enum Readiness {
    Wait,
    Capture,
    Redraw,
}

/// Whether the frame that painted only the queued sketch's ink is on screen.
/// `timing` is GDK's (complete, presentation time) for that frame; GDK fills it
/// from wp_presentation feedback. A complete frame without a presentation time
/// was discarded (never shown), so it must be painted again.
fn readiness(timing: Option<(bool, i64)>, waited: Duration) -> Readiness {
    match timing {
        Some((true, presented)) if presented > 0 => Readiness::Capture,
        _ if waited >= PRESENT_TIMEOUT => Readiness::Capture,
        Some((true, _)) => Readiness::Redraw,
        _ => Readiness::Wait,
    }
}

/// Pill timeline position: wall time since the recording started, matching
/// `koett_engine::sketch::elapsed_ms`, so it includes paused spans.
fn pill_ms(started: Instant, at: Instant) -> u64 {
    u64::try_from(at.saturating_duration_since(started).as_millis()).unwrap_or(u64::MAX)
}

/// Maps wall time to the captured-audio clock: paused audio is discarded at
/// the capture callback, so paused spans must not advance sketch times.
#[derive(Default)]
struct AudioClock {
    paused_total: Duration,
    paused_since: Option<Instant>,
}

impl AudioClock {
    fn observe(&mut self, paused: bool, now: Instant) {
        match (paused, self.paused_since) {
            (true, None) => self.paused_since = Some(now),
            (false, Some(since)) => {
                self.paused_total += now.saturating_duration_since(since);
                self.paused_since = None;
            }
            _ => {}
        }
    }

    fn at(&self, started: Instant, at: Instant) -> Duration {
        let open = self
            .paused_since
            .map_or(Duration::ZERO, |since| at.saturating_duration_since(since));
        at.saturating_duration_since(started)
            .saturating_sub(self.paused_total + open)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Drawing,
    /// Other ink on its monitor is hidden; capture starts after it settles.
    Queued(Instant),
    Capturing,
    Captured,
}

impl Phase {
    fn busy(self) -> bool {
        matches!(self, Self::Queued(_) | Self::Capturing)
    }
}

struct Group {
    id: u64,
    surface: usize,
    strokes: Vec<Vec<(f64, f64)>>,
    start: Instant,
    end: Instant,
    /// Start on the captured-audio clock, used to link speech.
    audio_start: Duration,
    /// Frame that painted this queued sketch with other ink hidden.
    painted: std::cell::Cell<Option<i64>>,
    lifted: Option<Instant>,
    /// Set when the draw key is released: one hold is one sketch.
    closed: bool,
    phase: Phase,
}

/// The sketch that movement on `surface` continues: only one still being
/// drawn there in the current hold. A released hold never continues, so the
/// next hold always starts a separate sketch.
fn continuing(groups: &[Group], surface: usize) -> Option<usize> {
    groups.iter().position(|group| {
        group.surface == surface && group.phase == Phase::Drawing && !group.closed
    })
}

/// While a sketch on a monitor is being captured, only its ink is shown there,
/// so the capture cannot include another sketch's strokes.
fn visible(groups: &[Group], surface: usize, group: &Group) -> bool {
    let busy = groups
        .iter()
        .find(|other| other.surface == surface && other.phase.busy());
    group.surface == surface && busy.is_none_or(|busy| busy.id == group.id)
}

impl Group {
    fn alpha(&self, now: Instant) -> f64 {
        match (self.phase, self.lifted) {
            (Phase::Captured, Some(lifted)) => {
                let fading = now.saturating_duration_since(lifted + FADE_AFTER);
                1.0 - (fading.as_secs_f64() / FADE_FOR.as_secs_f64()).min(1.0)
            }
            _ => 1.0,
        }
    }
}

struct Surface {
    window: gtk::Window,
    area: gtk::DrawingArea,
    target: capture::Target,
}

/// Finished captures by sketch id, filled by capture threads.
type CaptureResults = Arc<Mutex<Vec<(u64, Result<Vec<u8>, String>)>>>;

struct Session {
    recording: u64,
    started: Instant,
    clock: AudioClock,
    key: DrawKey,
    surfaces: Vec<Surface>,
    groups: Vec<Group>,
    next_id: u64,
    held: bool,
    stroking: Option<u64>,
    finishing: bool,
    results: CaptureResults,
    /// Device mode (no keyboard focus); `None` means the overlay holds focus.
    keyboards: Option<keys::Keyboards>,
}

impl Session {
    fn set_input(&self, accept: bool) {
        for surface in &self.surfaces {
            if let Some(native) = surface.window.surface() {
                if accept {
                    let (_, _, width, height) = surface.target.monitor;
                    let all = gtk::cairo::RectangleInt::new(0, 0, width, height);
                    native.set_input_region(Some(&gtk::cairo::Region::create_rectangle(&all)));
                } else {
                    native.set_input_region(Some(&gtk::cairo::Region::create()));
                }
            }
            surface
                .window
                .set_cursor_from_name(accept.then_some("crosshair"));
            // GDK applies a new input region with the next surface commit.
            surface.area.queue_draw();
        }
    }

    fn redraw(&self) {
        for surface in &self.surfaces {
            surface.area.queue_draw();
        }
    }

    fn lift(&mut self) {
        if let Some(id) = self.stroking.take()
            && let Some(group) = self.groups.iter_mut().find(|group| group.id == id)
        {
            group.lifted = Some(Instant::now());
            self.publish();
        }
    }

    /// Ends the current hold: its sketch closes and is captured; the next hold
    /// starts a new sketch, on this monitor or another.
    fn release(&mut self) {
        self.held = false;
        self.lift();
        for group in &mut self.groups {
            group.closed = true;
        }
        self.set_input(false);
    }

    fn press(&mut self) {
        self.held = true;
        self.set_input(true);
    }

    /// Device mode: follows the draw key from keyboard modifier state.
    fn poll_keys(&mut self) {
        let Some(keyboards) = &self.keyboards else {
            return;
        };
        let draws = !self.finishing && keyboards.modifiers().draws(self.key);
        match (draws, self.held) {
            (true, false) => self.press(),
            (false, true) => self.release(),
            _ => {}
        }
    }

    fn point(&mut self, surface: usize, x: f64, y: f64) {
        let now = Instant::now();
        let current = self
            .stroking
            .and_then(|id| self.groups.iter().position(|group| group.id == id))
            .filter(|index| self.groups[*index].surface == surface);
        let index = match current {
            Some(index) => index,
            None => {
                self.lift();
                let open = continuing(&self.groups, surface);
                if open.is_none() && self.groups.len() >= MAX_SKETCHES {
                    return;
                }
                let index = open.unwrap_or_else(|| {
                    self.next_id += 1;
                    self.groups.push(Group {
                        id: self.next_id,
                        surface,
                        strokes: Vec::new(),
                        start: now,
                        end: now,
                        audio_start: self.clock.at(self.started, now),
                        painted: std::cell::Cell::new(None),
                        lifted: None,
                        closed: false,
                        phase: Phase::Drawing,
                    });
                    self.groups.len() - 1
                });
                self.groups[index].strokes.push(Vec::new());
                self.stroking = Some(self.groups[index].id);
                index
            }
        };
        let group = &mut self.groups[index];
        group.lifted = None;
        group.end = now;
        if let Some(stroke) = group.strokes.last_mut() {
            stroke.push((x, y));
        }
        self.surfaces[surface].area.queue_draw();
        self.publish();
    }

    fn undo(&mut self) {
        self.lift();
        if let Some(index) = self
            .groups
            .iter()
            .rposition(|group| group.phase == Phase::Drawing)
        {
            self.groups[index].strokes.pop();
            if self.groups[index].strokes.is_empty() {
                self.drop_group(index);
            } else {
                self.publish();
            }
            self.redraw();
        }
    }

    /// Removes a sketch; later numbers shift down, so the pill gets the
    /// renumbered list and loses the now-unused highest number.
    fn drop_group(&mut self, index: usize) {
        let before = self.groups.len();
        self.groups.remove(index);
        self.publish();
        crate::sketch::remove_for(self.started, u32::try_from(before).unwrap_or(u32::MAX));
    }

    /// Republishes every sketch to the recording pill's timeline. Numbers
    /// follow start order, matching the pasted contact sheet and bullets.
    /// Times are pill (wall-clock) times, which include pauses; speech
    /// linking uses `audio_start` instead.
    fn publish(&self) {
        let mut groups: Vec<&Group> = self.groups.iter().collect();
        groups.sort_by_key(|group| group.id);
        for (index, group) in groups.into_iter().enumerate() {
            crate::sketch::publish_for(
                self.started,
                crate::sketch::SketchEvent {
                    index: u32::try_from(index + 1).unwrap_or(u32::MAX),
                    start_ms: pill_ms(self.started, group.start),
                    end_ms: pill_ms(self.started, group.end),
                    live: self.stroking == Some(group.id),
                },
            );
        }
    }

    /// One timer step. Returns false once a finishing session is complete.
    fn tick(&mut self) -> bool {
        self.poll_keys();
        let now = Instant::now();
        let results = std::mem::take(
            &mut *self
                .results
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        for (id, result) in results {
            let Some(index) = self.groups.iter().position(|group| group.id == id) else {
                continue;
            };
            match result {
                Ok(png) => {
                    self.groups[index].phase = Phase::Captured;
                    let frame = Keyframe {
                        order: id,
                        start: self.groups[index].audio_start.as_secs_f32(),
                        png,
                    };
                    frames::push(self.recording, frame);
                }
                Err(error) => {
                    eprintln!("Sketch capture failed: {error}");
                    self.drop_group(index);
                }
            }
            self.redraw();
        }
        for index in 0..self.groups.len() {
            let group = &self.groups[index];
            let surface_busy = self
                .groups
                .iter()
                .any(|other| other.surface == group.surface && other.phase.busy());
            if group.phase == Phase::Drawing
                && self.stroking != Some(group.id)
                && (group.closed || self.finishing)
                && !surface_busy
            {
                let surface = group.surface;
                self.groups[index].phase = Phase::Queued(now);
                self.groups[index].painted.set(None);
                self.surfaces[surface].area.queue_draw();
                continue;
            }
            let Phase::Queued(since) = group.phase else {
                continue;
            };
            let area = &self.surfaces[group.surface].area;
            let timing = group.painted.get().map(|frame| {
                area.frame_clock()
                    .and_then(|clock| clock.timings(frame))
                    .map_or((true, 0), |timing| {
                        (timing.is_complete(), timing.presentation_time())
                    })
            });
            match readiness(timing, now.saturating_duration_since(since)) {
                Readiness::Wait => continue,
                Readiness::Redraw => {
                    group.painted.set(None);
                    area.queue_draw();
                    continue;
                }
                Readiness::Capture => {
                    if !timing.is_some_and(|(_, presented)| presented > 0) {
                        eprintln!("Sketch capture without presentation feedback");
                    }
                }
            }
            {
                let (id, target) = (group.id, self.surfaces[group.surface].target.clone());
                let strokes = group.strokes.clone();
                self.groups[index].phase = Phase::Capturing;
                let results = self.results.clone();
                std::thread::spawn(move || {
                    let (_, _, width, height) = target.monitor;
                    let result = capture::capture(&target)
                        .and_then(|png| sheet::crop_to_ink(&png, (width, height), &strokes));
                    results
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .push((id, result));
                });
            }
        }
        let busy = self
            .groups
            .iter()
            .any(|group| group.phase != Phase::Captured);
        if self.finishing && !busy {
            return false;
        }
        if self.groups.iter().any(|group| {
            group.phase == Phase::Captured && group.alpha(now) > 0.0 && group.alpha(now) < 1.0
        }) {
            self.redraw();
        }
        true
    }
}

fn paint(session: &Session, surface: usize, context: &gtk::cairo::Context) {
    let now = Instant::now();
    context.set_line_width(INK_WIDTH);
    context.set_line_cap(gtk::cairo::LineCap::Round);
    context.set_line_join(gtk::cairo::LineJoin::Round);
    for group in session
        .groups
        .iter()
        .filter(|group| visible(&session.groups, surface, group))
    {
        let alpha = group.alpha(now);
        if alpha <= 0.0 {
            continue;
        }
        context.set_source_rgba(INK.0, INK.1, INK.2, alpha);
        for stroke in &group.strokes {
            let Some((first, rest)) = stroke.split_first() else {
                continue;
            };
            context.move_to(first.0, first.1);
            if rest.is_empty() {
                context.line_to(first.0 + 0.1, first.1);
            }
            for (x, y) in rest {
                context.line_to(*x, *y);
            }
            let _ = context.stroke();
        }
    }
}

fn key_matches(key: DrawKey, keyval: gdk::Key) -> bool {
    match key {
        DrawKey::Shift => matches!(keyval, gdk::Key::Shift_L | gdk::Key::Shift_R),
        DrawKey::Control => matches!(keyval, gdk::Key::Control_L | gdk::Key::Control_R),
        DrawKey::Alt => matches!(keyval, gdk::Key::Alt_L | gdk::Key::Alt_R | gdk::Key::Meta_L),
    }
}

fn key_mask(key: DrawKey) -> gdk::ModifierType {
    match key {
        DrawKey::Shift => gdk::ModifierType::SHIFT_MASK,
        DrawKey::Control => gdk::ModifierType::CONTROL_MASK,
        DrawKey::Alt => gdk::ModifierType::ALT_MASK,
    }
}

fn other_modifiers(key: DrawKey) -> gdk::ModifierType {
    (gdk::ModifierType::SHIFT_MASK
        | gdk::ModifierType::CONTROL_MASK
        | gdk::ModifierType::ALT_MASK
        | gdk::ModifierType::SUPER_MASK
        | gdk::ModifierType::META_MASK)
        - key_mask(key)
}

type SessionCell = Rc<RefCell<Option<Session>>>;

pub struct SketchOverlay {
    layer: super::recording_overlay::LayerShell,
    session: SessionCell,
    styled: std::cell::Cell<bool>,
    /// Last recording id the overlay decided on, so settings load once.
    decided: std::cell::Cell<u64>,
    notice: RefCell<Option<String>>,
}

fn fallback_notice(access: keys::Access) -> String {
    match access {
        keys::Access::Denied => "Drawing: keyboard access not set up, so typing is paused while recording. Settings › Text shows the one-time setup.".into(),
        keys::Access::NoKeyboards => "Drawing: no keyboard found to read Shift from; typing is paused while recording.".into(),
    }
}

impl SketchOverlay {
    /// `None` without layer-shell (GNOME, X11): dictation is unaffected.
    pub fn new() -> Option<Self> {
        let layer = super::recording_overlay::LayerShell::load()
            .inspect_err(|error| eprintln!("Draw while dictating unavailable: {error}"))
            .ok()?;
        Some(Self {
            layer,
            session: Rc::new(RefCell::new(None)),
            styled: std::cell::Cell::new(false),
            decided: std::cell::Cell::new(0),
            notice: RefCell::new(None),
        })
    }

    /// Call on every UI poll with the active recording (its start and the id
    /// from [`begin_recording`]). Opens the overlay once per dictation when
    /// drawing is enabled; finishes it when the recording ends.
    pub fn update(
        &self,
        application: &gtk::Application,
        recording: Option<(Instant, u64)>,
        ask: bool,
        paused: bool,
    ) {
        let Ok(mut current) = self.session.try_borrow_mut() else {
            return;
        };
        match (current.as_mut(), recording) {
            (None, Some((started, id))) if !ask && self.decided.replace(id) != id => {
                let settings = crate::settings::Settings::load().unwrap_or_default();
                if settings.draw_while_dictating {
                    drop(current);
                    let keyboards = settings
                        .draw_key_from_keyboard
                        .then(keys::Keyboards::open)
                        .and_then(|result| {
                            result
                                .inspect_err(|access| {
                                    *self.notice.borrow_mut() = Some(fallback_notice(*access));
                                })
                                .ok()
                        });
                    self.open(application, started, id, settings.draw_key, keyboards);
                }
            }
            (Some(session), Some(_)) => session.clock.observe(paused, Instant::now()),
            (Some(session), None) if !session.finishing => {
                session.finishing = true;
                session.release();
                // Close keyboard devices as soon as recording ends.
                session.keyboards = None;
                if let Some(first) = session.surfaces.first()
                    && let Err(error) = self.layer.set_keyboard(&first.window, 0)
                {
                    eprintln!("Sketch keyboard release failed: {error}");
                }
            }
            _ => {}
        }
    }

    /// A one-time message for the main window (e.g. the focus fallback).
    pub fn take_notice(&self) -> Option<String> {
        self.notice.borrow_mut().take()
    }

    fn style(&self) {
        if self.styled.replace(true) {
            return;
        }
        if let Some(display) = gdk::Display::default() {
            let css = gtk::CssProvider::new();
            css.load_from_data("window.koett-sketch, window.koett-sketch > * { background: transparent; box-shadow: none; }");
            gtk::style_context_add_provider_for_display(
                &display,
                &css,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
            );
        }
    }

    fn open(
        &self,
        application: &gtk::Application,
        started: Instant,
        recording: u64,
        key: DrawKey,
        keyboards: Option<keys::Keyboards>,
    ) {
        let focus = keyboards.is_none();
        let Some(display) = gdk::Display::default() else {
            return;
        };
        self.style();
        let monitors: Vec<gdk::Monitor> = display
            .monitors()
            .iter::<gdk::Monitor>()
            .filter_map(Result::ok)
            .collect();
        let rect = |monitor: &gdk::Monitor| {
            let area = monitor.geometry();
            (area.x(), area.y(), area.width(), area.height())
        };
        let desktop =
            monitors
                .iter()
                .map(rect)
                .fold(None, |union: Option<capture::Rect>, (x, y, w, h)| {
                    Some(match union {
                        None => (x, y, w, h),
                        Some((ux, uy, uw, uh)) => {
                            let (left, top) = (ux.min(x), uy.min(y));
                            let (right, bottom) = ((ux + uw).max(x + w), (uy + uh).max(y + h));
                            (left, top, right - left, bottom - top)
                        }
                    })
                });
        let mut surfaces = Vec::new();
        for (index, monitor) in monitors.iter().enumerate() {
            let window = gtk::Window::builder()
                .application(application)
                .title("Koett sketch")
                .decorated(false)
                .build();
            if let Err(error) = self.layer.cover(&window, monitor, focus && index == 0) {
                eprintln!("Sketch overlay unavailable: {error}");
                window.destroy();
                continue;
            }
            window.add_css_class("koett-sketch");
            let area = gtk::DrawingArea::new();
            area.set_hexpand(true);
            area.set_vexpand(true);
            window.set_child(Some(&area));
            let surface = surfaces.len();
            area.set_draw_func({
                let session = Rc::downgrade(&self.session);
                move |area, context, _, _| {
                    if let Some(session) = session.upgrade()
                        && let Ok(session) = session.try_borrow()
                        && let Some(session) = session.as_ref()
                    {
                        paint(session, surface, context);
                        // Remember which frame shows the isolated ink, so the
                        // capture waits until that frame is presented.
                        let frame = area.frame_clock().map(|clock| clock.frame_counter());
                        for group in &session.groups {
                            if group.surface == surface && matches!(group.phase, Phase::Queued(_)) {
                                group.painted.set(frame);
                            }
                        }
                    }
                }
            });
            let motion = gtk::EventControllerMotion::new();
            motion.connect_motion({
                let session = Rc::downgrade(&self.session);
                move |_, x, y| {
                    let Some(session) = session.upgrade() else {
                        return;
                    };
                    let Ok(mut session) = session.try_borrow_mut() else {
                        return;
                    };
                    let Some(session) = session.as_mut() else {
                        return;
                    };
                    if !session.held || session.finishing {
                        return;
                    }
                    // Pointer events can carry stale modifier state (seen on
                    // KWin), so drawing follows the overlay's own key events.
                    session.point(surface, x, y);
                }
            });
            motion.connect_leave({
                let session = Rc::downgrade(&self.session);
                move |_| {
                    if let Some(session) = session.upgrade()
                        && let Ok(mut session) = session.try_borrow_mut()
                        && let Some(session) = session.as_mut()
                    {
                        session.lift();
                    }
                }
            });
            area.add_controller(motion);
            if focus && index == 0 {
                window.add_controller(self.keys());
                // A missed key release must not leave clicks blocked.
                window.connect_is_active_notify({
                    let session = Rc::downgrade(&self.session);
                    move |window| {
                        if !window.is_active()
                            && let Some(session) = session.upgrade()
                            && let Ok(mut session) = session.try_borrow_mut()
                            && let Some(session) = session.as_mut()
                            && session.held
                        {
                            session.release();
                        }
                    }
                });
            }
            window.connect_realize(|window| {
                if let Some(native) = window.surface() {
                    native.set_input_region(Some(&gtk::cairo::Region::create()));
                }
            });
            surfaces.push(Surface {
                window,
                area,
                target: capture::Target {
                    connector: monitor.connector().map(|name| name.to_string()),
                    monitor: rect(monitor),
                    desktop: desktop.unwrap_or_else(|| rect(monitor)),
                },
            });
        }
        if surfaces.is_empty() {
            return;
        }
        frames::set_open(recording, true);
        for surface in &surfaces {
            surface.window.present();
        }
        *self.session.borrow_mut() = Some(Session {
            recording,
            started,
            clock: AudioClock::default(),
            key,
            surfaces,
            groups: Vec::new(),
            next_id: 0,
            held: false,
            stroking: None,
            finishing: false,
            results: Arc::new(Mutex::new(Vec::new())),
            keyboards,
        });
        // 16 ms: device mode polls the draw key on this tick.
        glib::timeout_add_local(Duration::from_millis(16), {
            let session = self.session.clone();
            move || {
                let Ok(mut current) = session.try_borrow_mut() else {
                    return glib::ControlFlow::Continue;
                };
                if current.as_mut().is_some_and(Session::tick) {
                    return glib::ControlFlow::Continue;
                }
                if let Some(done) = current.take() {
                    for surface in done.surfaces {
                        surface.window.destroy();
                    }
                    frames::set_open(done.recording, false);
                }
                glib::ControlFlow::Break
            }
        });
    }

    fn keys(&self) -> gtk::EventControllerKey {
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed({
            let session = Rc::downgrade(&self.session);
            move |_, keyval, _, state| {
                let Some(session) = session.upgrade() else {
                    return glib::Propagation::Proceed;
                };
                let Ok(mut session) = session.try_borrow_mut() else {
                    return glib::Propagation::Proceed;
                };
                let Some(session) = session.as_mut().filter(|session| !session.finishing) else {
                    return glib::Propagation::Proceed;
                };
                if key_matches(session.key, keyval)
                    && !state.intersects(other_modifiers(session.key))
                {
                    session.press();
                } else if keyval.to_lower() == gdk::Key::z
                    && state.contains(gdk::ModifierType::CONTROL_MASK)
                {
                    session.undo();
                }
                glib::Propagation::Stop
            }
        });
        keys.connect_key_released({
            let session = Rc::downgrade(&self.session);
            move |_, keyval, _, _| {
                if let Some(session) = session.upgrade()
                    && let Ok(mut session) = session.try_borrow_mut()
                    && let Some(session) = session.as_mut()
                    && key_matches(session.key, keyval)
                {
                    session.release();
                }
            }
        });
        keys
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_key_ignores_chords_with_other_modifiers() {
        let others = other_modifiers(DrawKey::Shift);
        assert!(others.contains(gdk::ModifierType::CONTROL_MASK));
        assert!(!others.contains(gdk::ModifierType::SHIFT_MASK));
        assert!(key_matches(DrawKey::Shift, gdk::Key::Shift_R));
        assert!(!key_matches(DrawKey::Shift, gdk::Key::Control_L));
        assert!(key_matches(DrawKey::Alt, gdk::Key::Alt_L));
    }

    #[test]
    fn captured_ink_fades_after_the_scrawl_delay() {
        let now = Instant::now();
        let mut group = Group {
            id: 1,
            surface: 0,
            strokes: Vec::new(),
            start: now,
            end: now,
            audio_start: Duration::ZERO,
            painted: std::cell::Cell::new(None),
            lifted: Some(now),
            closed: false,
            phase: Phase::Capturing,
        };
        assert_eq!(group.alpha(now + Duration::from_secs(5)), 1.0);
        group.phase = Phase::Captured;
        assert_eq!(group.alpha(now + FADE_AFTER), 1.0);
        assert_eq!(group.alpha(now + FADE_AFTER + FADE_FOR), 0.0);
    }

    fn group(id: u64, surface: usize, phase: Phase) -> Group {
        let now = Instant::now();
        Group {
            id,
            surface,
            strokes: vec![vec![(1.0, 1.0)]],
            start: now,
            end: now,
            audio_start: Duration::ZERO,
            painted: std::cell::Cell::new(None),
            lifted: None,
            closed: false,
            phase,
        }
    }

    #[test]
    fn a_capture_shows_only_its_own_ink_on_that_monitor() {
        let now = Instant::now();
        let groups = [
            group(1, 0, Phase::Captured),
            group(2, 0, Phase::Queued(now)),
            group(3, 0, Phase::Drawing),
            group(4, 1, Phase::Drawing),
        ];
        let shown = |surface| {
            groups
                .iter()
                .filter(|group| visible(&groups, surface, group))
                .map(|group| group.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            shown(0),
            [2],
            "older fading ink and the next sketch stay hidden"
        );
        assert_eq!(shown(1), [4], "other monitors are unaffected");
        let idle = [group(1, 0, Phase::Captured), group(3, 0, Phase::Drawing)];
        assert_eq!(
            idle.iter().filter(|group| visible(&idle, 0, group)).count(),
            2
        );
    }

    #[test]
    fn each_hold_is_a_separate_sketch_on_any_monitor() {
        let mut groups = vec![group(1, 0, Phase::Drawing)];
        assert_eq!(continuing(&groups, 0), Some(0), "same hold, same monitor");
        assert_eq!(
            continuing(&groups, 1),
            None,
            "another monitor gets its own sketch"
        );
        groups[0].closed = true; // draw key released
        assert_eq!(
            continuing(&groups, 0),
            None,
            "next hold starts a new sketch"
        );
        groups.push(group(2, 1, Phase::Drawing));
        groups[1].closed = true;
        assert_eq!(continuing(&groups, 1), None);
    }

    #[test]
    fn capture_waits_for_the_isolated_frame_to_be_presented() {
        let soon = Duration::from_millis(50);
        assert_eq!(readiness(None, soon), Readiness::Wait, "not painted yet");
        assert_eq!(
            readiness(Some((false, 0)), soon),
            Readiness::Wait,
            "in flight"
        );
        assert_eq!(
            readiness(Some((true, 0)), soon),
            Readiness::Redraw,
            "discarded"
        );
        assert_eq!(readiness(Some((true, 123)), soon), Readiness::Capture);
        assert_eq!(
            readiness(Some((false, 0)), PRESENT_TIMEOUT),
            Readiness::Capture
        );
    }

    #[test]
    fn paused_time_does_not_advance_the_sketch_clock() {
        let started = Instant::now();
        let mut clock = AudioClock::default();
        clock.observe(false, started + Duration::from_secs(1));
        clock.observe(true, started + Duration::from_secs(5));
        let during = started + Duration::from_secs(20);
        assert_eq!(clock.at(started, during), Duration::from_secs(5));
        clock.observe(false, started + Duration::from_secs(35));
        let after = started + Duration::from_secs(37);
        assert_eq!(clock.at(started, after), Duration::from_secs(7));
        // The pill places the same sketch on wall time, pauses included.
        assert_eq!(pill_ms(started, after), 37_000);
    }
}
