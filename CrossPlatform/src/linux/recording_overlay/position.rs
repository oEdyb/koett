use super::stage::{self, PAD_TOP, PAD_X, RESTING, Stage};
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

/// Where the surface goes for a `size` pill at `pill` on a monitor, and where
/// the pill then sits inside it: the surface stays on the monitor, so its light
/// is cut only by the screen edge and the pill never moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Placement {
    surface: (i32, i32),
    inset: (i32, i32),
}

fn placement(rect: Rect, pill: (i32, i32), size: (i32, i32)) -> Placement {
    let around = stage::surface(size.0);
    let surface = (
        (pill.0 - PAD_X).clamp(0, (rect.width - around.0).max(0)),
        (pill.1 - PAD_TOP).clamp(0, (rect.height - around.1).max(0)),
    );
    Placement {
        surface,
        inset: (pill.0 - surface.0, pill.1 - surface.1),
    }
}

/// The pill's resting place on a monitor: centred, 24 px above the bottom.
fn resting(rect: Rect, size: (i32, i32)) -> (i32, i32) {
    (
        (rect.width - size.0) / 2,
        (rect.height - size.1 - 24).max(0),
    )
}

fn saved_monitor(window: &gtk::Window, position: &Position) -> Option<gtk::gdk::Monitor> {
    let outputs = monitors(window);
    outputs
        .iter()
        .find(|m| m.connector().as_deref() == position.connector.as_deref())
        .or_else(|| outputs.first())
        .cloned()
}

fn place(window: &gtk::Window, layer: &LayerShell, stage: &Stage, position: &Position) {
    let Some(monitor) = saved_monitor(window, position) else {
        return;
    };
    let rect = Rect::from(monitor.geometry());
    let size = stage.pill();
    let at = placement(rect, pixels(position, rect, size), size);
    if let Err(error) = layer.position(window, &monitor, at.surface.0, at.surface.1) {
        eprintln!("Pill position failed: {error}");
    }
    stage.set_inset(at.inset);
}

/// A pill with no saved place rides the bottom anchor `configure` set.
pub(super) fn apply(window: &gtk::Window, layer: &LayerShell, stage: &Stage) {
    if DRAGGING.with(|dragging| dragging.get()) {
        return;
    }
    match load() {
        Some(position) => place(window, layer, stage, &position),
        None => stage.set_inset(RESTING),
    }
}

/// Where the surface goes in desktop coordinates, for desktops that move the
/// pill themselves; the pill's inset follows. `monitor` is used when nothing
/// is saved.
pub(super) fn origin(
    window: &gtk::Window,
    monitor: Option<&gtk::gdk::Monitor>,
    stage: &Stage,
) -> Option<(i32, i32)> {
    let size = stage.pill();
    let (monitor, pill) = match load() {
        Some(position) => {
            let monitor = saved_monitor(window, &position)?;
            let rect = Rect::from(monitor.geometry());
            (rect, pixels(&position, rect, size))
        }
        None => {
            let monitor = monitor
                .cloned()
                .or_else(|| monitors(window).into_iter().next())?;
            let rect = Rect::from(monitor.geometry());
            (rect, resting(rect, size))
        }
    };
    let at = placement(monitor, pill, size);
    stage.set_inset(at.inset);
    Some((monitor.x + at.surface.0, monitor.y + at.surface.1))
}

/// Remembers where a desktop's own drag left the surface.
fn save_origin(window: &gtk::Window, stage: &Stage, (x, y): (i32, i32)) {
    let (x, y) = (x + stage.inset().0, y + stage.inset().1);
    let size = stage.pill();
    let outputs = monitors(window);
    let rects: Vec<Rect> = outputs.iter().map(|m| Rect::from(m.geometry())).collect();
    let center = (
        f64::from(x) + f64::from(size.0) / 2.0,
        f64::from(y) + f64::from(size.1) / 2.0,
    );
    let Some(index) = monitor_under(&rects, center).or((!rects.is_empty()).then_some(0)) else {
        return;
    };
    let rect = rects[index];
    save(&position_for(
        outputs[index]
            .connector()
            .map(|connector| connector.to_string()),
        rect,
        size,
        clamped(rect, size, (f64::from(x - rect.x), f64::from(y - rect.y))),
    ));
}

/// GNOME and X11: a press outside the buttons starts a drag that the desktop side
/// follows until the button comes up, then the place is saved.
pub(super) fn attach_follow(window: &gtk::Window, stage: Rc<Stage>, gnome: bool) {
    let press = gtk::GestureClick::new();
    press.set_button(gtk::gdk::BUTTON_PRIMARY);
    press.connect_pressed({
        let window = window.clone();
        let stage = stage.clone();
        move |_, _, x, y| {
            if button_at(stage.root().upcast_ref(), x, y) {
                return;
            }
            let finished = {
                let window = window.clone();
                let stage = stage.clone();
                move |origin: Option<(i32, i32)>| {
                    if let Some(origin) = origin {
                        save_origin(&window, &stage, origin);
                    }
                }
            };
            if gnome {
                super::super::gnome_pill::follow_pointer(finished);
            } else {
                super::super::x11_surface::follow_pointer(&window, finished);
            }
        }
    });
    window.add_controller(press);
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

/// Where a `size` pill is right now, from its monitor's top-left corner.
fn current_pill(monitor: &gtk::gdk::Monitor, size: (i32, i32)) -> (i32, i32) {
    let rect = Rect::from(monitor.geometry());
    match load().filter(|saved| saved.connector.as_deref() == monitor.connector().as_deref()) {
        Some(saved) => pixels(&saved, rect, size),
        None => resting(rect, size),
    }
}

/// Where a press on the surface holds the pill, from the pill's top-left.
fn grip((x, y): (f64, f64), inset: (i32, i32)) -> (f64, f64) {
    (x - f64::from(inset.0), y - f64::from(inset.1))
}

/// The pointer, for a pill at `pill` held at `grip`.
fn pointer_at(pill: (f64, f64), grip: (f64, f64)) -> (f64, f64) {
    (pill.0 + grip.0, pill.1 + grip.1)
}

/// The live copy that follows the pointer: a picture of the real surface,
/// shifted so its pill lands where the copy's own placement puts it.
struct Carrier {
    window: gtk::Window,
    content: gtk::Fixed,
    picture: gtk::Picture,
}

/// Shows the live copy with its pill at `pill` on `monitor`. Layer windows take no
/// focus, and this one takes no input either, so the grab stays with the real pill
/// and a click just after the drop reaches whatever is under the lingering copy.
fn carry(
    carrier: &mut Option<Carrier>,
    window: &gtk::Window,
    shell: &gtk::Box,
    stage: &Stage,
    layer: &LayerShell,
    monitor: &gtk::gdk::Monitor,
    pill: (i32, i32),
) {
    if carrier.is_none() {
        let around = stage.surface();
        let copy = gtk::Window::builder()
            .title("Koett recording")
            .decorated(false)
            .resizable(false)
            .default_width(around.0)
            .default_height(around.1)
            .build();
        copy.connect_realize(|copy| {
            if let Some(surface) = copy.surface() {
                surface.set_input_region(Some(&gtk::cairo::Region::create()));
            }
        });
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
        let picture = gtk::Picture::for_paintable(&gtk::WidgetPaintable::new(Some(stage.root())));
        picture.set_can_shrink(false);
        let content = gtk::Fixed::new();
        content.set_size_request(around.0, around.1);
        content.set_overflow(gtk::Overflow::Hidden);
        content.put(&picture, 0.0, 0.0);
        copy.set_child(Some(&content));
        // Hide the real pill only once the copy is up, so it never blinks away.
        copy.connect_map({
            let shell = shell.clone();
            move |_| shell.set_opacity(HIDDEN)
        });
        *carrier = Some(Carrier {
            window: copy,
            content,
            picture,
        });
    }
    if let Some(carrier) = carrier {
        let at = placement(Rect::from(monitor.geometry()), pill, stage.pill());
        let real = stage.inset();
        carrier.content.move_(
            &carrier.picture,
            f64::from(at.inset.0 - real.0),
            f64::from(at.inset.1 - real.1),
        );
        if let Err(error) = layer.position(&carrier.window, monitor, at.surface.0, at.surface.1) {
            eprintln!("Pill copy move failed: {error}");
        }
        carrier.window.present();
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
    carrier: Option<Carrier>,
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
    stage: Rc<Stage>,
    layer: Rc<LayerShell>,
) {
    let body = stage.root().clone();
    // Where the press holds the pill, from the pill's top-left (not the surface's).
    let press = Rc::new(std::cell::Cell::new((0.0, 0.0)));
    let drag = Rc::new(RefCell::new(None::<Drag>));
    let gesture = gtk::GestureDrag::new();
    gesture.set_button(gtk::gdk::BUTTON_PRIMARY);
    gesture.connect_drag_begin({
        let body = body.clone();
        let press = press.clone();
        let stage = stage.clone();
        move |gesture, x, y| {
            if button_at(body.upcast_ref(), x, y) {
                gesture.set_state(gtk::EventSequenceState::Denied);
                return;
            }
            press.set(grip((x, y), stage.inset()));
        }
    });
    gesture.connect_drag_update({
        let window = window.clone();
        let body = body.clone();
        let layer = layer.clone();
        let press = press.clone();
        let shell = shell.clone();
        let stage = stage.clone();
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
                let margins = current_pill(&monitor, stage.pill());
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
            let pointer = pointer_at(drag.global, press.get());
            let target = target_monitor(&window, pointer).unwrap_or_else(|| drag.monitor.clone());
            let rect = Rect::from(target.geometry());
            let pill = clamped(
                rect,
                stage.pill(),
                (
                    drag.global.0 - f64::from(rect.x),
                    drag.global.1 - f64::from(rect.y),
                ),
            );
            carry(
                &mut drag.carrier,
                &window,
                &shell,
                &stage,
                &layer,
                &target,
                pill,
            );
        }
    });
    let finish = {
        let window = window.clone();
        let shell = shell.clone();
        let body = body.clone();
        let stage = stage.clone();
        let layer = layer.clone();
        let press = press.clone();
        let drag = drag.clone();
        move || {
            let Some(drag) = drag.borrow_mut().take() else {
                return;
            };
            DRAGGING.with(|dragging| dragging.set(false));
            body.set_cursor_from_name(None);
            let pointer = pointer_at(drag.global, press.get());
            let target = target_monitor(&window, pointer).unwrap_or_else(|| drag.monitor.clone());
            let rect = Rect::from(target.geometry());
            let size = stage.pill();
            let pill = clamped(
                rect,
                size,
                (
                    drag.global.0 - f64::from(rect.x),
                    drag.global.1 - f64::from(rect.y),
                ),
            );
            let position =
                position_for(target.connector().map(|v| v.to_string()), rect, size, pill);
            save(&position);
            place(&window, &layer, &stage, &position);
            shell.set_opacity(1.0);
            // Hold the copy over the real pill's remap so it doesn't blink.
            if let Some(copy) = drag.carrier {
                gtk::glib::timeout_add_local_once(CARRIER_LINGER, move || copy.window.destroy());
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
                let size = stage.pill();
                let position = position_for(
                    monitor.connector().map(|v| v.to_string()),
                    rect,
                    size,
                    resting(rect, size),
                );
                place(&window, &layer, &stage, &position);
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

    const PILL: (i32, i32) = (WIDTH, ROW);
    /// A pill widened for an h:mm:ss clock.
    const WIDE: (i32, i32) = (WIDTH + 21, ROW);

    #[test]
    fn the_surface_stays_on_screen_and_the_pill_never_moves() {
        for size in [PILL, WIDE] {
            let around = stage::surface(size.0);
            let right = FIRST.width - size.0;
            for pill in [
                (0, 0),
                (right, 1032),
                (811, 377),
                resting(FIRST, size),
                (20, 1000),
            ] {
                let at = placement(FIRST, pill, size);
                assert_eq!((at.surface.0 + at.inset.0, at.surface.1 + at.inset.1), pill);
                assert!(at.surface.0 >= 0 && at.surface.0 + around.0 <= FIRST.width);
                assert!(at.surface.1 >= 0 && at.surface.1 + around.1 <= FIRST.height);
                assert!((0..=around.0 - size.0).contains(&at.inset.0));
                assert!((0..=around.1 - ROW).contains(&at.inset.1));
            }
        }
        assert_eq!(
            placement(FIRST, (811, 377), PILL).inset,
            (PAD_X, PAD_TOP),
            "room on every side: the full light"
        );
    }

    #[test]
    fn the_resting_pill_matches_the_bottom_anchored_surface() {
        assert_eq!(resting(FIRST, PILL), (810, 1008));
        assert_eq!(placement(FIRST, resting(FIRST, PILL), PILL).inset, RESTING);
        assert_eq!(placement(FIRST, resting(FIRST, WIDE), WIDE).inset, RESTING);
    }

    #[test]
    fn a_wider_pill_keeps_its_saved_place_on_screen() {
        let saved = position_for(None, FIRST, PILL, (FIRST.width - PILL.0, 500));
        let (left, _) = pixels(&saved, FIRST, WIDE);
        assert_eq!(
            left + WIDE.0,
            FIRST.width,
            "grows leftwards at the right edge"
        );
        let centred = position_for(None, FIRST, PILL, resting(FIRST, PILL));
        let (left, top) = pixels(&centred, FIRST, WIDE);
        let resting = resting(FIRST, WIDE);
        assert!(
            (left - resting.0).abs() <= 1 && top == resting.1,
            "stays centred"
        );
    }

    #[test]
    fn a_drag_near_the_right_edge_stays_on_its_monitor() {
        // The pill against the first monitor's right edge, held 20 px from it.
        for size in [PILL, WIDE] {
            let pill = (FIRST.width - size.0, 500);
            let at = placement(FIRST, pill, size);
            assert!(
                at.inset.0 > PAD_X,
                "the surface is clamped, so the inset grows"
            );
            let press = (
                f64::from(at.inset.0 + size.0 - 20),
                f64::from(at.inset.1 + 24),
            );
            let held = grip(press, at.inset);
            let pointer = pointer_at((f64::from(pill.0), f64::from(pill.1)), held);
            assert_eq!(pointer, (f64::from(FIRST.width - 20), 524.0));
            assert_eq!(monitor_under(&[FIRST, SECOND], pointer), Some(0));
        }
    }

    #[test]
    fn a_pointer_in_a_gap_between_monitors_keeps_the_current_one() {
        assert_eq!(monitor_under(&[FIRST, SECOND], (-5.0, -5.0)), None);
    }
}
