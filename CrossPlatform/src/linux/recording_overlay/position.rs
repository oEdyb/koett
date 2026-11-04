use super::*;

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
struct Position {
    connector: Option<String>,
    x: u32,
    y: u32,
}

const DRAG_THRESHOLD: f64 = 4.0;

fn file() -> Option<std::path::PathBuf> {
    Some(
        crate::paths::settings_file()
            .ok()?
            .with_file_name("pill-position.json"),
    )
}

thread_local! { static DRAGGING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; static STORED: RefCell<Option<Option<Position>>> = const { RefCell::new(None) }; }
fn load() -> Option<Position> {
    STORED.with(|stored| {
        stored
            .borrow_mut()
            .get_or_insert_with(|| serde_json::from_slice(&std::fs::read(file()?).ok()?).ok())
            .clone()
    })
}

fn save(position: &Position) {
    STORED.with(|stored| *stored.borrow_mut() = Some(Some(position.clone())));
    let result = (|| {
        let path = file().ok_or("Position directory unavailable".to_string())?;
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        let temporary = path.with_extension("json.new");
        std::fs::write(
            &temporary,
            serde_json::to_vec(position).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        std::fs::rename(temporary, path).map_err(|e| e.to_string())
    })();
    if let Err(error) = result {
        eprintln!("Could not remember pill position: {error}");
    }
}

fn monitors(window: &gtk::Window) -> Vec<gtk::gdk::Monitor> {
    gtk::prelude::WidgetExt::display(window)
        .monitors()
        .iter::<gtk::gdk::Monitor>()
        .filter_map(Result::ok)
        .filter(|m| m.is_valid())
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Rect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

impl From<gtk::gdk::Rectangle> for Rect {
    fn from(rect: gtk::gdk::Rectangle) -> Self {
        Self {
            x: rect.x(),
            y: rect.y(),
            width: rect.width(),
            height: rect.height(),
        }
    }
}

/// Top-left margins for a monitor-relative pill position, kept fully on screen.
fn clamped(rect: Rect, size: (i32, i32), wanted: (f64, f64)) -> (i32, i32) {
    let max_x = (rect.width - size.0).max(0);
    let max_y = (rect.height - size.1).max(0);
    (
        wanted.0.round().clamp(0.0, f64::from(max_x)) as i32,
        wanted.1.round().clamp(0.0, f64::from(max_y)) as i32,
    )
}

fn position_for(
    connector: Option<String>,
    rect: Rect,
    size: (i32, i32),
    (left, top): (i32, i32),
) -> Position {
    let scale = |value: i32, max: i32| {
        if max <= 0 {
            0
        } else {
            (f64::from(value) * 10000.0 / f64::from(max)).round() as u32
        }
    };
    Position {
        connector,
        x: scale(left, rect.width - size.0),
        y: scale(top, rect.height - size.1),
    }
}

fn pixels(position: &Position, rect: Rect, size: (i32, i32)) -> (i32, i32) {
    let scale = |value: u32, max: i32| {
        (f64::from(max.max(0)) * f64::from(value.min(10000)) / 10000.0).round() as i32
    };
    (
        scale(position.x, rect.width - size.0),
        scale(position.y, rect.height - size.1),
    )
}

fn monitor_under(rects: &[Rect], (x, y): (f64, f64)) -> Option<usize> {
    rects.iter().position(|rect| {
        x >= f64::from(rect.x)
            && y >= f64::from(rect.y)
            && x < f64::from(rect.x + rect.width)
            && y < f64::from(rect.y + rect.height)
    })
}

fn size_on(window: &gtk::Window, rect: Rect) -> (i32, i32) {
    (
        window.width().max(WIDTH).min(rect.width),
        window
            .height()
            .max(window.default_height())
            .max(ROW)
            .min(rect.height),
    )
}

fn place(window: &gtk::Window, layer: &LayerShell, position: &Position) {
    let outputs = monitors(window);
    let Some(monitor) = outputs
        .iter()
        .find(|m| m.connector().as_deref() == position.connector.as_deref())
        .or_else(|| outputs.first())
    else {
        return;
    };
    let rect = Rect::from(monitor.geometry());
    let (x, y) = pixels(position, rect, size_on(window, rect));
    if let Err(error) = layer.position(window, monitor, x, y) {
        eprintln!("Pill position failed: {error}");
    }
}

pub(super) fn apply(window: &gtk::Window, layer: &LayerShell) {
    if DRAGGING.with(|dragging| dragging.get()) {
        return;
    }
    if let Some(position) = load() {
        place(window, layer, &position);
    }
}

fn button_at(widget: &gtk::Widget, x: f64, y: f64) -> bool {
    let mut picked = widget.pick(x, y, gtk::PickFlags::DEFAULT);
    while let Some(child) = picked {
        if child.is::<gtk::Button>() {
            return true;
        }
        picked = child.parent();
    }
    false
}

fn current_monitor(window: &gtk::Window) -> Option<gtk::gdk::Monitor> {
    window
        .surface()
        .and_then(|s| s.display().monitor_at_surface(&s))
        .or_else(|| monitors(window).into_iter().next())
}

/// Where the pill is right now, in margins from its monitor's top-left corner.
fn current_margins(window: &gtk::Window, monitor: &gtk::gdk::Monitor) -> (i32, i32) {
    let rect = Rect::from(monitor.geometry());
    let size = size_on(window, rect);
    match load().filter(|saved| saved.connector.as_deref() == monitor.connector().as_deref()) {
        Some(saved) => pixels(&saved, rect, size),
        None => (
            (rect.width - size.0) / 2,
            (rect.height - size.1 - 24).max(0),
        ),
    }
}

/// niri (measured) and sway (`seatop_down`) keep the surface origin of the press
/// for the whole implicit grab, so an offset is the pointer's distance from the
/// press however far the pill has moved. Others, such as KWin, report against the
/// surface's current position, so an offset is added to the position the
/// compositor has confirmed. Adding it to the last request would compound, and
/// using the wrong model throws the pill away from the pointer.
fn grab_freezes_origin() -> bool {
    std::env::var_os("NIRI_SOCKET").is_some() || std::env::var_os("SWAYSOCK").is_some()
}

/// Margins an offset asks for.
fn wanted(frozen: bool, start: (i32, i32), applied: (i32, i32), offset: (f64, f64)) -> (f64, f64) {
    let base = if frozen { start } else { applied };
    (f64::from(base.0) + offset.0, f64::from(base.1) + offset.1)
}

/// A press that has moved far enough to be a drag rather than a click.
struct Drag {
    monitor: gtk::gdk::Monitor,
    /// Margins when the drag began.
    start: (i32, i32),
    /// Margins the compositor has confirmed. Pointer events are relative to this.
    applied: (i32, i32),
    /// Margins sent in the last painted frame, confirmed by the next frame.
    committed: (i32, i32),
    /// Margins most recently asked for.
    requested: (i32, i32),
    /// Pill's unclamped top-left in global layout coordinates.
    global: (f64, f64),
}

impl Drag {
    /// GDK holds the next frame until the compositor's frame callback, which it
    /// sends after applying the previous commit, so each frame confirms the last.
    fn next_frame(&mut self) {
        self.applied = self.committed;
        self.committed = self.requested;
    }
}

/// Press-and-drag moves the real layer surface by updating its margins. The
/// press's implicit pointer grab keeps events coming even outside the surface.
/// What an offset means depends on the compositor; see `grab_freezes_origin`.
/// Nothing here takes the
/// keyboard, offers data or opens another window, so focus, the clipboard and
/// the recording are untouched. A layer surface can't change output without a
/// remap, which would end the grab, so the switch happens on release.
pub(super) fn attach(window: &gtk::Window, body: &gtk::Box, layer: Rc<LayerShell>) {
    let press = Rc::new(std::cell::Cell::new((0.0, 0.0)));
    let drag = Rc::new(RefCell::new(None::<Drag>));
    let gesture = gtk::GestureDrag::new();
    gesture.set_button(gtk::gdk::BUTTON_PRIMARY);
    gesture.connect_drag_begin({
        let body = body.clone();
        let press = press.clone();
        move |gesture, x, y| {
            if button_at(body.upcast_ref(), x, y) {
                gesture.set_state(gtk::EventSequenceState::Denied);
                return;
            }
            press.set((x, y));
        }
    });
    gesture.connect_drag_update({
        let window = window.clone();
        let body = body.clone();
        let layer = layer.clone();
        let state = drag.clone();
        let frozen = grab_freezes_origin();
        move |gesture, dx, dy| {
            let mut drag = state.borrow_mut();
            if drag.is_none() {
                if dx.hypot(dy) < DRAG_THRESHOLD {
                    return;
                }
                let Some(monitor) = current_monitor(&window) else {
                    return;
                };
                let margins = current_margins(&window, &monitor);
                let rect = monitor.geometry();
                *drag = Some(Drag {
                    global: (
                        f64::from(rect.x() + margins.0),
                        f64::from(rect.y() + margins.1),
                    ),
                    monitor,
                    start: margins,
                    applied: margins,
                    committed: margins,
                    requested: margins,
                });
                DRAGGING.with(|dragging| dragging.set(true));
                gesture.set_state(gtk::EventSequenceState::Claimed);
                body.set_cursor_from_name(Some("grabbing"));
                body.add_tick_callback({
                    let state = state.clone();
                    move |_, _| match state.borrow_mut().as_mut() {
                        Some(drag) => {
                            drag.next_frame();
                            gtk::glib::ControlFlow::Continue
                        }
                        None => gtk::glib::ControlFlow::Break,
                    }
                });
                if let Err(error) = layer.margins(&window, margins.0, margins.1) {
                    eprintln!("Pill move failed: {error}");
                }
            }
            let Some(drag) = drag.as_mut() else {
                return;
            };
            let rect = Rect::from(drag.monitor.geometry());
            let wanted = wanted(frozen, drag.start, drag.applied, (dx, dy));
            drag.global = (f64::from(rect.x) + wanted.0, f64::from(rect.y) + wanted.1);
            let margins = clamped(rect, size_on(&window, rect), wanted);
            if margins != drag.requested {
                drag.requested = margins;
                if let Err(error) = layer.margins(&window, margins.0, margins.1) {
                    eprintln!("Pill move failed: {error}");
                }
            }
        }
    });
    let finish = {
        let window = window.clone();
        let body = body.clone();
        let layer = layer.clone();
        let press = press.clone();
        let drag = drag.clone();
        move || {
            let Some(drag) = drag.borrow_mut().take() else {
                return;
            };
            DRAGGING.with(|dragging| dragging.set(false));
            body.set_cursor_from_name(None);
            let outputs = monitors(&window);
            let rects: Vec<Rect> = outputs
                .iter()
                .map(|monitor| Rect::from(monitor.geometry()))
                .collect();
            let (px, py) = press.get();
            let pointer = (drag.global.0 + px, drag.global.1 + py);
            let target = monitor_under(&rects, pointer)
                .and_then(|index| outputs.get(index).cloned())
                .filter(gtk::gdk::Monitor::is_valid)
                .unwrap_or_else(|| drag.monitor.clone());
            let rect = Rect::from(target.geometry());
            let size = size_on(&window, rect);
            let margins = if target == drag.monitor {
                drag.requested
            } else {
                clamped(
                    rect,
                    size,
                    (
                        drag.global.0 - f64::from(rect.x),
                        drag.global.1 - f64::from(rect.y),
                    ),
                )
            };
            let position = position_for(
                target.connector().map(|v| v.to_string()),
                rect,
                size,
                margins,
            );
            save(&position);
            place(&window, &layer, &position);
        }
    };
    gesture.connect_drag_end({
        let finish = finish.clone();
        move |_, _, _| finish()
    });
    // The pill can hide mid-drag when a recording ends; never stay stuck dragging.
    body.connect_unmap(move |_| finish());
    body.add_controller(gesture);
    let reset = gtk::GestureClick::new();
    reset.set_button(gtk::gdk::BUTTON_PRIMARY);
    reset.connect_pressed({
        let body = body.clone();
        let window = window.clone();
        let layer = layer.clone();
        move |gesture, count, x, y| {
            if count != 2 || button_at(body.upcast_ref(), x, y) {
                return;
            }
            if let Some(monitor) = current_monitor(&window) {
                let rect = Rect::from(monitor.geometry());
                let size = size_on(&window, rect);
                let position = position_for(
                    monitor.connector().map(|v| v.to_string()),
                    rect,
                    size,
                    (
                        (rect.width - size.0) / 2,
                        (rect.height - size.1 - 24).max(0),
                    ),
                );
                place(&window, &layer, &position);
                save(&position);
                gesture.set_state(gtk::EventSequenceState::Claimed);
            }
        }
    });
    body.add_controller(reset);
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIRST: Rect = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    const SECOND: Rect = Rect {
        x: 1920,
        y: 0,
        width: 2560,
        height: 1440,
    };

    #[test]
    fn a_drag_follows_the_pointer_and_stays_on_the_monitor() {
        assert_eq!(clamped(FIRST, (300, 48), (500.0, 400.0)), (500, 400));
        assert_eq!(clamped(FIRST, (300, 48), (-40.0, 5000.0)), (0, 1032));
        assert_eq!(clamped(FIRST, (300, 48), (9000.0, -1.0)), (1620, 0));
    }

    #[test]
    fn saved_position_round_trips_to_the_same_pixels() {
        for (left, top) in [(0, 0), (1620, 1032), (811, 377)] {
            let position = position_for(Some("DP-1".into()), FIRST, (300, 48), (left, top));
            assert_eq!(pixels(&position, FIRST, (300, 48)), (left, top));
        }
    }

    #[test]
    fn dropping_past_the_edge_lands_under_the_pointer_on_the_other_monitor() {
        // Pill clamped at the first monitor's edge, pointer 600 px into the second.
        let target = monitor_under(&[FIRST, SECOND], (2520.0, 500.0));
        assert_eq!(target, Some(1));
        assert_eq!(
            clamped(SECOND, (300, 48), (2400.0 - 1920.0, 480.0)),
            (480, 480)
        );
    }

    #[test]
    fn a_frozen_grab_measures_from_the_press_and_a_live_one_from_the_surface() {
        // niri trace: pill started at 490, has reached 655, pointer is 30 from the press.
        assert_eq!(
            wanted(true, (490, 728), (655, 508), (30.0, -40.0)),
            (520.0, 688.0)
        );
        // Live origin: the same pointer position reports as 0 once the move landed.
        assert_eq!(
            wanted(false, (490, 728), (520, 688), (0.0, 0.0)),
            (520.0, 688.0)
        );
    }

    #[test]
    fn a_pointer_in_a_gap_between_monitors_keeps_the_current_one() {
        assert_eq!(monitor_under(&[FIRST, SECOND], (-5.0, -5.0)), None);
    }
}
