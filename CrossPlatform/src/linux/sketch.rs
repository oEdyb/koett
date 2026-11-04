//! Draw while dictating. While a dictation records, a transparent layer-shell
//! overlay covers every monitor. Holding the draw key (Shift by default) and
//! moving the pointer leaves red ink; otherwise clicks pass through. Strokes on
//! one monitor close into a sketch shortly after the key is released; that
//! monitor is then captured with the ink still visible, and the ink fades.
//! A click with the key held drops a numbered pin instead (`gesture`), which
//! also reads one line of context (`pin_reader`). Delivery turns the captures
//! into one numbered contact sheet.
//!
//! A recording never blocks typing by default: Wayland gives no global key
//! state, so the draw key is read from keyboard devices (see `keys`) and the
//! overlay stays without keyboard focus. Only a user who opted in lets the
//! overlay hold focus when keyboards can't be read; otherwise drawing stays off
//! for that dictation with a one-line hint.
use crate::pin_context::PinContext;
use crate::settings::DrawKey;
use gtk4::{self as gtk, gdk, glib, prelude::*};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::{cell::RefCell, rc::Rc};

mod capture;
mod frames;
mod gesture;
pub mod keys;
mod kwin_context;
mod niri_pick;
mod pin_reader;
pub mod sheet;
mod video_time;
mod wl_click;

const INK: (f64, f64, f64) = (1.0, 59.0 / 255.0, 48.0 / 255.0);
const INK_WIDTH: f64 = 5.0;
const FADE_AFTER: Duration = Duration::from_millis(1500);
const FADE_FOR: Duration = Duration::from_millis(400);
/// Matches the pill timeline's cap (`timeline::MAX_SKETCHES`, 256 per dictation).
const MAX_SKETCHES: usize = 256;
/// Ink smaller than this (logical px) is a resting hand, not a sketch.
const MIN_INK: f64 = 12.0;
/// How often device mode looks for added or removed keyboards.
const KEYBOARD_RESCAN: Duration = Duration::from_secs(2);
const PIN_RADIUS: f64 = 16.0;
/// The reader bounds itself well below this; a frame never waits longer.
const PIN_CONTEXT_WAIT: Duration = Duration::from_millis(1500);
/// Exclusive focus moves between overlays when a click changes the active
/// monitor, and the old one can report inactive first. Only a longer gap means
/// the key release was really missed.
const FOCUS_GRACE: Duration = Duration::from_millis(100);
/// Longest wait for the frame that commits a pick's change to the input region.
const COMMIT_WAIT: Duration = Duration::from_millis(300);
/// Half the side of the hole a pick opens at a pin, in logical px: larger than the
/// pin's click slop, far smaller than a window.
const PICK_HOLE: f64 = 32.0;

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

/// Where the overlays take the pointer.
#[derive(Clone, Copy, Debug, PartialEq)]
enum PointerInput {
    Nowhere,
    Everywhere,
    /// Everywhere except a hole at this point on this surface.
    ExceptPick(usize, gesture::Point),
}

/// A monitor's whole surface, minus the hole when there is one.
fn input_region((width, height): (i32, i32), hole: Option<gesture::Point>) -> gtk::cairo::Region {
    let all = gtk::cairo::RectangleInt::new(0, 0, width, height);
    let region = gtk::cairo::Region::create_rectangle(&all);
    if let Some((x, y)) = hole {
        let side = (PICK_HOLE * 2.0) as i32;
        let around = gtk::cairo::RectangleInt::new(
            (x - PICK_HOLE).floor() as i32,
            (y - PICK_HOLE).floor() as i32,
            side,
            side,
        );
        if let Err(error) = region.subtract_rectangle(&around) {
            eprintln!("Sketch input region failed: {error}");
        }
    }
    region
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Drawing,
    /// Other ink on its monitor is hidden; capture starts after it settles.
    Queued(Instant),
    Capturing,
    /// The pin's image is taken; its context is still being read. Other
    /// captures on the monitor need not wait for that.
    Reading(Instant),
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
    /// Set for a pin: the click, in the same coordinates as strokes.
    pin: Option<(f64, f64)>,
    context: Option<std::sync::mpsc::Receiver<Option<PinContext>>>,
    png: Option<Vec<u8>>,
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
    gesture: gesture::PinGesture,
    include_selection: bool,
    pick: Arc<niri_pick::PickGate>,
    /// What the input regions were last asked to be.
    input: PointerInput,
    /// Changed the input regions at this time, with each overlay's frame counter.
    committing: Option<(Instant, Vec<i64>)>,
    last_pin: Option<(usize, gesture::Point)>,
    /// When the last active overlay went inactive during a hold.
    inactive_since: Option<Instant>,
    /// Device mode (no keyboard focus); `None` means the overlay holds focus.
    keyboards: Option<keys::Keyboards>,
    /// The user allows focus as a last resort if every keyboard is lost.
    may_take_keyboard: bool,
    /// Decides whether the sampled key state is a deliberate drawing hold.
    gate: keys::DrawGate,
    /// Keyboards were reported lost; cleared when one answers again.
    keyboards_lost: bool,
    rescanned: Option<Instant>,
    reported_unreadable: usize,
    notices: Vec<String>,
    layer: Rc<super::recording_overlay::LayerShell>,
}

impl Session {
    /// Takes the pointer while the key is held. A running window pick overrides
    /// that: input stays on until the pick is proven, then opens around the pin.
    fn sync_input(&mut self) {
        let wanted = match self.last_pin {
            Some((surface, point)) if self.pick.wants_open() => {
                PointerInput::ExceptPick(surface, point)
            }
            _ if self.pick.busy() || self.held => PointerInput::Everywhere,
            _ => PointerInput::Nowhere,
        };
        if wanted != self.input {
            self.input = wanted;
            self.set_input(wanted);
            self.committing = Some((Instant::now(), self.frame_counters()));
        }
        let committed = self.committing.as_ref().is_none_or(|(asked, before)| {
            asked.elapsed() >= COMMIT_WAIT
                || self
                    .frame_counters()
                    .iter()
                    .zip(before)
                    .all(|(now, before)| now > before)
        });
        if committed {
            self.committing = None;
            if self.pick.wants_open() {
                self.pick.opened();
            } else if self.pick.wants_closed() {
                self.pick.closed();
            }
        }
    }

    fn frame_counters(&self) -> Vec<i64> {
        self.surfaces
            .iter()
            .map(|surface| {
                surface
                    .area
                    .frame_clock()
                    .map_or(0, |clock| clock.frame_counter())
            })
            .collect()
    }

    /// Focused mode: follows a key release that focus loss swallowed.
    fn watch_focus(&mut self, now: Instant) {
        let active = self
            .surfaces
            .iter()
            .any(|surface| surface.window.is_active());
        if self.keyboards.is_some() || !self.held || active {
            self.inactive_since = None;
            return;
        }
        let since = *self.inactive_since.get_or_insert(now);
        if now.saturating_duration_since(since) >= FOCUS_GRACE {
            self.inactive_since = None;
            self.release();
        }
    }

    fn set_input(&self, input: PointerInput) {
        for (index, surface) in self.surfaces.iter().enumerate() {
            if let Some(native) = surface.window.surface() {
                let (_, _, width, height) = surface.target.monitor;
                let region = match input {
                    PointerInput::Nowhere => gtk::cairo::Region::create(),
                    PointerInput::Everywhere => input_region((width, height), None),
                    PointerInput::ExceptPick(pinned, point) => {
                        input_region((width, height), (pinned == index).then_some(point))
                    }
                };
                native.set_input_region(Some(&region));
            }
            surface
                .window
                .set_cursor_from_name((input != PointerInput::Nowhere).then_some("crosshair"));
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
        if let Some(pin) = self.gesture.release(Instant::now()) {
            self.drop_pin(pin);
        }
        self.held = false;
        self.lift();
        for group in &mut self.groups {
            group.closed = true;
        }
        self.sync_input();
    }

    fn press(&mut self) {
        self.gate.moved(Instant::now());
        if self.held {
            return;
        }
        self.held = true;
        self.gesture.hold();
        self.sync_input();
    }

    /// Raw left-button events while the overlay has the pointer.
    fn button(&mut self, surface: usize, pressed: bool, point: gesture::Point) {
        let now = Instant::now();
        self.gate.moved(now);
        if !pressed {
            if let Some(pin) = self.gesture.button_up(now) {
                self.drop_pin(pin);
            }
        } else if !self.pick.busy()
            && !self.finishing
            && self.gesture.button_down(surface, point, now) == gesture::Down::Press
        {
            self.drop_jitter();
        }
    }

    /// Ink this short in the hold was the hand settling before the click.
    fn drop_jitter(&mut self) {
        self.lift();
        while let Some(index) = self
            .groups
            .iter()
            .rposition(|group| !group.closed && group.pin.is_none())
        {
            self.drop_group(index);
        }
        self.redraw();
    }

    fn drop_pin(&mut self, mark: gesture::Mark) {
        if self.groups.len() >= MAX_SKETCHES {
            return;
        }
        self.lift();
        let (sender, receiver) = std::sync::mpsc::channel();
        self.pick.begin();
        self.last_pin = Some((mark.surface, mark.point));
        self.sync_input();
        let (include_selection, point, pick) =
            (self.include_selection, mark.point, self.pick.clone());
        let monitor = self.surfaces[mark.surface].target.monitor;
        let screen_point = (
            point.0 + f64::from(monitor.0),
            point.1 + f64::from(monitor.1),
        );
        std::thread::spawn(move || {
            let _ = sender.send(pin_reader::read(
                include_selection,
                point,
                screen_point,
                &pick,
            ));
        });
        self.next_id += 1;
        self.groups.push(Group {
            id: self.next_id,
            surface: mark.surface,
            strokes: Vec::new(),
            start: mark.at,
            end: mark.at,
            audio_start: self.clock.at(self.started, mark.at),
            painted: std::cell::Cell::new(None),
            lifted: Some(Instant::now()),
            closed: true,
            phase: Phase::Drawing,
            pin: Some(mark.point),
            context: Some(receiver),
            png: None,
        });
        self.surfaces[mark.surface].area.queue_draw();
        self.publish();
    }

    fn pointer_moved(&mut self, surface: usize, point: gesture::Point) {
        let now = Instant::now();
        if let gesture::Motion::Ink { from, to } = self.gesture.motion(surface, point, now) {
            if let Some(from) = from {
                self.point(surface, from.point.0, from.point.1, from.at);
            }
            self.point(surface, to.0, to.1, now);
        }
    }

    /// Device mode: follows the draw key from keyboard modifier state,
    /// picking up hot-plugged keyboards and reporting unreadable ones.
    fn poll_keys(&mut self) {
        let Some(keyboards) = self.keyboards.as_mut() else {
            return;
        };
        if self
            .rescanned
            .is_none_or(|at| at.elapsed() >= KEYBOARD_RESCAN)
        {
            keyboards.refresh();
            self.rescanned = Some(Instant::now());
            let unreadable = keyboards.unreadable();
            if unreadable > self.reported_unreadable {
                self.notices.push(format!(
                    "Drawing: {unreadable} keyboard(s) can't be read, so Shift on them won't draw. Settings › Draw on screen has the setup."
                ));
            }
            self.reported_unreadable = unreadable;
        }
        let (held, others) = keyboards.modifiers().for_key(self.key);
        let held = held && !self.finishing;
        if keyboards.is_empty() {
            if self.may_take_keyboard {
                self.keyboards = None;
                for surface in &self.surfaces {
                    if let Err(error) = self.layer.set_keyboard(&surface.window, 1) {
                        eprintln!("Sketch keyboard focus failed: {error}");
                    }
                }
                self.notices.push(
                    "Drawing: keyboards can't be read, so typing is paused for the rest of this recording."
                        .into(),
                );
                return;
            }
            // Keep rescanning: a replugged keyboard draws again; typing is untouched.
            if !self.keyboards_lost {
                self.keyboards_lost = true;
                self.notices.push(
                    "Drawing: keyboards can't be read right now, so the draw key won't draw."
                        .into(),
                );
            }
        } else {
            self.keyboards_lost = false;
        }
        match (self.gate.update(held, others, Instant::now()), self.held) {
            (true, false) => self.press(),
            (false, true) => self.release(),
            _ => {}
        }
    }

    fn point(&mut self, surface: usize, x: f64, y: f64, now: Instant) {
        self.gate.moved(now);
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
                        pin: None,
                        context: None,
                        png: None,
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

    /// Hands over captured images whose context, if any, has arrived.
    fn finish_frames(&mut self, now: Instant) {
        for group in &mut self.groups {
            let Phase::Reading(since) = group.phase else {
                continue;
            };
            let context = match group.context.as_ref().map(|context| context.try_recv()) {
                None => None,
                Some(Ok(context)) => context,
                Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => None,
                Some(Err(std::sync::mpsc::TryRecvError::Empty))
                    if now.saturating_duration_since(since) < PIN_CONTEXT_WAIT =>
                {
                    continue;
                }
                Some(Err(std::sync::mpsc::TryRecvError::Empty)) => None,
            };
            group.phase = Phase::Captured;
            group.context = None;
            if let Some(png) = group.png.take() {
                frames::push(
                    self.recording,
                    Keyframe {
                        order: group.id,
                        start: group.audio_start.as_secs_f32(),
                        png,
                        context,
                    },
                );
            }
        }
    }

    /// One timer step. Returns false once a finishing session is complete.
    fn tick(&mut self) -> bool {
        self.poll_keys();
        // Focused mode has no key state to sample: a missed release must not
        // leave clicks blocked once the pointer has stopped drawing.
        if self.keyboards.is_none() && self.held && self.gate.expired(Instant::now()) {
            self.release();
        }
        let now = Instant::now();
        self.watch_focus(now);
        self.sync_input();
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
                    self.groups[index].png = Some(png);
                    self.groups[index].phase = Phase::Reading(now);
                }
                Err(error) => {
                    eprintln!("Sketch capture failed: {error}");
                    self.drop_group(index);
                }
            }
            self.redraw();
        }
        while let Some(index) = self.groups.iter().position(|group| {
            group.phase == Phase::Drawing
                && self.stroking != Some(group.id)
                && (group.closed || self.finishing)
                && group.pin.is_none()
                && accidental(&group.strokes)
        }) {
            self.drop_group(index);
            self.redraw();
        }
        self.finish_frames(now);
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
                    if timing.is_none_or(|(_, presented)| presented <= 0) {
                        eprintln!("Sketch capture without presentation feedback");
                    }
                }
            }
            {
                let (id, target) = (group.id, self.surfaces[group.surface].target.clone());
                let strokes = match group.pin {
                    Some(pin) => vec![vec![pin]],
                    None => group.strokes.clone(),
                };
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
        if self.finishing && !busy && !self.pick.busy() {
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
        if let Some(pin) = group.pin {
            paint_pin(context, pin, alpha);
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

/// Whether strokes are too small to be a deliberate sketch.
fn accidental(strokes: &[Vec<(f64, f64)>]) -> bool {
    let mut points = strokes.iter().flatten();
    let Some(&(x, y)) = points.next() else {
        return true;
    };
    let (mut left, mut right, mut top, mut bottom) = (x, x, y, y);
    for &(x, y) in points {
        (left, right) = (left.min(x), right.max(x));
        (top, bottom) = (top.min(y), bottom.max(y));
    }
    (right - left).max(bottom - top) < MIN_INK
}

/// How one dictation's overlay sees the draw key.
#[derive(Debug)]
enum DrawInput<K> {
    /// Keyboard state from devices: no focus, typing keeps working.
    Devices(K),
    /// The user opted in to pausing typing when devices can't be read.
    Focus,
    /// Drawing stays off for this dictation; the keyboard is never taken.
    Off,
}

fn draw_input<K>(access: Result<K, keys::Access>, may_take_keyboard: bool) -> DrawInput<K> {
    match access {
        Ok(keyboards) => DrawInput::Devices(keyboards),
        Err(_) if may_take_keyboard => DrawInput::Focus,
        Err(_) => DrawInput::Off,
    }
}

/// A pin's ring, white-edged so it reads on dark and light pages.
fn paint_pin(context: &gtk::cairo::Context, (x, y): (f64, f64), alpha: f64) {
    let full_circle = std::f64::consts::TAU;
    context.set_line_width(INK_WIDTH + 3.0);
    context.set_source_rgba(1.0, 1.0, 1.0, 0.9 * alpha);
    context.arc(x, y, PIN_RADIUS, 0.0, full_circle);
    let _ = context.stroke();
    context.set_line_width(INK_WIDTH);
    context.set_source_rgba(INK.0, INK.1, INK.2, alpha);
    context.arc(x, y, PIN_RADIUS, 0.0, full_circle);
    let _ = context.stroke();
    context.arc(x, y, 3.0, 0.0, full_circle);
    let _ = context.fill();
}

/// Whether each monitor's overlay asks for exclusive keyboard focus. In
/// focused mode every overlay asks: compositors grant it per output (niri only
/// on its active output), so asking on the first monitor alone left Shift
/// unseen whenever the user worked on another one.
fn keyboard_modes(focus_mode: bool, monitors: usize) -> Vec<bool> {
    vec![focus_mode; monitors]
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
    layer: Rc<super::recording_overlay::LayerShell>,
    session: SessionCell,
    styled: std::cell::Cell<bool>,
    /// Last recording id the overlay decided on, so settings load once.
    decided: std::cell::Cell<u64>,
    notice: RefCell<Option<String>>,
    hinted: std::cell::Cell<bool>,
}

fn unavailable_hint(access: keys::Access, focus: bool) -> &'static str {
    match (access, focus) {
        (keys::Access::Denied, false) => {
            "Drawing is off: Koett needs keyboard access to see the draw key. Settings › Draw on screen has a one-line setup."
        }
        (keys::Access::NoKeyboards, false) => {
            "Drawing is off: no keyboard found to read the draw key from."
        }
        (_, true) => "Drawing: keyboards can't be read, so typing is paused while recording.",
    }
}

impl SketchOverlay {
    /// `None` without layer-shell (GNOME, X11): dictation is unaffected.
    pub fn new() -> Option<Self> {
        let layer = super::recording_overlay::LayerShell::load()
            .inspect_err(|error| eprintln!("Draw while dictating unavailable: {error}"))
            .ok()?;
        Some(Self {
            layer: Rc::new(layer),
            session: Rc::new(RefCell::new(None)),
            styled: std::cell::Cell::new(false),
            decided: std::cell::Cell::new(0),
            notice: RefCell::new(None),
            hinted: std::cell::Cell::new(false),
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
                    let access = keys::Keyboards::open();
                    let failure = access.as_ref().err().copied();
                    let may_take = settings.draw_may_take_keyboard;
                    let input = draw_input(access, may_take);
                    if let Some(failure) = failure {
                        self.hint(unavailable_hint(failure, may_take));
                    }
                    let keyboards = match input {
                        DrawInput::Devices(keyboards) => Some(keyboards),
                        DrawInput::Focus => None,
                        DrawInput::Off => return,
                    };
                    self.open(application, started, id, &settings, keyboards);
                }
            }
            (Some(session), Some(_)) => {
                session.clock.observe(paused, Instant::now());
                if !session.notices.is_empty() {
                    *self.notice.borrow_mut() =
                        Some(std::mem::take(&mut session.notices).join(" "));
                }
            }
            (Some(session), None) if !session.finishing => {
                session.release();
                session.finishing = true;
                // Close keyboard devices as soon as recording ends.
                session.keyboards = None;
                for surface in &session.surfaces {
                    if let Err(error) = self.layer.set_keyboard(&surface.window, 0) {
                        eprintln!("Sketch keyboard release failed: {error}");
                    }
                }
            }
            _ => {}
        }
    }

    /// Tells the user why drawing is limited: in the main window every time,
    /// and as a desktop notification once per run, since the window is usually
    /// hidden while dictating.
    fn hint(&self, message: &'static str) {
        *self.notice.borrow_mut() = Some(message.to_owned());
        if !self.hinted.replace(true) {
            let _ = std::process::Command::new("notify-send")
                .args(["--app-name=Koett", "Koett", message])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
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
        settings: &crate::settings::Settings,
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
        let modes = keyboard_modes(focus, monitors.len());
        for (index, monitor) in monitors.iter().enumerate() {
            let window = gtk::Window::builder()
                .application(application)
                .title("Koett sketch")
                .decorated(false)
                .build();
            if let Err(error) = self.layer.cover(&window, monitor, modes[index]) {
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
                    session.pointer_moved(surface, (x, y));
                }
            });
            motion.connect_leave({
                let session = Rc::downgrade(&self.session);
                move |_| {
                    if let Some(session) = session.upgrade()
                        && let Ok(mut session) = session.try_borrow_mut()
                        && let Some(session) = session.as_mut()
                    {
                        session.pick.pointer_left();
                        session.sync_input();
                        session.gesture.lift();
                        session.lift();
                    }
                }
            });
            motion.connect_enter({
                let session = Rc::downgrade(&self.session);
                move |_, _, _| {
                    if let Some(session) = session.upgrade()
                        && let Ok(session) = session.try_borrow()
                        && let Some(session) = session.as_ref()
                    {
                        session.pick.pointer_entered();
                    }
                }
            });
            area.add_controller(motion);
            // Raw events: GestureClick would cancel when a press turns into a drag.
            let buttons = gtk::EventControllerLegacy::new();
            buttons.connect_event({
                let session = Rc::downgrade(&self.session);
                move |_, event| {
                    let pressed = match event.event_type() {
                        gdk::EventType::ButtonPress => true,
                        gdk::EventType::ButtonRelease => false,
                        _ => return glib::Propagation::Proceed,
                    };
                    if let Some(button) = event.downcast_ref::<gdk::ButtonEvent>()
                        && button.button() == gdk::BUTTON_PRIMARY
                        && let Some(point) = event.position()
                        && let Some(session) = session.upgrade()
                        && let Ok(mut session) = session.try_borrow_mut()
                        && let Some(session) = session.as_mut()
                    {
                        session.button(surface, pressed, point);
                    }
                    glib::Propagation::Proceed
                }
            });
            area.add_controller(buttons);
            // Every monitor's overlay listens: the compositor focuses the one on
            // its active output. Device mode can fall back to focus
            // mid-recording, so the controller is always present.
            {
                window.add_controller(self.keys());
                // A missed key release cannot leave clicks blocked: `watch_focus`.
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
            key: settings.draw_key,
            surfaces,
            groups: Vec::new(),
            next_id: 0,
            held: false,
            stroking: None,
            finishing: false,
            results: Arc::new(Mutex::new(Vec::new())),
            gesture: gesture::PinGesture::default(),
            include_selection: settings.pin_selected_text,
            pick: Arc::new(niri_pick::PickGate::default()),
            input: PointerInput::Nowhere,
            committing: None,
            last_pin: None,
            inactive_since: None,
            keyboards,
            may_take_keyboard: settings.draw_may_take_keyboard,
            gate: keys::DrawGate::default(),
            keyboards_lost: false,
            rescanned: None,
            reported_unreadable: 0,
            notices: Vec::new(),
            layer: self.layer.clone(),
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
    fn a_picks_hole_lets_through_only_the_pin() {
        let region = input_region((1000, 800), Some((500.0, 400.0)));
        assert!(!region.contains_point(500, 400), "the pin itself");
        assert!(!region.contains_point(530, 400), "a hand that barely moved");
        assert!(region.contains_point(540, 400), "the pointer moved on");
        assert!(region.contains_point(100, 100), "a click elsewhere");
    }

    #[test]
    fn a_hole_at_the_screen_edge_keeps_the_rest_of_the_screen() {
        let region = input_region((1000, 800), Some((5.0, 5.0)));
        assert!(!region.contains_point(0, 0));
        assert!(region.contains_point(60, 60));
    }

    #[test]
    fn a_pin_is_never_dropped_as_a_resting_hand() {
        let mut pin = group(1, 0, Phase::Drawing);
        pin.strokes.clear();
        pin.pin = Some((5.0, 5.0));
        assert!(accidental(&pin.strokes), "a pin has no strokes");
        let mut session_groups = vec![pin];
        session_groups.retain(|group| !(group.pin.is_none() && accidental(&group.strokes)));
        assert_eq!(session_groups.len(), 1);
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
            pin: None,
            context: None,
            png: None,
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
            pin: None,
            context: None,
            png: None,
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
    fn a_recording_never_takes_the_keyboard_unless_opted_in() {
        // Olle's bug: drawing on, no opt-in, two monitors, and typing died.
        let readable = draw_input(Ok(()), false);
        assert!(matches!(readable, DrawInput::Devices(())));
        let opted_in_but_readable = draw_input(Ok(()), true);
        assert!(
            matches!(opted_in_but_readable, DrawInput::Devices(())),
            "devices win even with the opt-in, so typing keeps working"
        );
        for access in [keys::Access::Denied, keys::Access::NoKeyboards] {
            assert!(matches!(
                draw_input::<()>(Err(access), false),
                DrawInput::Off
            ));
            assert!(matches!(
                draw_input::<()>(Err(access), true),
                DrawInput::Focus
            ));
        }
        assert_eq!(keyboard_modes(false, 2), [false, false]);
    }

    #[test]
    fn a_resting_hand_is_not_a_sketch() {
        assert!(accidental(&[]));
        assert!(accidental(&[vec![(100.0, 100.0), (104.0, 103.0)]]));
        assert!(!accidental(&[vec![(100.0, 100.0), (100.0, 140.0)]]));
        assert!(!accidental(&[vec![(0.0, 0.0)], vec![(30.0, 0.0)]]));
    }

    #[test]
    fn focused_mode_asks_for_keyboard_focus_on_every_monitor() {
        // Olle's niri: overlay on DP-1 and DP-3, active output DP-3. Only the
        // active output's exclusive surface gets keys, so all must ask.
        assert_eq!(keyboard_modes(true, 2), [true, true]);
        assert_eq!(keyboard_modes(true, 1), [true]);
        assert_eq!(
            keyboard_modes(false, 2),
            [false, false],
            "device mode never takes focus"
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
