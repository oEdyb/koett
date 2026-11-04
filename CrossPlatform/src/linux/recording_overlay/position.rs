use super::*;

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
struct Position {
    connector: Option<String>,
    x: u32,
    y: u32,
}

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

fn place(window: &gtk::Window, layer: &LayerShell, position: &Position) {
    let outputs = monitors(window);
    let Some(monitor) = outputs
        .iter()
        .find(|m| m.connector().as_deref() == position.connector.as_deref())
        .or_else(|| outputs.first())
    else {
        return;
    };
    let rect = monitor.geometry();
    let width = window.width().max(WIDTH).min(rect.width());
    let height = window
        .height()
        .max(window.default_height())
        .max(ROW)
        .min(rect.height());
    let x = (i64::from((rect.width() - width).max(0)) * i64::from(position.x.min(10000)) / 10000)
        as i32;
    let y = (i64::from((rect.height() - height).max(0)) * i64::from(position.y.min(10000)) / 10000)
        as i32;
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

pub(super) fn attach(window: &gtk::Window, body: &gtk::Box, layer: Rc<LayerShell>) {
    let active = Rc::new(RefCell::new(Vec::<gtk::Window>::new()));
    let offset = Rc::new(std::cell::Cell::new((0.0, 0.0)));
    let token = uuid::Uuid::new_v4().to_string();
    let source = gtk::DragSource::new();
    source.set_actions(gtk::gdk::DragAction::MOVE);
    source.connect_prepare({
        let body = body.clone();
        let offset = offset.clone();
        let token = token.clone();
        move |source, x, y| {
            if button_at(body.upcast_ref(), x, y) {
                return None;
            }
            offset.set((x, y));
            source.set_icon(
                Some(&gtk::WidgetPaintable::new(Some(&body))),
                x.round() as i32,
                y.round() as i32,
            );
            Some(gtk::gdk::ContentProvider::for_value(&token.to_value()))
        }
    });
    source.connect_drag_begin({
        let window = window.clone();
        let layer = layer.clone();
        let active = active.clone();
        let offset = offset.clone();
        move |_, _| {
            DRAGGING.with(|dragging| dragging.set(true));
            let mut targets = active.borrow_mut();
            for monitor in monitors(&window) {
                let target = gtk::Window::builder()
                    .application(&window.application().unwrap())
                    .title("Koett pill move target")
                    .decorated(false)
                    .build();
                target.add_css_class("koett-pill-move-target");
                target.set_focusable(false);
                if let Err(error) = layer.cover(&target, &monitor, false) {
                    eprintln!("Pill move target failed: {error}");
                    continue;
                }
                let area = gtk::DrawingArea::new();
                let drop = gtk::DropTarget::new(String::static_type(), gtk::gdk::DragAction::MOVE);
                drop.connect_drop({
                    let window = window.clone();
                    let monitor = monitor.clone();
                    let token = token.clone();
                    let offset = offset.clone();
                    move |_, value, x, y| {
                        if value.get::<String>().ok().as_deref() != Some(&token)
                            || !monitor.is_valid()
                        {
                            return false;
                        }
                        let rect = monitor.geometry();
                        let (dx, dy) = offset.get();
                        let max_x = (rect.width() - window.width().max(WIDTH)).max(0);
                        let max_y = (rect.height()
                            - window.height().max(window.default_height()).max(ROW))
                        .max(0);
                        let left = (x - dx).round().clamp(0.0, f64::from(max_x));
                        let top = (y - dy).round().clamp(0.0, f64::from(max_y));
                        let position = Position {
                            connector: monitor.connector().map(|v| v.to_string()),
                            x: if max_x == 0 {
                                0
                            } else {
                                (left * 10000.0 / f64::from(max_x)).round() as u32
                            },
                            y: if max_y == 0 {
                                0
                            } else {
                                (top * 10000.0 / f64::from(max_y)).round() as u32
                            },
                        };
                        save(&position);
                        true
                    }
                });
                area.add_controller(drop);
                target.set_child(Some(&area));
                target.present();
                targets.push(target);
            }
            window.set_opacity(0.0);
        }
    });
    source.connect_drag_end({
        let window = window.clone();
        let active = active.clone();
        let layer = layer.clone();
        move |_, _, _| {
            for target in active.borrow_mut().drain(..) {
                target.destroy();
            }
            DRAGGING.with(|dragging| dragging.set(false));
            apply(&window, &layer);
            window.set_opacity(1.0);
        }
    });
    body.add_controller(source);
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
            let monitor = window
                .surface()
                .and_then(|s| s.display().monitor_at_surface(&s))
                .or_else(|| monitors(&window).into_iter().next());
            if let Some(monitor) = monitor {
                let rect = monitor.geometry();
                let max_y =
                    (rect.height() - window.height().max(window.default_height()).max(ROW)).max(0);
                let position = Position {
                    connector: monitor.connector().map(|v| v.to_string()),
                    x: 5000,
                    y: if max_y == 0 {
                        0
                    } else {
                        ((max_y - 24).max(0) as u32) * 10000 / (max_y as u32)
                    },
                };
                place(&window, &layer, &position);
                save(&position);
                gesture.set_state(gtk::EventSequenceState::Claimed);
            }
        }
    });
    body.add_controller(reset);
}
