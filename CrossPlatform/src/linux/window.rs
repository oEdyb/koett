use super::{TrayAction, history_view::HistoryView};
use crate::settings::Settings;
use gtk4::{self as gtk, prelude::*};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

pub enum Update {
    Status(String),
    Completed(String),
    Shortcut(String),
    Levels(crate::audio::AudioLevels),
}

pub fn run(actions: Sender<TrayAction>, updates: Receiver<Update>, visible: bool) {
    let application = gtk::Application::builder()
        .application_id(super::APP_ID)
        .build();
    let updates = RefCell::new(Some(updates));
    application.connect_activate(move |application| {
        if let Some(window) = application.active_window() {
            window.present();
            return;
        }
        if let Some(updates) = updates.borrow_mut().take() {
            build_window(application, actions.clone(), updates, visible);
        }
    });
    application.run_with_args::<&str>(&[]);
}

fn build_window(
    application: &gtk::Application,
    actions: Sender<TrayAction>,
    updates: Receiver<Update>,
    visible: bool,
) {
    let window = gtk::ApplicationWindow::builder()
        .application(application)
        .title("Koett")
        .default_width(960)
        .default_height(680)
        .build();
    let header = gtk::HeaderBar::new();
    let title = gtk::Label::new(Some("Koett"));
    title.add_css_class("title");
    header.set_title_widget(Some(&title));
    let record = gtk::Button::with_label("Record");
    record.add_css_class("koett-record");
    record.set_sensitive(false);
    header.pack_start(&record);
    let settings_button = gtk::Button::with_label("Settings");
    header.pack_end(&settings_button);
    window.set_titlebar(Some(&header));

    let layout = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let history = Rc::new(HistoryView::new(&window, actions.clone()));
    history.record_button.set_sensitive(false);
    layout.append(&history.root);
    let status_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    status_row.set_margin_top(8);
    status_row.set_margin_bottom(8);
    status_row.set_margin_start(16);
    status_row.set_margin_end(16);
    let status = gtk::Label::new(Some("Starting…"));
    status.set_xalign(0.0);
    status.set_hexpand(true);
    status.set_wrap(true);
    let shortcut = gtk::Label::new(None);
    shortcut.add_css_class("dim-label");
    if let Ok(settings) = Settings::load() {
        shortcut.set_text(&settings.shortcut);
    }
    let settings_saved: Rc<dyn Fn(&Settings)> = Rc::new({
        let shortcut = shortcut.clone();
        move |settings| {
            if std::env::var_os("WAYLAND_DISPLAY").is_none() {
                shortcut.set_text(&settings.shortcut);
            }
        }
    });
    status_row.append(&status);
    let meter = gtk::LevelBar::for_interval(0.0, 1.0);
    meter.set_size_request(72, -1);
    let meter_label = gtk::Label::new(Some("Microphone level"));
    meter_label.set_mnemonic_widget(Some(&meter));
    let meter_group = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    meter_group.append(&meter_label);
    meter_group.append(&meter);
    meter_group.set_visible(false);
    status_row.append(&meter_group);
    status_row.append(&shortcut);
    let recovery = gtk::Button::with_label("Copy last transcript");
    recovery.set_visible(false);
    let latest_completed = Rc::new(RefCell::new(None::<String>));
    recovery.connect_clicked({
        let latest = latest_completed.clone();
        move |button| {
            if let Some(text) = latest.borrow().as_ref() {
                button.clipboard().set_text(text);
                button.set_label("Copied");
            }
        }
    });
    status_row.append(&recovery);
    layout.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    layout.append(&status_row);
    window.set_child(Some(&layout));

    let css = gtk::CssProvider::new();
    css.load_from_data(
        ".transcript { font-size: 16px; }\n\
         .koett-record { background: #ac3d31; color: #ffffff; }\n\
         .koett-record:hover { background: #923126; }",
    );
    gtk::style_context_add_provider_for_display(
        &WidgetExt::display(&window),
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    record.connect_clicked({
        let actions = actions.clone();
        move |_| {
            let _ = actions.send(TrayAction::Toggle);
        }
    });
    settings_button.connect_clicked({
        let window = window.clone();
        let settings_saved = settings_saved.clone();
        move |_| super::settings_view::show(&window, settings_saved.clone())
    });
    window.connect_is_active_notify(|window| {
        super::output::WINDOW_ACTIVE
            .store(window.is_active(), std::sync::atomic::Ordering::Release);
    });
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed({
        let search = history.search.clone();
        move |_, key, _, modifiers| {
            if modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK) && key == gtk::gdk::Key::f {
                search.grab_focus();
                return gtk::glib::Propagation::Stop;
            }
            gtk::glib::Propagation::Proceed
        }
    });
    window.add_controller(keys);

    // A hidden window remains reachable through the launcher, tray and compositor commands.
    let hold = Rc::new(RefCell::new(Some(application.hold())));
    window.connect_close_request({
        let window = window.clone();
        move |_| {
            window.set_visible(false);
            gtk::glib::Propagation::Stop
        }
    });
    let quit = gtk::gio::SimpleAction::new("quit", None);
    quit.connect_activate({
        let actions = actions.clone();
        move |_, _| {
            let _ = actions.send(TrayAction::Quit);
        }
    });
    application.add_action(&quit);
    application.set_accels_for_action("app.quit", &["<Control>q"]);

    let mut recording_started = None::<Instant>;
    let mut state = String::from("Starting…");
    let mut notice = None::<String>;
    let mut audio_levels = None::<crate::audio::AudioLevels>;
    gtk::glib::timeout_add_local(Duration::from_millis(80), {
        let window = window.clone();
        move || {
            status_row.set_orientation(if window.width() < 650 {
                gtk::Orientation::Vertical
            } else {
                gtk::Orientation::Horizontal
            });
            for update in updates.try_iter() {
                match update {
                    Update::Levels(levels) => audio_levels = Some(levels),
                    Update::Shortcut(label) => shortcut.set_text(&label),
                    Update::Completed(text) => {
                        *latest_completed.borrow_mut() = Some(text);
                        recovery.set_visible(true);
                        recovery.set_label("Copy last transcript");
                    }
                    Update::Status(message) if message == "@quit" => {
                        hold.borrow_mut().take();
                        if let Some(app) = window.application() {
                            app.quit();
                        }
                        return gtk::glib::ControlFlow::Break;
                    }
                    Update::Status(message) if message == "@show" => {
                        history.refresh();
                        window.present();
                    }
                    Update::Status(message) if message == "@settings" => {
                        window.present();
                        super::settings_view::show(&window, settings_saved.clone());
                    }
                    Update::Status(message) if message == "@refresh" => history.refresh(),
                    Update::Status(message) if message.starts_with("@notice:") => {
                        notice = Some(message.trim_start_matches("@notice:").to_string());
                    }
                    Update::Status(message) => {
                        let recording = message.starts_with("Recording");
                        if recording && recording_started.is_none() {
                            recording_started = Some(Instant::now());
                        }
                        if !recording {
                            recording_started = None;
                        }
                        record.set_label(if recording {
                            "Stop"
                        } else if message.starts_with("Error:") {
                            "Retry"
                        } else {
                            "Record"
                        });
                        record.set_sensitive(
                            recording || message == "Ready" || message.starts_with("Error:"),
                        );
                        history
                            .record_button
                            .set_label(record.label().as_deref().unwrap_or("Record"));
                        history.record_button.set_sensitive(record.is_sensitive());
                        state = message;
                    }
                }
            }
            meter_group.set_visible(recording_started.is_some());
            if recording_started.is_some()
                && let Some(levels) = &audio_levels
            {
                meter.set_value(f64::from((levels.current().0 * 10.0).clamp(0.0, 1.0)));
            }
            status.set_text(
                &recording_started
                    .map(|started| {
                        format!(
                            "Recording · {}:{:02}",
                            started.elapsed().as_secs() / 60,
                            started.elapsed().as_secs() % 60
                        )
                    })
                    .unwrap_or_else(|| match &notice {
                        Some(notice) => format!("{state} · {notice}"),
                        None => state.clone(),
                    }),
            );
            gtk::glib::ControlFlow::Continue
        }
    });
    if visible {
        window.present();
    }
}
