use super::{TrayAction, history_view::HistoryView};
use crate::settings::Settings;
use gtk4::{self as gtk, prelude::*};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DrawingRequest {
    Status,
    Undo,
    ToggleLock,
}

pub enum Update {
    DrawingCommand(DrawingRequest, Sender<serde_json::Value>, Instant),
    RecordingStatus(Sender<serde_json::Value>),
    MediaCommand(bool, Sender<serde_json::Value>),
    PrepareAsk(Sender<()>),
    Status(String),
    Show(String),
    Settings(String),
    Completed(String),
    DeliveryStarted,
    Delivered,
    Copied,
    /// The text pasted and the drawing didn't; true when the drawing is on the clipboard.
    TextOnly(bool),
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
            super::permissions_view::maybe_show(application);
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
    #[cfg(feature = "ask")]
    let ask_button = gtk::Button::with_label("Ask");
    #[cfg(feature = "ask")]
    let ask_actions = actions.clone();
    #[cfg(feature = "ask")]
    ask_button.connect_clicked(move |_| {
        let _ = ask_actions.send(TrayAction::Ask);
    });
    #[cfg(feature = "ask")]
    header.pack_start(&ask_button);
    let settings_button = gtk::Button::from_icon_name("preferences-system-symbolic");
    settings_button.set_tooltip_text(Some("Settings"));
    super::menu::set_accessible_label(settings_button.upcast_ref(), "Settings");
    #[cfg(feature = "ask")]
    super::menu_preferences::attach(application);
    header.pack_end(&super::menu::button());
    header.pack_end(&settings_button);
    window.set_titlebar(Some(&header));

    let formatting = Rc::new(super::formatting_view::FormattingPanel::new(
        actions.clone(),
    ));
    let models = Rc::new(super::models_view::ModelsPanel::new(actions.clone()));
    let panels = Rc::new(super::settings_view::Panels {
        models: models.clone(),
        formatting: formatting.clone(),
    });
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
    let media_results = Rc::new(std::cell::Cell::new(0_u64));
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
        let panels = panels.clone();
        move |_, _| super::settings_view::show(&window, &panels, settings_saved.clone(), None, None)
    });
    application.add_action(&settings);
    let permissions = gtk::gio::SimpleAction::new("permissions", None);
    permissions.connect_activate({
        let window = window.clone();
        let panels = panels.clone();
        let settings_saved = settings_saved.clone();
        move |_, _| {
            let parent = window.clone();
            let panels = panels.clone();
            let saved = settings_saved.clone();
            super::permissions_view::show(
                &window,
                Rc::new(move |page| {
                    super::settings_view::show(&parent, &panels, saved.clone(), None, Some(page))
                }),
            );
        }
    });
    application.add_action(&permissions);

    let about = gtk::gio::SimpleAction::new("about", None);
    about.connect_activate({
        let window = window.clone();
        move |_, _| super::settings_view::about_dialog(window.upcast_ref())
    });
    application.add_action(&about);
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
    // For the GNOME extension's top bar menu, which reads these over the bus.
    let toggle_recording = gtk::gio::SimpleAction::new("toggle-recording", None);
    toggle_recording.connect_activate({
        let actions = actions.clone();
        move |_, _| {
            let _ = actions.send(TrayAction::Toggle);
        }
    });
    application.add_action(&toggle_recording);
    let recording_state =
        gtk::gio::SimpleAction::new_stateful("recording", None, &false.to_variant());
    recording_state.set_enabled(false);
    application.add_action(&recording_state);
    let check_updates = gtk::gio::SimpleAction::new("check-updates", None);
    check_updates.set_enabled(super::updates::enabled());
    check_updates.connect_activate(|_, _| {
        super::updates::check_now();
    });
    application.add_action(&check_updates);
    #[cfg(feature = "ask")]
    for (name, command) in [
        #[cfg(feature = "ask")]
        ("ask", TrayAction::Ask),
        #[cfg(feature = "ask")]
        ("cancel-ask", TrayAction::CancelAsk),
    ] {
        let action = gtk::gio::SimpleAction::new(name, None);
        let actions = actions.clone();
        action.connect_activate(move |_, _| {
            let _ = actions.send(command.clone());
        });
        application.add_action(&action);
    }
    let media_action = gtk::gio::SimpleAction::new("media-manual", None);
    media_action.connect_activate({
        let latest = latest_media.clone();
        let results = media_results.clone();
        let status = status.clone();
        let window = window.clone();
        let actions = actions.clone();
        move |_, _| {
            super::media_view::show(
                window.clone(),
                actions.clone(),
                latest.clone(),
                results.clone(),
                status.clone(),
            )
        }
    });
    application.add_action(&media_action);
    let current_media = gtk::gio::SimpleAction::new("media", None);
    current_media.connect_activate({
        let actions = actions.clone();
        move |_, _| {
            let _ = actions.send(TrayAction::OpenMedia);
        }
    });
    application.add_action(&current_media);
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

    #[cfg(feature = "ask")]
    let ask = super::ask_view::AskView::new(application, actions.clone());
    let overlay = super::recording_overlay::RecordingOverlay::new(
        application,
        latest_delivery.clone(),
        actions.clone(),
    );
    // Done: shown briefly after a successful paste, held while hovered, then
    // faded. Copied to clipboard, when a paste didn't happen, shows the same way.
    // Not pasted only appears when even the clipboard failed, and stays until
    // Copy or the next dictation.
    let mut done_until = None::<Instant>;
    let mut done_fading = None::<Instant>;
    // Set after Copy: the pill closes on time even while hovered.
    let mut closing = false;
    // Wayland pastes asynchronously, so Ready arrives before the paste
    // outcome. Keep showing the decoding pill until the outcome (or SETTLE),
    // so it morphs straight into Done instead of hiding and remapping.
    let mut settling_until = None::<Instant>;
    let mut delivery_pending = false;
    let sketch = super::sketch::SketchOverlay::new().map(Rc::new);
    let undo_drawing = gtk::gio::SimpleAction::new("undo-drawing", None);
    undo_drawing.set_enabled(false);
    undo_drawing.connect_activate({
        let sketch = sketch.clone();
        move |_, _| {
            if let Some(sketch) = &sketch {
                sketch.undo_last();
            }
        }
    });
    application.add_action(&undo_drawing);
    let sketch_application = application.clone();
    let mut asking_until = None::<Instant>;
    let mut sketch_recording = 0_u64;
    let mut recording_started = None::<Instant>;
    let mut media_result = false;
    let mut state = String::from("Starting…");
    let mut notice = None::<String>;
    let mut recovery_until = None::<Instant>;
    let mut notice_until = None::<Instant>;
    let mut pill_message = None::<(String, Instant)>;
    let mut stopped_at_limit = None::<String>;
    let mut feedback = super::feedback::CaptureFeedback::default();
    let mut audio_levels = None::<crate::audio::AudioLevels>;
    gtk::glib::timeout_add_local(Duration::from_millis(80), {
        let window = window.clone();
        move || {
            shortcut.set_visible(window.width() >= 650);
            meter_label.set_visible(window.width() >= 650);
            super::updates::set_ui_state(
                overlay.as_ref().is_some_and(|overlay| overlay.is_shown())
                    || gtk::Window::list_toplevels().iter().any(|toplevel| {
                        toplevel.downcast_ref::<gtk::Window>().is_some_and(|other| {
                            other.is_visible() && (other.is_active() || other.is_modal())
                        })
                    }),
                window.is_visible(),
            );
            for update in updates.try_iter() {
                if let Update::Status(message) = &update {
                    if message == "@undo-drawing" {
                        if recording_started.is_some()
                            && let Some(sketch) = &sketch
                        {
                            sketch.undo_last();
                        }
                        continue;
                    }
                    if media_result
                        && (message.starts_with("@ask:")
                            || (message.starts_with("@notice:")
                                && !message.starts_with("@notice:@media-result:")))
                    {
                        media_result = false;
                        recovery_until = None;
                    }
                    #[cfg(feature = "ask")]
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
                    Update::RecordingStatus(reply) => {
                        let _ = reply.send(super::menu_preferences::recording_status());
                    }
                    Update::DrawingCommand(request, reply, expires) => {
                        let value = if Instant::now() >= expires {
                            serde_json::json!({"ok":false,"error":{"code":"drawing_request_expired","message":"Drawing request expired; inspect drawing-status before retrying"}})
                        } else if request == DrawingRequest::ToggleLock {
                            if recording_started.is_some()
                                && sketch.as_ref().is_some_and(|sketch| sketch.toggle_lock())
                            {
                                sketch
                                    .as_ref()
                                    .map_or(serde_json::json!({"ok":true}), |sketch| {
                                        sketch.status()
                                    })
                            } else {
                                serde_json::json!({"ok":false,"error":{"code":"no_drawing_session","message":"Draw lock needs a recording with drawing on"}})
                            }
                        } else if request == DrawingRequest::Undo {
                            if recording_started.is_some()
                                && sketch.as_ref().is_some_and(|sketch| sketch.undo_last())
                            {
                                serde_json::json!({"ok":true,"status":"drawing_removed"})
                            } else {
                                serde_json::json!({"ok":false,"error":{"code":"no_drawing","message":"No drawing to undo in the current recording"}})
                            }
                        } else {
                            sketch.as_ref().map_or_else(|| serde_json::json!({"ok":true,"active":false,"count":0,"captured_count":0,"can_undo":false}), |sketch| sketch.status())
                        };
                        let _ = reply.send(value);
                    }
                    Update::PrepareAsk(ack) => {
                        // The next recording is an Ask question, not dictation.
                        asking_until = Some(Instant::now() + Duration::from_secs(20));
                        media_result = false;
                        recovery_until = None;
                        #[cfg(feature = "ask")]
                        ask.hide();
                        gtk::glib::timeout_add_local_once(Duration::from_millis(100), move || {
                            let _ = ack.send(());
                        });
                    }
                    Update::Show(token) => {
                        history.refresh();
                        present(window.upcast_ref(), Some(&token));
                        if let Some(app) = window.application() {
                            super::permissions_view::maybe_show(&app);
                        }
                    }
                    Update::Settings(token) => {
                        // Map the parent without requesting activation. The
                        // one-use token belongs to Settings, not History.
                        window.set_visible(true);
                        super::settings_view::show(
                            &window,
                            &panels,
                            settings_saved.clone(),
                            Some(&token),
                            None,
                        );
                    }
                    Update::MediaCommand(copy, reply) => {
                        let value = match latest_media.borrow().as_ref() {
                            Some(text) if copy => {
                                match super::feedback::copy_transcript(&window.clipboard(), text) {
                                    Ok(()) => {
                                        serde_json::json!({"ok":true,"status":"copied","characters":text.chars().count()})
                                    }
                                    Err(error) => {
                                        serde_json::json!({"ok":false,"error":{"code":"clipboard_failed","message":error.to_string()}})
                                    }
                                }
                            }
                            Some(text) => {
                                serde_json::json!({"ok":true,"status":"available","characters":text.chars().count()})
                            }
                            None => {
                                serde_json::json!({"ok":false,"error":{"code":"no_media_result","message":"Complete a media transcription first"}})
                            }
                        };
                        let _ = reply.send(value);
                    }
                    Update::Levels(levels) => audio_levels = Some(levels),
                    Update::Shortcut(label) => shortcut.set_text(&label),
                    Update::DeliveryStarted => {
                        if let Some(overlay) = &overlay {
                            overlay.set_copied(false);
                        }
                        delivery_pending = true;
                        settling_until = None;
                        done_until = None;
                        recovery_until = None;
                        closing = false;
                    }
                    // Not pasted but on the clipboard: Done's timing, saying so instead.
                    Update::Copied => {
                        delivery_pending = false;
                        settling_until = None;
                        recovery_until = None;
                        if let Some(overlay) = &overlay {
                            overlay.set_copied(true);
                        }
                        done_until = Some(Instant::now() + DONE_DURATION);
                        done_fading = None;
                    }
                    // Never the rescue with Paste: pasting again would repeat the text.
                    Update::TextOnly(drawing_copied) => {
                        delivery_pending = false;
                        settling_until = None;
                        recovery_until = None;
                        if let Some(overlay) = &overlay {
                            overlay.set_text_only(drawing_copied);
                        }
                        done_until = Some(Instant::now() + DONE_DURATION);
                        done_fading = None;
                    }
                    Update::Delivered => {
                        if let Some(overlay) = &overlay {
                            overlay.set_copied(false);
                        }
                        delivery_pending = false;
                        settling_until = None;
                        if let Some(limit_notice) = stopped_at_limit.take() {
                            done_until = None;
                            pill_message =
                                Some((limit_notice, Instant::now() + PILL_MESSAGE_DURATION));
                        } else {
                            done_until = Some(Instant::now() + DONE_DURATION);
                        }
                        done_fading = None;
                    }
                    Update::DeliveryFailed => {
                        delivery_pending = false;
                        settling_until = None;
                        done_until = None;
                        if latest_completed.borrow().is_some() {
                            recovery_until = Some(Instant::now() + UNTIL_ACTED);
                            delivery_copy.set_label("Copy transcript");
                        }
                    }
                    Update::Status(message) if message.starts_with("@notice:@media-result:") => {
                        let text = message
                            .trim_start_matches("@notice:@media-result:")
                            .to_string();
                        *latest_media.borrow_mut() = Some(text.clone());
                        media_results.set(media_results.get() + 1);
                        *latest_delivery.borrow_mut() = Some(text);
                        recovery.set_visible(true);
                        recovery_until = Some(Instant::now() + Duration::from_secs(10));
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
                        if let Some(app) = window.application() {
                            super::permissions_view::maybe_show(&app);
                        }
                    }
                    Update::Status(message) if message == "@media" => {
                        window.set_visible(true);
                        if let Some(app) = window.application() {
                            app.activate_action("media-manual", None);
                        }
                    }
                    Update::Status(message)
                        if matches!(message.as_str(), "@models" | "@formatting" | "@about") =>
                    {
                        if message == "@about" {
                            super::settings_view::about_dialog(window.upcast_ref());
                        } else {
                            window.present();
                            super::settings_view::show(
                                &window,
                                &panels,
                                settings_saved.clone(),
                                None,
                                Some("model"),
                            );
                        }
                    }
                    Update::Status(message) if message == "@permissions" => {
                        if let Some(app) = window.application() {
                            app.activate_action("permissions", None);
                        }
                    }
                    Update::Status(message) if message == "@settings" => {
                        window.present();
                        let parent = window.clone();
                        let saved = settings_saved.clone();
                        let panels = panels.clone();
                        gtk::glib::idle_add_local_once(move || {
                            super::settings_view::show(&parent, &panels, saved, None, None);
                        });
                    }
                    Update::Status(message) if message == "@words" => {
                        window.present();
                        super::settings_view::show(
                            &window,
                            &panels,
                            settings_saved.clone(),
                            None,
                            Some("words"),
                        );
                    }
                    Update::Status(message) if message == "@refresh" => history.refresh(),
                    Update::Status(message) if message == "@silence" => {
                        // No delivery outcome will arrive for an empty result.
                        state = "Ready".into();
                        recording_started = None;
                        settling_until = None;
                        done_until = None;
                        done_fading = None;
                        recovery_until = None;
                        stopped_at_limit = None;
                    }
                    Update::Status(message) if message.starts_with("@error:") => {
                        let error = message.trim_start_matches("@error:");
                        pill_message = Some((
                            super::recording_overlay::error_row(error).to_string(),
                            Instant::now() + PILL_MESSAGE_DURATION,
                        ));
                        notice = Some(error.to_string());
                        notice_until = Some(Instant::now() + Duration::from_secs(8));
                    }
                    Update::Status(message)
                        if message.strip_prefix("@notice:").is_some_and(|notice| {
                            notice.starts_with(crate::runtime::MEDIA_ERROR)
                        }) =>
                    {
                        let error = message
                            .trim_start_matches("@notice:")
                            .trim_start_matches(crate::runtime::MEDIA_ERROR);
                        pill_message = Some((
                            super::recording_overlay::error_row("media").to_string(),
                            Instant::now() + PILL_MESSAGE_DURATION,
                        ));
                        notice = Some(error.to_string());
                        notice_until = Some(Instant::now() + Duration::from_secs(8));
                    }
                    Update::Status(message) if message.starts_with("@notice:") => {
                        let phase = message
                            .trim_start_matches("@notice:")
                            .trim_start_matches("@model-notice:");
                        if crate::runtime::is_limit_notice(phase) {
                            stopped_at_limit = Some(phase.to_string());
                        }
                        if phase == crate::runtime::LIMIT_WARNING {
                            pill_message =
                                Some((phase.to_string(), Instant::now() + PILL_MESSAGE_DURATION));
                        }
                        if phase == "Dictation cancelled" {
                            // Says Cancelled once and fades on Done's timing, even
                            // under the pointer that just pressed ✕.
                            settling_until = None;
                            recovery_until = None;
                            pill_message = None;
                            if let Some(overlay) = &overlay {
                                overlay.set_cancelled();
                            }
                            done_until = Some(Instant::now() + DONE_DURATION);
                            done_fading = None;
                            closing = true;
                        }
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
                        if message.starts_with("Error: ") && message != state {
                            pill_message = Some((
                                super::recording_overlay::error_row(&message).to_string(),
                                Instant::now() + PILL_MESSAGE_DURATION,
                            ));
                        }
                        if media_result && !message.starts_with("Ready") {
                            media_result = false;
                            recovery_until = None;
                        }
                        let recording = message.starts_with("Recording");
                        recording_state.set_state(&recording.to_variant());
                        // X11 delivers synchronously: the outcome may already be in.
                        if state.starts_with("Transcribing")
                            && !message.starts_with("Transcribing")
                            && !recording
                            && done_until.is_none()
                            && recovery_until.is_none()
                        {
                            settling_until = Some(Instant::now() + SETTLE);
                        }
                        if recording {
                            delivery_pending = false;
                            settling_until = None;
                        }
                        if recording && recording_started.is_none() {
                            recording_started = Some(Instant::now());
                            // Scope sketches to this recording, even with drawing off.
                            sketch_recording = super::sketch::begin_recording();
                            done_until = None;
                            feedback = super::feedback::CaptureFeedback::default();
                            recovery_until = None;
                            notice = None;
                            notice_until = None;
                            pill_message = None;
                            stopped_at_limit = None;
                        }
                        if !recording {
                            if recording_started.is_some() {
                                asking_until = None;
                                // Cancelled or failed: no transcript will claim them.
                                if !message.starts_with("Transcribing") {
                                    super::sketch::discard_recording();
                                }
                            }
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
            if pill_message
                .as_ref()
                .is_some_and(|(_, until)| Instant::now() >= *until)
            {
                pill_message = None;
            }
            let pill_hovered = overlay.as_ref().is_some_and(|overlay| overlay.hovered());
            // Copy morphs the pill into Copied to clipboard, which fades on Done's timing
            // even while hovered, since the pointer is still on the button it pressed.
            // Paste (Not pasted) closes at once and pastes from the tray action.
            match overlay.as_ref().and_then(|overlay| overlay.take_acted()) {
                Some(super::recording_overlay::Acted::Copied) => {
                    if recovery_until.is_some() || done_until.is_some() {
                        if let Some(overlay) = &overlay {
                            overlay.set_copied(true);
                        }
                        recovery_until = None;
                        done_until = Some(Instant::now() + DONE_DURATION);
                        done_fading = None;
                        closing = true;
                        // The failed delivery's error row outranks Done and would come
                        // back for the rest of its five seconds; Copy just fixed it.
                        pill_message = None;
                    }
                }
                Some(super::recording_overlay::Acted::CopyFailed) => {
                    if let Some(overlay) = &overlay {
                        overlay.set_copy_failed();
                    }
                }
                Some(super::recording_overlay::Acted::Pasted) => {
                    recovery_until = None;
                }
                None => {}
            }
            if recovery_until.is_none() && done_until.is_none() {
                closing = false;
            }
            let pill_hovered = pill_hovered && !closing;
            if recovery_until.is_some_and(|until| Instant::now() >= until) {
                if pill_hovered {
                    recovery_until = Some(Instant::now() + HOVER_GRACE);
                } else {
                    recovery_until = None;
                }
            }
            if let Some(until) = done_until
                && Instant::now() >= until
            {
                if pill_hovered {
                    done_until = Some(Instant::now() + HOVER_GRACE);
                    done_fading = None;
                    if let Some(overlay) = &overlay {
                        overlay.fade(false);
                    }
                } else if let Some(since) = done_fading {
                    if since.elapsed() >= FADE {
                        done_until = None;
                        done_fading = None;
                    }
                } else {
                    done_fading = Some(Instant::now());
                    if let Some(overlay) = &overlay {
                        overlay.fade(true);
                    }
                }
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
            if let Some(sketch) = &sketch {
                let asking = asking_until.is_some_and(|until| Instant::now() < until);
                let paused = audio_levels
                    .as_ref()
                    .is_some_and(crate::audio::AudioLevels::paused);
                sketch.update(
                    &sketch_application,
                    recording_started.map(|started| (started, sketch_recording)),
                    asking,
                    paused,
                );
                if let Some(message) = sketch.take_notice() {
                    if message.redraw_required {
                        pill_message = Some((
                            "Display changed · redraw unfinished items".into(),
                            Instant::now() + Duration::from_secs(12),
                        ));
                    }
                    notice = Some(message.text);
                    notice_until = Some(Instant::now() + Duration::from_secs(12));
                }
            }
            let can_undo = recording_started.is_some()
                && sketch.as_ref().is_some_and(|sketch| sketch.can_undo());
            undo_drawing.set_enabled(can_undo);
            super::sketch::CAN_UNDO.store(can_undo, std::sync::atomic::Ordering::Release);
            if let Some(overlay) = &overlay {
                overlay.update(
                    if media_result && recovering {
                        "Media result"
                    } else if delivery_pending
                        || settling_until.is_some_and(|until| Instant::now() < until)
                    {
                        "Transcribing…"
                    } else {
                        &state
                    },
                    recording_started,
                    audio_levels.as_ref(),
                    warning,
                    recovering,
                    done_until.is_some(),
                    if media_result {
                        latest_media.borrow().clone()
                    } else {
                        None
                    }
                    .as_deref(),
                    pill_message.as_ref().map(|(text, _)| text.as_str()),
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
        super::permissions_view::maybe_show(application);
    }
}

/// Done stays this long after a successful paste, unless hovered.
const PILL_MESSAGE_DURATION: Duration = Duration::from_secs(5);
const DONE_DURATION: Duration = Duration::from_millis(1_750);
/// Longest wait for a paste outcome after decoding before the pill hides.
const SETTLE: Duration = Duration::from_millis(1_000);
/// Fade-out of the Done pill before it is hidden (matches the CSS transition).
const FADE: Duration = Duration::from_millis(260);
/// After the pointer leaves a held pill.
const HOVER_GRACE: Duration = Duration::from_millis(800);
/// Not pasted has no timeout; this only keeps the Instant arithmetic finite.
const UNTIL_ACTED: Duration = Duration::from_secs(60 * 60 * 24 * 365);

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
