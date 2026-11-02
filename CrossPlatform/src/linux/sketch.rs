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
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};
use std::{cell::RefCell, rc::Rc};

mod capture;
pub mod sheet;

const INK: (f64, f64, f64) = (1.0, 59.0 / 255.0, 48.0 / 255.0);
const INK_WIDTH: f64 = 5.0;
const CAPTURE_AFTER: Duration = Duration::from_millis(700);
const FADE_AFTER: Duration = Duration::from_millis(1500);
const FADE_FOR: Duration = Duration::from_millis(400);
const GROUP_STROKES: usize = 12;
const GROUP_SPAN: Duration = Duration::from_secs(20);

/// A captured sketch: start seconds on the recording clock and the monitor PNG.
pub struct Keyframe {
    pub start: f32,
    pub png: Vec<u8>,
}

struct Shared {
    open: bool,
    frames: Vec<Keyframe>,
}

static SHARED: Mutex<Shared> = Mutex::new(Shared {
    open: false,
    frames: Vec::new(),
});
static CLOSED: Condvar = Condvar::new();

fn shared() -> std::sync::MutexGuard<'static, Shared> {
    SHARED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Waits (bounded) for the overlay to finish its captures, then returns the
/// keyframes of the last recording in time order. Empty when nothing was drawn.
pub fn take_keyframes(timeout: Duration) -> Vec<Keyframe> {
    let guard = shared();
    let (mut guard, _) = CLOSED
        .wait_timeout_while(guard, timeout, |state| state.open)
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut frames = std::mem::take(&mut guard.frames);
    frames.sort_by(|a, b| a.start.total_cmp(&b.start));
    frames
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Drawing,
    Capturing,
    Captured,
}

struct Group {
    id: u64,
    surface: usize,
    strokes: Vec<Vec<(f64, f64)>>,
    start: Instant,
    end: Instant,
    lifted: Option<Instant>,
    phase: Phase,
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
    started: Instant,
    key: DrawKey,
    surfaces: Vec<Surface>,
    groups: Vec<Group>,
    next_id: u64,
    held: bool,
    stroking: Option<u64>,
    finishing: bool,
    results: CaptureResults,
}

impl Session {
    fn seconds(&self, at: Instant) -> f32 {
        at.saturating_duration_since(self.started).as_secs_f32()
    }

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

    fn release(&mut self) {
        self.held = false;
        self.lift();
        self.set_input(false);
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
                let open = self.groups.iter().position(|group| {
                    group.surface == surface
                        && group.phase == Phase::Drawing
                        && group.strokes.len() < GROUP_STROKES
                        && now.duration_since(group.start) < GROUP_SPAN
                });
                let index = open.unwrap_or_else(|| {
                    self.next_id += 1;
                    self.groups.push(Group {
                        id: self.next_id,
                        surface,
                        strokes: Vec::new(),
                        start: now,
                        end: now,
                        lifted: None,
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
                self.groups.remove(index);
            }
            self.publish();
            self.redraw();
        }
    }

    /// Republishes every sketch to the recording pill's timeline. Numbers
    /// follow start order, matching the pasted contact sheet and bullets.
    fn publish(&self) {
        let mut groups: Vec<&Group> = self.groups.iter().collect();
        groups.sort_by_key(|group| group.start);
        let millis = |at: Instant| {
            u64::try_from(at.saturating_duration_since(self.started).as_millis())
                .unwrap_or(u64::MAX)
        };
        for (index, group) in groups.into_iter().enumerate() {
            crate::sketch::publish(crate::sketch::SketchEvent {
                index: u32::try_from(index + 1).unwrap_or(u32::MAX),
                start_ms: millis(group.start),
                end_ms: millis(group.end),
                live: self.stroking == Some(group.id),
            });
        }
    }

    /// One timer step. Returns false once a finishing session is complete.
    fn tick(&mut self) -> bool {
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
                    let group = &self.groups[index];
                    let frame = Keyframe {
                        start: self.seconds(group.start),
                        png,
                    };
                    shared().frames.push(frame);
                }
                Err(error) => {
                    eprintln!("Sketch capture failed: {error}");
                    self.groups.remove(index);
                }
            }
        }
        for index in 0..self.groups.len() {
            let group = &self.groups[index];
            let idle = group
                .lifted
                .is_some_and(|lifted| now >= lifted + CAPTURE_AFTER);
            if group.phase == Phase::Drawing
                && self.stroking != Some(group.id)
                && (idle || self.finishing)
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
        .filter(|group| group.surface == surface)
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
        })
    }

    /// Call on every UI poll. Opens the overlay when a dictation recording
    /// starts with drawing enabled; finishes it when the recording ends.
    pub fn update(&self, application: &gtk::Application, recording: Option<Instant>, ask: bool) {
        let Ok(mut current) = self.session.try_borrow_mut() else {
            return;
        };
        match (current.as_mut(), recording) {
            (None, Some(started)) if !ask => {
                let settings = crate::settings::Settings::load().unwrap_or_default();
                if settings.draw_while_dictating {
                    drop(current);
                    self.open(application, started, settings.draw_key);
                }
            }
            (Some(session), None) if !session.finishing => {
                session.finishing = true;
                session.release();
                if let Some(first) = session.surfaces.first()
                    && let Err(error) = self.layer.set_keyboard(&first.window, 0)
                {
                    eprintln!("Sketch keyboard release failed: {error}");
                }
            }
            _ => {}
        }
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

    fn open(&self, application: &gtk::Application, started: Instant, key: DrawKey) {
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
            if let Err(error) = self.layer.cover(&window, monitor, index == 0) {
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
                move |_, context, _, _| {
                    if let Some(session) = session.upgrade()
                        && let Ok(session) = session.try_borrow()
                        && let Some(session) = session.as_ref()
                    {
                        paint(session, surface, context);
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
            if index == 0 {
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
        {
            let mut state = shared();
            state.open = true;
            state.frames.clear();
        }
        for surface in &surfaces {
            surface.window.present();
        }
        *self.session.borrow_mut() = Some(Session {
            started,
            key,
            surfaces,
            groups: Vec::new(),
            next_id: 0,
            held: false,
            stroking: None,
            finishing: false,
            results: Arc::new(Mutex::new(Vec::new())),
        });
        glib::timeout_add_local(Duration::from_millis(33), {
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
                }
                shared().open = false;
                CLOSED.notify_all();
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
                    session.held = true;
                    session.set_input(true);
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
            lifted: Some(now),
            phase: Phase::Capturing,
        };
        assert_eq!(group.alpha(now + Duration::from_secs(5)), 1.0);
        group.phase = Phase::Captured;
        assert_eq!(group.alpha(now + FADE_AFTER), 1.0);
        assert_eq!(group.alpha(now + FADE_AFTER + FADE_FOR), 0.0);
    }

    #[test]
    fn take_keyframes_returns_time_order_and_waits_only_while_open() {
        {
            let mut state = shared();
            state.open = false;
            state.frames = vec![
                Keyframe {
                    start: 2.0,
                    png: vec![2],
                },
                Keyframe {
                    start: 1.0,
                    png: vec![1],
                },
            ];
        }
        let started = Instant::now();
        let frames = take_keyframes(Duration::from_secs(5));
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(
            frames.iter().map(|frame| frame.png[0]).collect::<Vec<_>>(),
            [1, 2]
        );
        assert!(take_keyframes(Duration::ZERO).is_empty());
    }
}
