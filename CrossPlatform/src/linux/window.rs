use super::{TrayAction, history_view::HistoryView};
use crate::settings::Settings;
use gtk4::{self as gtk, prelude::*};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

pub enum Update {
    PrepareAsk(Sender<()>),
    Status(String),
    Show(String),
    Settings(String),
    Completed(String),
    DeliveryFailed,
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
    window.add_css_class("koett-window");
    super::theme::install(&window);
    let header = gtk::HeaderBar::new();
    let title = gtk::Label::new(Some("Koett"));
    title.add_css_class("koett-title");
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    header.set_title_widget(Some(&title));
    let record = gtk::Button::with_label("Record");
    record.add_css_class("koett-record");
    record.set_sensitive(false);
    header.pack_start(&record);
    let ask_button = gtk::Button::with_label("Ask");
    let ask_actions = actions.clone();
    ask_button.connect_clicked(move |_| {
        let _ = ask_actions.send(TrayAction::Ask);
    });
    header.pack_start(&ask_button);
    let settings_button = gtk::Button::from_icon_name("preferences-system-symbolic");
    settings_button.set_tooltip_text(Some("Settings"));
    super::menu::set_accessible_label(settings_button.upcast_ref(), "Settings");
    super::menu_preferences::attach(application, actions.clone());
    header.pack_end(&super::menu::button());
    header.pack_end(&settings_button);
    window.set_titlebar(Some(&header));

    let formatting = Rc::new(super::formatting_view::FormattingView::new(actions.clone()));
    formatting
        .button
        .set_icon_name("format-text-italic-symbolic");
    formatting.button.set_tooltip_text(Some("Formatting"));
    super::menu::set_accessible_label(formatting.button.upcast_ref(), "Formatting");
    header.pack_end(&formatting.button);
    let formatting_action = gtk::gio::SimpleAction::new("formatting", None);
    formatting_action.connect_activate({
        let formatting = formatting.clone();
        move |_, _| formatting.button.popup()
    });
    application.add_action(&formatting_action);
    let models = Rc::new(super::models_view::ModelsView::new(
        &window,
        actions.clone(),
    ));
    let model_action = gtk::gio::SimpleAction::new("models", None);
    model_action.connect_activate({
        let models = models.clone();
        move |_, _| models.show()
    });
    application.add_action(&model_action);
    let layout = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let history = Rc::new(HistoryView::new(&window, actions.clone()));
    history.record_button.set_sensitive(false);
    layout.append(&history.root);
    let status_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    status_row.add_css_class("koett-status");
    let status = gtk::Label::new(Some("Starting…"));
    status.set_xalign(0.0);
    status.set_hexpand(true);
    status.set_ellipsize(gtk::pango::EllipsizeMode::End);
    status.set_tooltip_text(Some("Starting…"));
    let shortcut = gtk::Label::new(None);
    shortcut.add_css_class("koett-metadata");
    shortcut.set_ellipsize(gtk::pango::EllipsizeMode::End);
    shortcut.set_max_width_chars(28);
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
    let latest_media = Rc::new(RefCell::new(None::<String>));
    let latest_delivery = Rc::new(RefCell::new(None::<String>));
    recovery.connect_clicked({
        let actions = actions.clone();
        move |_| {
            let _ = actions.send(TrayAction::Recovery("copy-last".into()));
        }
    });
    status_row.append(&recovery);
    layout.append(&status_row);
    let delivery_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    delivery_row.add_css_class("koett-delivery");
    let delivery_label = gtk::Label::new(Some("Automatic paste unavailable"));
    delivery_row.append(&delivery_label);
    let delivery_copy = gtk::Button::with_label("Copy transcript");
    delivery_row.append(&delivery_copy);
    delivery_row.set_visible(false);
    layout.append(&delivery_row);
    delivery_copy.connect_clicked({
        let latest = latest_delivery.clone();
        move |button| {
            if let Some(text) = latest.borrow().as_ref() {
                match super::feedback::copy_transcript(&button.clipboard(), text) {
                    Ok(()) => button.set_label("Copied"),
                    Err(error) => {
                        button.set_label("Copy failed · retry");
                        eprintln!("Copy transcript: {error}");
                    }
                }
            }
        }
    });
    let unsaved_row = gtk::Box::new(gtk::Orientation::Vertical, 6);
    unsaved_row.add_css_class("koett-delivery");
    let unsaved_label = gtk::Label::new(Some("Save failed · Unsaved transcripts in memory"));
    unsaved_label.set_wrap(true);
    unsaved_label.set_xalign(0.0);
    unsaved_row.append(&unsaved_label);
    let unsaved_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    for (label, command) in [
        ("Copy Unsaved", "copy-unsaved"),
        ("Retry Save", "retry-unsaved"),
    ] {
        let button = gtk::Button::with_label(label);
        button.connect_clicked({
            let actions = actions.clone();
            move |_| {
                let _ = actions.send(TrayAction::Recovery(command.into()));
            }
        });
        unsaved_actions.append(&button);
    }
    unsaved_row.append(&unsaved_actions);
    unsaved_row.set_visible(false);
    layout.append(&unsaved_row);
    window.set_child(Some(&layout));

    record.connect_clicked({
        let actions = actions.clone();
        move |_| {
            let _ = actions.send(TrayAction::Toggle);
        }
    });
    let settings = gtk::gio::SimpleAction::new("settings", None);
    settings.connect_activate({
        let window = window.clone();
        let settings_saved = settings_saved.clone();
        move |_, _| super::settings_view::show(&window, settings_saved.clone(), None)
    });
    application.add_action(&settings);
    application.set_accels_for_action("app.settings", &["<Control>comma"]);
    settings_button.set_action_name(Some("app.settings"));
    let show_history = gtk::gio::SimpleAction::new("history", None);
    show_history.connect_activate({
        let window = window.clone();
        let history = history.clone();
        move |_, _| {
            history.refresh();
            window.present();
        }
    });
    application.add_action(&show_history);
    for (name, command) in [
        ("ask", TrayAction::Ask),
        ("cancel-ask", TrayAction::CancelAsk),
    ] {
        let action = gtk::gio::SimpleAction::new(name, None);
        let actions = actions.clone();
        action.connect_activate(move |_, _| {
            let _ = actions.send(command.clone());
        });
        application.add_action(&action);
    }
    let media_action = gtk::gio::SimpleAction::new("media", None);
    media_action.connect_activate({
        let latest = latest_media.clone();
        let status = status.clone();
        let window = window.clone();
        let actions = actions.clone();
        move |_, _| {
            super::media_view::show(
                window.clone(),
                actions.clone(),
                latest.clone(),
                status.clone(),
            )
        }
    });
    application.add_action(&media_action);
    application.set_accels_for_action("app.media", &["<Control><Shift>t"]);
    for name in [
        "paste-last",
        "copy-unsaved",
        "retry-unsaved",
        "recovery-status",
        "media-transcripts",
        "failed-transcripts",
        "failed-recordings",
    ] {
        let action = gtk::gio::SimpleAction::new(name, None);
        action.connect_activate({
            let actions = actions.clone();
            move |_, _| {
                let _ = actions.send(TrayAction::Recovery(name.into()));
            }
        });
        if name == "paste-last" {
            action.set_enabled(false);
            gtk::glib::timeout_add_local(Duration::from_millis(100), {
                let action = action.clone();
                move || {
                    action.set_enabled(crate::history::recovery::last_available());
                    gtk::glib::ControlFlow::Continue
                }
            });
        }
        application.add_action(&action);
    }
    let hide = gtk::gio::SimpleAction::new("hide", None);
    hide.connect_activate({
        let window = window.clone();
        move |_, _| window.set_visible(false)
    });
    application.add_action(&hide);
    let copy_last = gtk::gio::SimpleAction::new("copy-last", None);
    copy_last.set_enabled(false);
    copy_last.connect_activate({
        let actions = actions.clone();
        move |_, _| {
            let _ = actions.send(TrayAction::Recovery("copy-last".into()));
        }
    });
    application.add_action(&copy_last);
    gtk::glib::timeout_add_local(Duration::from_millis(100), {
        let copy_last = copy_last.clone();
        move || {
            copy_last.set_enabled(crate::history::recovery::last_available());
            gtk::glib::ControlFlow::Continue
        }
    });
    window.connect_is_active_notify(|window| {
        update_active(window.upcast_ref());
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

    let ask = super::ask_view::AskView::new(application, actions.clone());
    let overlay =
        super::recording_overlay::RecordingOverlay::new(application, latest_delivery.clone());
    let mut recording_started = None::<Instant>;
    let mut media_result = false;
    let mut state = String::from("Starting…");
    let mut notice = None::<String>;
    let mut recovery_until = None::<Instant>;
    let mut notice_until = None::<Instant>;
    let mut feedback = super::feedback::CaptureFeedback::default();
    let mut audio_levels = None::<crate::audio::AudioLevels>;
    gtk::glib::timeout_add_local(Duration::from_millis(80), {
        let window = window.clone();
        move || {
            shortcut.set_visible(window.width() >= 650);
            meter_label.set_visible(window.width() >= 650);
            for update in updates.try_iter() {
                if let Update::Status(message) = &update {
                    if let Some(payload) = message.strip_prefix("@ask:") {
                        if let Ok(value) = serde_json::from_str(payload) {
                            ask.update(&value);
                        }
                        continue;
                    }
                    if let Some(name) = message.strip_prefix("@model:") {
                        models.active(name);
                        continue;
                    }
                    formatting.update(message);
                    if message.starts_with("@format-") {
                        continue;
                    }
                    if !message.starts_with('@') || message.starts_with("@notice:") {
                        models.update(message);
                    }
                }
                match update {
                    Update::PrepareAsk(ack) => {
                        ask.hide();
                        gtk::glib::timeout_add_local_once(Duration::from_millis(100), move || {
                            let _ = ack.send(());
                        });
                    }
                    Update::Show(token) => {
                        history.refresh();
                        present(window.upcast_ref(), Some(&token));
                    }
                    Update::Settings(token) => {
                        // Map the parent without requesting activation. The
                        // one-use token belongs to Settings, not History.
                        window.set_visible(true);
                        super::settings_view::show(&window, settings_saved.clone(), Some(&token));
                    }
                    Update::Levels(levels) => audio_levels = Some(levels),
                    Update::Shortcut(label) => shortcut.set_text(&label),
                    Update::DeliveryFailed => {
                        if latest_completed.borrow().is_some() {
                            recovery_until =
                                Some(Instant::now() + super::feedback::RECOVERY_DURATION);
                            delivery_copy.set_label("Copy transcript");
                        }
                    }
                    Update::Status(message) if message.starts_with("@notice:@media-result:") => {
                        let text = message
                            .trim_start_matches("@notice:@media-result:")
                            .to_string();
                        *latest_media.borrow_mut() = Some(text.clone());
                        *latest_delivery.borrow_mut() = Some(text);
                        recovery.set_visible(true);
                        recovery_until = Some(Instant::now() + super::feedback::RECOVERY_DURATION);
                        media_result = true;
                        state = "Media result".into();
                    }
                    Update::Completed(text) => {
                        *latest_delivery.borrow_mut() = Some(text.clone());
                        media_result = false;
                        *latest_completed.borrow_mut() = Some(text);
                        recovery.set_visible(true);
                        copy_last.set_enabled(true);
                        recovery.set_label("Copy last transcript");
                    }
                    Update::Status(message) if message == "@quit-protected" => {
                        let dialog = gtk::MessageDialog::builder().transient_for(&window).modal(true).text("Unsaved transcripts remain in memory").secondary_text("Saving failed. Retry Save or Copy Unsaved before quitting. Quit Anyway discards these records.").build();
                        dialog.add_button("Keep open", gtk::ResponseType::Cancel);
                        dialog.add_button("Copy Unsaved", gtk::ResponseType::Other(1));
                        dialog.add_button("Retry Save", gtk::ResponseType::Other(2));
                        dialog.add_button("Quit Anyway", gtk::ResponseType::Other(3));
                        dialog.connect_response({
                            let actions = actions.clone();
                            move |dialog, response| {
                                let action = match response {
                                    gtk::ResponseType::Other(1) => {
                                        Some(TrayAction::Recovery("copy-unsaved".into()))
                                    }
                                    gtk::ResponseType::Other(2) => {
                                        Some(TrayAction::Recovery("retry-unsaved".into()))
                                    }
                                    gtk::ResponseType::Other(3) => Some(TrayAction::QuitAnyway),
                                    _ => None,
                                };
                                if let Some(action) = action {
                                    let _ = actions.send(action);
                                }
                                dialog.close();
                            }
                        });
                        dialog.present();
                    }
                    Update::Status(message) if message == "@quit" => {
                        if let Some(overlay) = &overlay {
                            overlay.close();
                        }
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
                    Update::Status(message) if message == "@media" => {
                        window.set_visible(true);
                        if let Some(app) = window.application() {
                            app.activate_action("media", None);
                        }
                    }
                    Update::Status(message)
                        if matches!(message.as_str(), "@models" | "@formatting" | "@about") =>
                    {
                        if message == "@models" {
                            models.show();
                        } else if message == "@formatting" {
                            formatting.button.popup();
                        } else {
                            gtk::AboutDialog::builder()
                                .transient_for(&window)
                                .modal(true)
                                .program_name("Koett")
                                .version(env!("CARGO_PKG_VERSION"))
                                .license_type(gtk::License::Apache20)
                                .build()
                                .present();
                        }
                    }
                    Update::Status(message) if message == "@settings" => {
                        window.present();
                        let parent = window.clone();
                        let saved = settings_saved.clone();
                        gtk::glib::idle_add_local_once(move || {
                            super::settings_view::show(&parent, saved, None);
                        });
                    }
                    Update::Status(message) if message == "@refresh" => history.refresh(),
                    Update::Status(message) if message.starts_with("@notice:") => {
                        let phase = message.trim_start_matches("@notice:");
                        if matches!(
                            phase,
                            "Fetching audio…" | "Converting audio…" | "Transcribing locally…"
                        ) {
                            state = phase.into();
                            notice = None;
                        } else {
                            notice = Some(phase.to_string());
                        }
                        notice_until = Some(Instant::now() + Duration::from_secs(8));
                    }
                    Update::Status(message) => {
                        let recording = message.starts_with("Recording");
                        if recording && recording_started.is_none() {
                            recording_started = Some(Instant::now());
                            feedback = super::feedback::CaptureFeedback::default();
                            recovery_until = None;
                            notice = None;
                            notice_until = None;
                        }
                        if !recording {
                            recording_started = None;
                        }
                        let action = super::menu::recording_action(&message);
                        record.set_label(action.label);
                        record.set_sensitive(action.enabled);
                        history
                            .record_button
                            .set_label(record.label().as_deref().unwrap_or("Record"));
                        history.record_button.set_sensitive(record.is_sensitive());
                        state = message;
                    }
                }
            }
            if notice_until.is_some_and(|until| Instant::now() >= until) {
                notice = None;
                notice_until = None;
            }
            if recovery_until.is_some_and(|until| Instant::now() >= until) {
                recovery_until = None;
            }
            let unsaved_count = crate::history::recovery::pending_count();
            unsaved_row.set_visible(unsaved_count > 0);
            if unsaved_count > 0 {
                unsaved_label.set_text(&format!(
                    "Save failed · {unsaved_count} unsaved transcript(s) in memory"
                ));
            }
            let recovering = recovery_until.is_some();
            delivery_label.set_text(if media_result {
                "Media transcript"
            } else {
                "Automatic paste unavailable"
            });
            delivery_row.set_visible(recovering);
            recovery.set_visible(latest_completed.borrow().is_some() && !recovering);
            let warning = recording_started.and_then(|started| {
                audio_levels.as_ref().and_then(|levels| {
                    let (rms, peak) = levels.current();
                    feedback.update(started.elapsed(), rms, peak)
                })
            });
            meter_group.set_visible(recording_started.is_some());
            if recording_started.is_some()
                && let Some(levels) = &audio_levels
            {
                meter.set_value(f64::from((levels.current().0 * 10.0).clamp(0.0, 1.0)));
            }
            if let Some(overlay) = &overlay {
                overlay.update(
                    if media_result && recovering {
                        "Media result"
                    } else {
                        &state
                    },
                    recording_started,
                    audio_levels.as_ref(),
                    warning,
                    recovering,
                );
            }
            status.set_text(
                &recording_started
                    .map(|started| {
                        super::feedback::recording_status(
                            started.elapsed(),
                            warning,
                            notice.as_deref(),
                        )
                    })
                    .unwrap_or_else(|| match &notice {
                        Some(notice) => format!("{state} · {notice}"),
                        None => state.clone(),
                    }),
            );
            status.set_tooltip_text(Some(&status.text()));
            gtk::glib::ControlFlow::Continue
        }
    });
    if visible {
        window.present();
    }
}

// Count interactive windows together: deactivation of History must not clear
// the paste guard while Settings is active. The layer surface never participates.
pub(super) fn update_active(window: &gtk::Window) {
    let active = window.application().is_some_and(|app| {
        app.windows().iter().any(|window| {
            window.title().as_deref() != Some("Koett recording") && window.is_active()
        })
    });
    super::output::WINDOW_ACTIVE.store(active, std::sync::atomic::Ordering::Release);
}

// GTK 4.6's Wayland startup-id property is a no-op. Its supported display
// API supplies the next focus request; GTK >=4.10 uses the per-window ID.
// https://gitlab.gnome.org/GNOME/gtk/-/blob/4.6.9/gdk/wayland/gdksurface-wayland.c
pub(super) fn present(window: &gtk::Window, token: Option<&str>) {
    if let Some(token) = token {
        let display = WidgetExt::display(window);
        if gtk::minor_version() < 10
            && let Some(display) = display.downcast_ref::<gdk4_wayland::WaylandDisplay>()
        {
            display.set_startup_notification_id(token);
        } else {
            window.set_startup_id(token);
        }
    }
    window.present();
}
