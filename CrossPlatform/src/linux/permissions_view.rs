use gtk4::{self as gtk, prelude::*};
use std::{rc::Rc, time::Duration};

pub fn status() -> serde_json::Value {
    let keyboard = match super::sketch::keys::Keyboards::open() {
        Ok(keys) if keys.unreadable() == 0 => "available",
        Ok(_) => "partial",
        Err(super::sketch::keys::Access::Denied) => "permission_required",
        Err(_) => "no_keyboard",
    };
    let shortcuts = super::SHORTCUT_STATUS
        .read()
        .map(|value| value.clone())
        .unwrap_or_default();
    serde_json::json!({"ok":true,"keyboard":keyboard,"shortcuts":shortcuts,"microphone":"check_in_audio_preview","screen":"explicit_capture_check_required"})
}

pub fn show(parent: &gtk::ApplicationWindow, settings: Rc<dyn Fn(&str)>) {
    if let Some(window) = parent.application().and_then(|app| {
        app.windows()
            .into_iter()
            .find(|w| w.title().as_deref() == Some("Permissions"))
    }) {
        window.present();
        return;
    }
    let Some(application) = parent.application() else {
        return;
    };
    let window = gtk::Window::builder()
        .title("Permissions")
        .transient_for(parent)
        .application(&application)
        .default_width(480)
        .resizable(false)
        .build();
    let layout = gtk::Box::new(gtk::Orientation::Vertical, 14);
    layout.set_margin_top(20);
    layout.set_margin_bottom(20);
    layout.set_margin_start(20);
    layout.set_margin_end(20);
    let row = |name: &str, label: &str, action: &str| {
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let text = gtk::Label::new(Some(label));
        text.set_xalign(0.0);
        text.set_wrap(true);
        text.set_hexpand(true);
        super::menu::set_accessible_label(text.upcast_ref(), name);
        let button = gtk::Button::with_label(action);
        line.append(&text);
        line.append(&button);
        layout.append(&line);
        (text, button)
    };
    let (_, microphone) = row(
        "Microphone access",
        "Microphone · check in Audio preview",
        "Check microphone",
    );
    let (keyboard, setup) = row("Keyboard access", "Keyboard access", "Copy keyboard setup");
    let (shortcut, edit) = row("Shortcut access", "Shortcuts", "Shortcuts…");
    let (screen, capture) = row(
        "Screen capture access",
        "Screen capture · not checked",
        "Check capture",
    );
    microphone.connect_clicked({
        let settings = settings.clone();
        move |_| settings("audio")
    });
    edit.connect_clicked(move |_| settings("shortcuts"));
    setup.connect_clicked(|button| {
        button
            .clipboard()
            .set_text(super::sketch::keys::SETUP_COMMAND);
        button.set_label("Copied");
    });
    capture.connect_clicked({
        let screen = screen.clone();
        move |button| {
            if super::QUIT_BUSY.load(std::sync::atomic::Ordering::Acquire) {
                screen.set_text("Finish recording or processing before checking capture");
                return;
            }
            let display = button.display();
            let monitors: Vec<_> = display
                .monitors()
                .iter::<gtk::gdk::Monitor>()
                .filter_map(Result::ok)
                .collect();
            let Some(first) = monitors.first() else {
                screen.set_text("No display available");
                return;
            };
            let rect = first.geometry();
            let min_x = monitors.iter().map(|m| m.geometry().x()).min().unwrap_or(0);
            let min_y = monitors.iter().map(|m| m.geometry().y()).min().unwrap_or(0);
            let max_x = monitors
                .iter()
                .map(|m| m.geometry().x() + m.geometry().width())
                .max()
                .unwrap_or(0);
            let max_y = monitors
                .iter()
                .map(|m| m.geometry().y() + m.geometry().height())
                .max()
                .unwrap_or(0);
            let target = super::sketch::capture::Target {
                connector: first.connector().map(|s| s.to_string()),
                monitor: (rect.x(), rect.y(), rect.width(), rect.height()),
                desktop: (min_x, min_y, max_x - min_x, max_y - min_y),
            };
            let (send, receive) = std::sync::mpsc::channel();
            button.set_sensitive(false);
            screen.set_text("Screen capture · checking…");
            std::thread::spawn(move || {
                let _ = send.send(super::sketch::capture::capture(&target).map(|_| ()));
            });
            let button = button.downgrade();
            let screen = screen.downgrade();
            gtk::glib::timeout_add_local(Duration::from_millis(50), move || {
                let (Some(button), Some(screen)) = (button.upgrade(), screen.upgrade()) else {
                    return gtk::glib::ControlFlow::Break;
                };
                match receive.try_recv() {
                    Ok(result) => {
                        button.set_sensitive(true);
                        screen.set_text(
                            &result
                                .map(|_| "Screen capture · available".to_owned())
                                .unwrap_or_else(|error| format!("Screen capture · {error}")),
                        );
                        gtk::glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => gtk::glib::ControlFlow::Continue,
                    Err(_) => {
                        button.set_sensitive(true);
                        screen.set_text("Screen capture · check failed");
                        gtk::glib::ControlFlow::Break
                    }
                }
            });
        }
    });
    let refresh = move || {
        let state = status();
        keyboard.set_text(&format!(
            "Keyboard · {}",
            state["keyboard"]
                .as_str()
                .unwrap_or("unknown")
                .replace('_', " ")
        ));
        shortcut.set_text(if state["shortcuts"]["ok"].as_bool() == Some(true) {
            "Shortcuts · registered"
        } else {
            "Shortcuts · desktop binding required"
        });
    };
    refresh();
    let weak = window.downgrade();
    gtk::glib::timeout_add_local(Duration::from_secs(1), move || {
        if weak.upgrade().is_some_and(|w| w.is_visible()) {
            refresh();
            gtk::glib::ControlFlow::Continue
        } else {
            gtk::glib::ControlFlow::Break
        }
    });
    let done = gtk::Button::with_label("Done");
    done.set_halign(gtk::Align::End);
    done.connect_clicked({
        let window = window.downgrade();
        move |_| {
            if let Some(window) = window.upgrade() {
                window.close();
            }
        }
    });
    layout.append(&done);
    window.set_child(Some(&layout));
    window.present();
}
