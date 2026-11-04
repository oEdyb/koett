use super::*;

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
struct Position {
    connector: Option<String>,
    x: u32,
    y: u32,
}

const DRAG_THRESHOLD: f64 = 4.0;
/// Invisible, yet not 0: a window with nothing to draw gets no render node, so GTK
/// skips the frame and the compositor keeps showing the old pill.
const HIDDEN: f64 = 1.0 / 255.0;
const CARRIER_LINGER: std::time::Duration = std::time::Duration::from_millis(120);

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

fn target_monitor(window: &gtk::Window, pointer: (f64, f64)) -> Option<gtk::gdk::Monitor> {
    let outputs = monitors(window);
    let rects: Vec<Rect> = outputs
        .iter()
        .map(|monitor| Rect::from(monitor.geometry()))
        .collect();
    outputs.into_iter().nth(monitor_under(&rects, pointer)?)
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

/// Shows the live copy at `margins` on `monitor`. Layer windows take no focus, and
/// this one takes no input either, so the grab stays with the real pill.
fn carry(
    carrier: &mut Option<gtk::Window>,
    window: &gtk::Window,
    shell: &gtk::Box,
    body: &gtk::Box,
    layer: &LayerShell,
    monitor: &gtk::gdk::Monitor,
    margins: (i32, i32),
) {
    if carrier.is_none() {
        let copy = gtk::Window::builder()
            .title("Koett recording")
            .decorated(false)
            .resizable(false)
            .default_width(window.width())
            .default_height(window.height())
            .build();
        if let Some(application) = window.application() {
            copy.set_application(Some(&application));
        }
        if let Err(error) = layer.configure(&copy) {
            eprintln!("Pill copy failed: {error}");
            copy.destroy();
            return;
        }
        copy.set_css_classes(
            &window
                .css_classes()
                .iter()
                .map(|class| class.as_str())
                .collect::<Vec<_>>(),
        );
        copy.set_focusable(false);
        copy.set_can_target(false);
        let picture = gtk::Picture::for_paintable(&gtk::WidgetPaintable::new(Some(body)));
        picture.set_can_shrink(false);
        copy.set_child(Some(&picture));
        // Hide the real pill only once the copy is up, so it never blinks away.
        copy.connect_map({
            let shell = shell.clone();
            move |_| shell.set_opacity(HIDDEN)
        });
        *carrier = Some(copy);
    }
    if let Some(copy) = carrier {
        if let Err(error) = layer.position(copy, monitor, margins.0, margins.1) {
            eprintln!("Pill copy move failed: {error}");
        }
        copy.present();
    }
}

/// A press that has moved far enough to be a drag rather than a click.
struct Drag {
    monitor: gtk::gdk::Monitor,
    /// Margins the real pill keeps for the whole drag.
    start: (i32, i32),
    /// Pill's unclamped top-left in global layout coordinates.
    global: (f64, f64),
    /// Live copy of the pill that follows the pointer.
    carrier: Option<gtk::Window>,
}

/// The press's implicit grab keeps events coming from the real pill wherever the
/// pointer goes. Moving that surface would change what the events measure, and
/// compositors differ: niri keeps the press's origin, KWin follows the surface a
/// variable time later. So the real pill stays put and invisible, its offsets stay
/// exact everywhere, and a live copy that takes no input follows the pointer onto
/// any monitor. A remap would also end the grab, so the real pill moves on release.
/// Nothing here takes the keyboard or offers data, so focus, the clipboard and the
/// recording are untouched.
pub(super) fn attach(
    window: &gtk::Window,
    shell: &gtk::Box,
    body: &gtk::Box,
    layer: Rc<LayerShell>,
) {
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
        let press = press.clone();
        let shell = shell.clone();
        let state = drag.clone();
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
                    carrier: None,
                });
                DRAGGING.with(|dragging| dragging.set(true));
                gesture.set_state(gtk::EventSequenceState::Claimed);
                body.set_cursor_from_name(Some("grabbing"));
            }
            let Some(drag) = drag.as_mut() else {
                return;
            };
            let origin = Rect::from(drag.monitor.geometry());
            drag.global = (
                f64::from(origin.x + drag.start.0) + dx,
                f64::from(origin.y + drag.start.1) + dy,
            );
            let (px, py) = press.get();
            let pointer = (drag.global.0 + px, drag.global.1 + py);
            let target = target_monitor(&window, pointer).unwrap_or_else(|| drag.monitor.clone());
            let rect = Rect::from(target.geometry());
            let margins = clamped(
                rect,
                size_on(&window, rect),
                (
                    drag.global.0 - f64::from(rect.x),
                    drag.global.1 - f64::from(rect.y),
                ),
            );
            carry(
                &mut drag.carrier,
                &window,
                &shell,
                &body,
                &layer,
                &target,
                margins,
            );
        }
    });
    let finish = {
        let window = window.clone();
        let shell = shell.clone();
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
            let (px, py) = press.get();
            let pointer = (drag.global.0 + px, drag.global.1 + py);
            let target = target_monitor(&window, pointer).unwrap_or_else(|| drag.monitor.clone());
            let rect = Rect::from(target.geometry());
            let size = size_on(&window, rect);
            let margins = clamped(
                rect,
                size,
                (
                    drag.global.0 - f64::from(rect.x),
                    drag.global.1 - f64::from(rect.y),
                ),
            );
            let position = position_for(
                target.connector().map(|v| v.to_string()),
                rect,
                size,
                margins,
            );
            save(&position);
            place(&window, &layer, &position);
            shell.set_opacity(1.0);
            // Hold the copy over the real pill's remap so it doesn't blink.
            if let Some(copy) = drag.carrier {
                gtk::glib::timeout_add_local_once(CARRIER_LINGER, move || copy.destroy());
            }
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
    fn a_pointer_in_a_gap_between_monitors_keeps_the_current_one() {
        assert_eq!(monitor_under(&[FIRST, SECOND], (-5.0, -5.0)), None);
    }
}
