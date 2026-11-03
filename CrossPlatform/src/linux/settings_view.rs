use crate::replacements::Replacement;
use crate::settings::{DrawKey, PasteShortcut, RecordingMode, Settings};
use gtk4::{self as gtk, prelude::*};
use std::rc::Rc;

const STACK_NAME: &str = "koett-settings-pages";

pub(super) struct Panels {
    pub models: Rc<super::models_view::ModelsPanel>,
    pub formatting: Rc<super::formatting_view::FormattingPanel>,
}

pub(super) fn show(
    parent: &gtk::ApplicationWindow,
    panels: &Panels,
    on_saved: Rc<dyn Fn(&Settings)>,
    token: Option<&str>,
    page: Option<&str>,
) {
    if let Some(window) = parent.application().and_then(|app| {
        app.windows()
            .into_iter()
            .find(|window| window.title().as_deref() == Some("Settings"))
    }) {
        if let Some(page) = page
            && let Some(stack) = find_stack(window.upcast_ref())
        {
            stack.set_visible_child_name(page);
        }
        super::window::present(&window, token);
        return;
    }
    let settings = match Settings::load() {
        Ok(s) => s,
        Err(error) => {
            super::show_desktop_message(&error);
            return;
        }
    };
    let window = gtk::Window::builder()
        .title("Settings")
        .transient_for(parent)
        .modal(true)
        .default_width(760)
        .default_height(600)
        .build();
    window.add_css_class("koett-window");
    window.set_application(parent.application().as_ref());
    window.connect_is_active_notify(super::window::update_active);
    let header = gtk::HeaderBar::new();
    let title = gtk::Label::new(Some("Settings"));
    title.add_css_class("koett-title");
    header.set_title_widget(Some(&title));
    window.set_titlebar(Some(&header));
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    body.set_vexpand(true);
    let stack = gtk::Stack::new();
    stack.set_widget_name(STACK_NAME);
    stack.set_hexpand(true);
    let navigation = gtk::StackSidebar::new();
    navigation.set_stack(&stack);
    navigation.add_css_class("koett-settings-sidebar");
    super::menu::set_accessible_label(navigation.upcast_ref(), "Settings section");
    body.append(&navigation);
    body.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    body.append(&stack);
    outer.append(&body);
    let dictation = section(&stack, "audio", "Dictation");
    let model = section(&stack, "model", "Model & formatting");
    let draw_page = section(&stack, "draw", "Draw on screen");
    let shortcuts_page = section(&stack, "shortcuts", "Shortcuts");
    let words = section(&stack, "words", "Words");
    let ask = section(&stack, "ask", "Ask About Screen");
    let general = section(&stack, "general", "General");
    let microphone = group(&dictation, "Microphone");
    let recording = group(&dictation, "Recording");
    let applies = gtk::Label::new(Some("Changes on this page apply right away."));
    applies.set_xalign(0.0);
    applies.set_wrap(true);
    applies.add_css_class("koett-setting-help");
    model.append(&applies);
    group(&model, "Speech model").append(&panels.models.build());
    group(&model, "Formatting").append(&panels.formatting.build());
    let shortcuts = group(&shortcuts_page, "Recording shortcuts");
    let shortcut_error = gtk::Label::new(None);
    shortcut_error.set_wrap(true);
    shortcut_error.set_xalign(0.0);
    shortcut_error.set_selectable(true);
    shortcut_error.add_css_class("error");
    shortcuts.append(&shortcut_error);
    gtk::glib::timeout_add_local(std::time::Duration::from_millis(250), {
        let label = shortcut_error.downgrade();
        move || {
            let Some(label) = label.upgrade() else {
                return gtk::glib::ControlFlow::Break;
            };
            if let Ok(status) = super::SHORTCUT_STATUS.read() {
                label.set_visible(status["ok"] == false);
                label.set_text(
                    status["error"]["message"]
                        .as_str()
                        .unwrap_or("Assign missing shortcuts in desktop settings."),
                );
            }
            gtk::glib::ControlFlow::Continue
        }
    });
    let dictionary = super::dictionary_view::DictionaryEditor::build(
        &group(&words, "Dictionary"),
        &window,
        &settings.dictionary,
    );
    let replacements = group(&words, "Replacements");
    super::ask_settings::build(&group(&ask, "Assistant"));
    stack.set_visible_child_name(page.unwrap_or("audio"));
    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    footer.add_css_class("koett-settings-footer");
    outer.append(&footer);
    tab_past_sidebar(&navigation, &stack, &header, &footer);
    window.set_child(Some(&outer));
    let input = gtk::ComboBoxText::new();
    for cell in input.cells() {
        if let Ok(text) = cell.downcast::<gtk::CellRendererText>() {
            text.set_ellipsize(gtk::pango::EllipsizeMode::End);
            text.set_max_width_chars(32);
        }
    }
    input.append(Some(""), "System default microphone");
    input.set_active_id(Some(""));
    if let Some(name) = &settings.microphone {
        input.append(Some(name), name);
        input.set_active_id(Some(name));
    }
    input.set_tooltip_text(input.active_text().as_deref());
    input.connect_changed(|input| input.set_tooltip_text(input.active_text().as_deref()));
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(crate::audio::input_devices());
    });
    gtk::glib::timeout_add_local(std::time::Duration::from_millis(50), {
        let input = input.downgrade();
        let saved = settings.microphone.clone();
        move || {
            let Some(input) = input.upgrade() else {
                return gtk::glib::ControlFlow::Break;
            };
            match rx.try_recv() {
                Ok(Ok(devices)) => {
                    // Enumeration must not overwrite a choice already made by the user.
                    let active = input.active_id();
                    if saved
                        .as_ref()
                        .is_some_and(|saved| devices.iter().any(|device| &device.id == saved))
                    {
                        input.remove(1);
                    }
                    for device in devices {
                        input.append(Some(&device.id), &device.label);
                    }
                    input.set_active_id(active.as_deref());
                }
                Ok(Err(error)) => {
                    input.set_tooltip_text(Some(&format!("Could not list microphones: {error}")))
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    return gtk::glib::ControlFlow::Continue;
                }
                Err(_) => {}
            }
            gtk::glib::ControlFlow::Break
        }
    });
    row(&microphone, "Microphone", &input);
    super::input_preview::attach(&window, &input, &microphone, &stack);
    let mode = gtk::ComboBoxText::new();
    mode.append(Some("toggle"), "Toggle");
    mode.append(Some("hold"), "Hold to record");
    mode.set_active_id(Some(if settings.recording_mode == RecordingMode::Hold {
        "hold"
    } else {
        "toggle"
    }));
    row(&recording, "Recording mode", &mode);
    let shortcut = std::rc::Rc::new(std::cell::RefCell::new(settings.shortcut.clone()));
    let ask_shortcut = std::rc::Rc::new(std::cell::RefCell::new(settings.ask_shortcut.clone()));
    let media_shortcut = std::rc::Rc::new(std::cell::RefCell::new(settings.media_shortcut.clone()));
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    for (label, shortcut, default) in [
        ("Dictation", shortcut.clone(), "Ctrl+Shift+Space"),
        ("Ask About Screen", ask_shortcut.clone(), "Ctrl+Alt+Space"),
        ("Transcribe media", media_shortcut.clone(), "Ctrl+Shift+T"),
    ] {
        if wayland {
            let trigger = gtk::Label::new(None);
            row(&shortcuts, label, &trigger);
            let id = match label {
                "Dictation" => super::SHORTCUT_ID,
                "Ask About Screen" => "ask",
                _ => "media",
            };
            let enabled = !shortcut.borrow().is_empty();
            gtk::glib::timeout_add_local(std::time::Duration::from_millis(250), {
                let trigger = trigger.downgrade();
                move || {
                    let Some(trigger) = trigger.upgrade() else {
                        return gtk::glib::ControlFlow::Break;
                    };
                    if let Ok(status) = super::SHORTCUT_STATUS.read() {
                        let assigned = status["triggers"][id]
                            .as_str()
                            .filter(|s| !s.trim().is_empty());
                        trigger.set_text(if !enabled {
                            "Disabled"
                        } else {
                            assigned.unwrap_or("Not assigned")
                        });
                    }
                    gtk::glib::ControlFlow::Continue
                }
            });
        } else {
            let current = shortcut.borrow().clone();
            let button = gtk::Button::with_label(if current.is_empty() {
                "Disabled"
            } else {
                &current
            });
            button.set_hexpand(true);
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            line.append(&button);
            row_for(&shortcuts, label, &line, &button);
            let capture_error = gtk::Label::new(None);
            capture_error.set_wrap(true);
            capture_error.set_xalign(0.0);
            capture_error.add_css_class("error");
            capture_error.set_visible(false);
            capture_error.connect_label_notify(|error| error.set_visible(!error.text().is_empty()));
            shortcuts.append(&capture_error);
            let capturing = std::rc::Rc::new(std::cell::Cell::new(false));
            let capture_generation = std::rc::Rc::new(std::cell::Cell::new(0));
            button.connect_clicked({
                let capturing = capturing.clone();
                let capture_generation = capture_generation.clone();
                let shortcut = shortcut.clone();
                let capture_error = capture_error.clone();
                move |button| {
                    let (generation, receiver) = match super::begin_shortcut_capture() {
                        Ok(receiver) => receiver,
                        Err(error) => {
                            capture_error.set_text(&error);
                            return;
                        }
                    };
                    capturing.set(true);
                    capture_generation.set(generation);
                    button.set_label("Press your shortcut…");
                    button.grab_focus();
                    gtk::glib::timeout_add_local(std::time::Duration::from_millis(16), {
                        let button = button.downgrade();
                        let capturing = capturing.clone();
                        let capture_generation = capture_generation.clone();
                        let shortcut = shortcut.clone();
                        let capture_error = capture_error.downgrade();
                        move || {
                            let Some(button) = button.upgrade() else {
                                return gtk::glib::ControlFlow::Break;
                            };
                            if super::SHORTCUT_CAPTURE_GENERATION
                                .load(std::sync::atomic::Ordering::Acquire)
                                != generation
                            {
                                if capture_generation.get() == generation {
                                    capturing.set(false);
                                    button.set_label(&shortcut.borrow());
                                }
                                return gtk::glib::ControlFlow::Break;
                            }
                            if !capturing.get() {
                                return gtk::glib::ControlFlow::Break;
                            }
                            match receiver.try_recv() {
                                Ok(value) => {
                                    *shortcut.borrow_mut() = value.clone();
                                    button.set_label(&value);
                                    capturing.set(false);
                                    super::end_shortcut_capture();
                                    if let Some(error) = capture_error.upgrade() {
                                        error.set_text("");
                                    }
                                    gtk::glib::ControlFlow::Break
                                }
                                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                    capturing.set(false);
                                    button.set_label(&shortcut.borrow());
                                    gtk::glib::ControlFlow::Break
                                }
                                Err(std::sync::mpsc::TryRecvError::Empty) => {
                                    gtk::glib::ControlFlow::Continue
                                }
                            }
                        }
                    });
                }
            });
            let keys = gtk::EventControllerKey::new();
            keys.set_propagation_phase(gtk::PropagationPhase::Capture);
            keys.connect_key_pressed({
                let button = button.downgrade();
                let capturing = capturing.clone();
                let capture_generation = capture_generation.clone();
                let shortcut = shortcut.clone();
                move |_, key, _, modifiers| {
                    let Some(button) = button.upgrade() else {
                        return gtk::glib::Propagation::Proceed;
                    };
                    if !capturing.get()
                        || capture_generation.get()
                            != super::SHORTCUT_CAPTURE_GENERATION
                                .load(std::sync::atomic::Ordering::Acquire)
                    {
                        return gtk::glib::Propagation::Proceed;
                    }
                    if key == gtk::gdk::Key::Escape {
                        capturing.set(false);
                        super::end_shortcut_capture();
                        button.set_label(&shortcut.borrow());
                        return gtk::glib::Propagation::Stop;
                    }
                    if matches!(
                        key,
                        gtk::gdk::Key::Control_L
                            | gtk::gdk::Key::Control_R
                            | gtk::gdk::Key::Alt_L
                            | gtk::gdk::Key::Alt_R
                            | gtk::gdk::Key::Shift_L
                            | gtk::gdk::Key::Shift_R
                            | gtk::gdk::Key::Super_L
                            | gtk::gdk::Key::Super_R
                    ) {
                        return gtk::glib::Propagation::Stop;
                    }
                    match captured_shortcut(key, modifiers) {
                        Ok(value) => {
                            *shortcut.borrow_mut() = value.to_string();
                            capturing.set(false);
                            super::end_shortcut_capture();
                            button.set_label(&value.to_string());
                            capture_error.set_text("");
                        }
                        Err(error) => {
                            capturing.set(false);
                            super::end_shortcut_capture();
                            button.set_label(&shortcut.borrow());
                            capture_error.set_text(&error);
                        }
                    }
                    gtk::glib::Propagation::Stop
                }
            });
            button.add_controller(keys);
            {
                for (title, value) in [("Clear", ""), ("Reset", default)] {
                    let reset = gtk::Button::with_label(title);
                    let shortcut = shortcut.clone();
                    let button = button.clone();
                    let capturing = capturing.clone();
                    reset.connect_clicked(move |_| {
                        capturing.set(false);
                        super::end_shortcut_capture();
                        *shortcut.borrow_mut() = value.to_string();
                        button.set_label(if value.is_empty() { "Disabled" } else { value });
                    });
                    super::menu::set_accessible_label(
                        reset.upcast_ref(),
                        &format!("{title} {label} shortcut"),
                    );
                    line.append(&reset);
                }
            }
        }
    }
    if wayland {
        let portal = gtk::Button::with_label("Change desktop shortcut…");
        portal.set_halign(gtk::Align::Start);
        shortcuts.append(&portal);
        portal.connect_clicked(|_| {
            std::thread::spawn(|| {
                let result = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|error| error.to_string())
                    .and_then(|runtime| runtime.block_on(super::configure_shortcut()));
                if let Err(error) = result {
                    super::show_desktop_message(&error);
                }
            });
        });
        let setup = gtk::Expander::new(Some("Use desktop shortcuts instead"));
        let instructions = gtk::Label::new(Some(
            "Bind these commands in your desktop shortcut settings.

Toggle: koett toggle
Hold press: koett start
Hold release: koett stop
Ask toggle: koett ask
Ask press: koett ask-start
Ask release: koett ask-stop
Media: open Koett → Transcribe media",
        ));
        instructions.set_wrap(true);
        instructions.set_xalign(0.0);
        instructions.set_selectable(true);
        setup.set_child(Some(&instructions));
        shortcuts.append(&setup);
    }
    window.connect_close_request(|_| {
        super::end_shortcut_capture();
        gtk::glib::Propagation::Proceed
    });
    let paste = gtk::ComboBoxText::new();
    paste.append(Some("ctrl_v"), "Ctrl+V");
    paste.append(Some("ctrl_shift_v"), "Ctrl+Shift+V");
    paste.set_active_id(Some(match settings.paste_shortcut {
        PasteShortcut::CtrlV => "ctrl_v",
        PasteShortcut::CtrlShiftV => "ctrl_shift_v",
    }));
    row(&recording, "Paste shortcut", &paste);
    let pause_media = gtk::CheckButton::with_label("_Pause media while recording");
    pause_media.set_use_underline(true);
    pause_media.set_active(settings.pause_media);
    recording.append(&pause_media);
    let drawing = group(&draw_page, "Drawing");
    let draw = gtk::CheckButton::with_label("_Draw on screen while dictating");
    draw.set_use_underline(true);
    draw.set_active(settings.draw_while_dictating);
    drawing.append(&draw);
    drawing.append(&help(
        "Hold the draw key and move the pointer to sketch. Sketches paste as one numbered image, then your text. Works on KDE, niri, Sway and Hyprland; KDE asks once for screenshot permission.",
    ));
    let draw_options = gtk::Box::new(gtk::Orientation::Vertical, 8);
    draw_options.add_css_class("koett-setting-nested");
    drawing.append(&draw_options);
    let draw_key = gtk::ComboBoxText::new();
    draw_key.append(Some("shift"), "Shift");
    draw_key.append(Some("control"), "Ctrl");
    draw_key.append(Some("alt"), "Alt");
    draw_key.set_active_id(Some(match settings.draw_key {
        DrawKey::Shift => "shift",
        DrawKey::Control => "control",
        DrawKey::Alt => "alt",
    }));
    row(&draw_options, "Draw key", &draw_key);
    draw_options.append(&help(
        "Typing keeps working while you record. To see the draw key, Koett checks physical keyboards for Shift, Ctrl, Alt, Super and whether another key is down, only while a drawing dictation records. It never reads typed text.",
    ));
    let (access_text, access_ready) = match super::sketch::keys::Keyboards::open() {
        Ok(keyboards) if keyboards.unreadable() > 0 => (
            format!(
                "Keyboard access: partial. {} keyboard(s) can't be read. Run the setup command, then log out and back in.",
                keyboards.unreadable()
            ),
            false,
        ),
        Ok(_) => ("Keyboard access: ready.".to_string(), true),
        Err(super::sketch::keys::Access::Denied) => (
            "Keyboard access: needs a one-time setup, and drawing stays off until then. Run the setup command in a terminal, then log out and back in. It gives your user read-only access to keyboards, which other apps you run could also use.".to_string(),
            false,
        ),
        Err(super::sketch::keys::Access::NoKeyboards) => {
            ("Keyboard access: no keyboard found.".to_string(), false)
        }
    };
    let access = gtk::Label::new(Some(&access_text));
    access.set_wrap(true);
    access.set_xalign(0.0);
    draw_options.append(&access);
    let setup = gtk::Button::with_label("Copy setup command");
    setup.set_halign(gtk::Align::Start);
    setup.set_tooltip_text(Some(super::sketch::keys::SETUP_COMMAND));
    setup.set_visible(!access_ready);
    setup.connect_clicked(|button| {
        button
            .clipboard()
            .set_text(super::sketch::keys::SETUP_COMMAND);
        button.set_label("Copied · paste into a terminal");
    });
    draw_options.append(&setup);
    let may_take = gtk::CheckButton::with_label(
        "_If keys can't be read, let drawing take the keyboard (typing pauses while recording)",
    );
    may_take.set_use_underline(true);
    may_take.set_active(settings.draw_may_take_keyboard);
    may_take.set_visible(!access_ready);
    draw_options.append(&may_take);
    let sync_draw = {
        let draw = draw.clone();
        let draw_options = draw_options.clone();
        move || draw_options.set_sensitive(draw.is_active())
    };
    sync_draw();
    draw.connect_toggled(move |_| sync_draw());
    replacements.append(&help(
        "Case-sensitive phrases. Longest match wins; replacements run once.",
    ));
    let pairs = gtk::Box::new(gtk::Orientation::Vertical, 8);
    replacements.append(&pairs);
    let entries = Rc::new(std::cell::RefCell::new(Vec::<(
        gtk::Entry,
        gtk::Entry,
        gtk::Box,
    )>::new()));
    for rule in &settings.replacements {
        add_pair(&pairs, &entries, &rule.phrase, &rule.replacement);
    }
    let add = gtk::Button::with_label("Add replacement");
    add.set_halign(gtk::Align::Start);
    replacements.append(&add);
    add.connect_clicked({
        let entries = entries.clone();
        let pairs = pairs.clone();
        move |_| add_pair(&pairs, &entries, "", "")
    });
    let behavior = group(&general, "Startup");
    let login = gtk::CheckButton::with_label("_Start at login");
    login.set_use_underline(true);
    login.set_active(settings.start_at_login);
    behavior.append(&login);
    let about = group(&general, "About");
    let version = gtk::Label::new(Some(&format!("Koett {}", env!("CARGO_PKG_VERSION"))));
    version.set_xalign(0.0);
    version.set_hexpand(true);
    let about_button = gtk::Button::with_label("About Koett…");
    about_button.connect_clicked({
        let window = window.clone();
        move |_| about_dialog(window.upcast_ref())
    });
    let about_line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    about_line.add_css_class("koett-setting-row");
    about_line.append(&version);
    about_line.append(&about_button);
    about.append(&about_line);
    let error = gtk::Label::new(None);
    error.set_wrap(true);
    error.set_hexpand(true);
    error.set_xalign(0.0);
    error.set_visible(false);
    error.connect_label_notify(|error| error.set_visible(!error.text().is_empty()));
    error.add_css_class("error");
    footer.append(&error);
    let save = gtk::Button::with_label("Save");
    save.add_css_class("koett-save");
    save.set_hexpand(true);
    save.set_halign(gtk::Align::End);
    footer.append(&save);
    save.connect_clicked({
        let window = window.clone();
        move |_| {
            let mut updated = settings.clone();
            updated.shortcut = shortcut.borrow().clone();
            updated.ask_shortcut = ask_shortcut.borrow().clone();
            updated.media_shortcut = media_shortcut.borrow().clone();
            updated.start_at_login = login.is_active();
            updated.microphone = input
                .active_id()
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
            updated.recording_mode = if mode.active_id().as_deref() == Some("hold") {
                RecordingMode::Hold
            } else {
                RecordingMode::Toggle
            };
            let mut rules = Vec::new();
            for (phrase, replacement, container) in entries.borrow().iter() {
                if container.parent().is_none() {
                    continue;
                }
                if phrase.text().trim().is_empty() {
                    error.set_text("Enter a phrase or remove the empty replacement.");
                    stack.set_visible_child_name("words");
                    phrase.grab_focus();
                    return;
                }
                rules.push(Replacement {
                    phrase: phrase.text().to_string(),
                    replacement: replacement.text().to_string(),
                });
            }
            updated.replacements = rules;
            updated.dictionary = match dictionary.entries() {
                Ok(entries) => entries,
                Err(message) => {
                    error.set_text(&message);
                    stack.set_visible_child_name("words");
                    return;
                }
            };
            updated.paste_shortcut = match paste.active_id().as_deref() {
                Some("ctrl_v") => PasteShortcut::CtrlV,
                Some("ctrl_shift_v") => PasteShortcut::CtrlShiftV,
                _ => {
                    error.set_text("Choose a paste shortcut.");
                    stack.set_visible_child_name("audio");
                    paste.grab_focus();
                    return;
                }
            };
            updated.pause_media = pause_media.is_active();
            updated.draw_while_dictating = draw.is_active();
            updated.draw_may_take_keyboard = may_take.is_active();
            updated.draw_key = match draw_key.active_id().as_deref() {
                Some("control") => DrawKey::Control,
                Some("alt") => DrawKey::Alt,
                _ => DrawKey::Shift,
            };
            let mut edit = match Settings::edit() {
                Ok(edit) => edit,
                Err(message) => {
                    error.set_text(&message);
                    return;
                }
            };
            let updated = updated.merge_editor(&settings, edit.settings.clone());
            if let Err(message) = updated.validate_shortcuts() {
                error.set_text(&message);
                return;
            }
            let previous = super::menu_preferences::login_registered();
            edit.settings = updated.clone();
            let result = super::set_start_at_login(updated.start_at_login).and_then(|()| {
                edit.commit().inspect_err(|_| {
                    let _ = super::set_start_at_login(previous);
                })
            });
            drop(edit);
            match result {
                Ok(()) => {
                    on_saved(&updated);
                    window.close();
                }
                Err(message) => error.set_text(&message),
            }
        }
    });
    super::window::present(&window, token);
}
fn section(stack: &gtk::Stack, name: &str, title: &str) -> gtk::Box {
    let layout = gtk::Box::new(gtk::Orientation::Vertical, 8);
    layout.add_css_class("koett-settings-page");
    layout.set_valign(gtk::Align::Start);
    let heading = gtk::Label::new(Some(title));
    heading.set_xalign(0.0);
    heading.add_css_class("koett-settings-heading");
    layout.append(&heading);
    let scroll = gtk::ScrolledWindow::builder()
        .child(&layout)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    stack.add_titled(&scroll, Some(name), title);
    layout
}

// GtkListBox moves row by row on Tab and the sidebar selects the row it reaches,
// so without this Tab walks every page and always lands on the last one.
fn tab_past_sidebar(
    sidebar: &gtk::StackSidebar,
    stack: &gtk::Stack,
    header: &gtk::HeaderBar,
    footer: &gtk::Box,
) {
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let stack = stack.clone();
    let header = header.clone();
    let footer = footer.clone();
    keys.connect_key_pressed(move |_, key, _, modifiers| {
        if !matches!(
            key,
            gtk::gdk::Key::Tab | gtk::gdk::Key::KP_Tab | gtk::gdk::Key::ISO_Left_Tab
        ) {
            return gtk::glib::Propagation::Proceed;
        }
        if modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK) {
            header.child_focus(gtk::DirectionType::TabBackward);
        } else if !stack
            .visible_child()
            .is_some_and(|page| page.child_focus(gtk::DirectionType::TabForward))
        {
            footer.child_focus(gtk::DirectionType::TabForward);
        }
        gtk::glib::Propagation::Stop
    });
    sidebar.add_controller(keys);
}

fn group(page: &gtk::Box, title: &str) -> gtk::Box {
    let heading = gtk::Label::new(Some(title));
    heading.set_xalign(0.0);
    heading.add_css_class("koett-settings-group-title");
    page.append(&heading);
    let group = gtk::Box::new(gtk::Orientation::Vertical, 8);
    group.add_css_class("koett-settings-group");
    page.append(&group);
    group
}

fn help(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_wrap(true);
    label.set_xalign(0.0);
    label.add_css_class("koett-setting-help");
    label
}

fn find_stack(widget: &gtk::Widget) -> Option<gtk::Stack> {
    if widget.widget_name() == STACK_NAME {
        return widget.clone().downcast().ok();
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        if let Some(stack) = find_stack(&widget) {
            return Some(stack);
        }
        child = widget.next_sibling();
    }
    None
}

pub(super) fn about_dialog(parent: &gtk::Window) {
    gtk::AboutDialog::builder()
        .transient_for(parent)
        .modal(true)
        .program_name("Koett")
        .version(env!("CARGO_PKG_VERSION"))
        .license_type(gtk::License::Apache20)
        .build()
        .present();
}

fn row(layout: &gtk::Box, label: &str, widget: &impl IsA<gtk::Widget>) {
    row_for(layout, label, widget, widget);
}

fn row_for(
    layout: &gtk::Box,
    label: &str,
    widget: &impl IsA<gtk::Widget>,
    target: &impl IsA<gtk::Widget>,
) {
    let title = gtk::Label::new(Some(label));
    title.set_xalign(0.0);
    title.set_width_chars(12);
    if !target.as_ref().is::<gtk::Label>() {
        title.set_mnemonic_widget(Some(target));
    }
    let group = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    group.add_css_class("koett-setting-row");
    title.add_css_class("koett-setting-label");
    widget.set_hexpand(true);
    group.append(&title);
    group.append(widget);
    layout.append(&group);
}
type Pairs = std::rc::Rc<std::cell::RefCell<Vec<(gtk::Entry, gtk::Entry, gtk::Box)>>>;
fn add_pair(pairs: &gtk::Box, entries: &Pairs, phrase: &str, replacement: &str) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.add_css_class("koett-setting-row");
    let from = gtk::Entry::builder()
        .placeholder_text("Phrase")
        .text(phrase)
        .hexpand(true)
        .build();
    let to = gtk::Entry::builder()
        .placeholder_text("Replacement")
        .text(replacement)
        .hexpand(true)
        .build();
    from.set_width_chars(8);
    to.set_width_chars(8);
    let remove = gtk::Button::with_label("Remove");
    remove.set_valign(gtk::Align::End);
    let from_group = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let from_label = gtk::Label::new(Some("Phrase"));
    from_label.set_xalign(0.0);
    from_label.set_mnemonic_widget(Some(&from));
    from_group.append(&from_label);
    from_group.append(&from);
    let to_group = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let to_label = gtk::Label::new(Some("Replacement"));
    to_label.set_xalign(0.0);
    to_label.set_mnemonic_widget(Some(&to));
    to_group.append(&to_label);
    to_group.append(&to);
    row.append(&from_group);
    row.append(&to_group);
    row.append(&remove);
    pairs.append(&row);
    remove.connect_clicked({
        let row = row.clone();
        let pairs = pairs.clone();
        move |_| pairs.remove(&row)
    });
    entries.borrow_mut().push((from, to, row));
}

fn captured_shortcut(
    key: gtk::gdk::Key,
    modifiers: gtk::gdk::ModifierType,
) -> Result<crate::shortcut::Shortcut, String> {
    let mut parts = Vec::new();
    if modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
        parts.push("Ctrl".to_string());
    }
    if modifiers.contains(gtk::gdk::ModifierType::ALT_MASK) {
        parts.push("Alt".to_string());
    }
    if modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK) {
        parts.push("Shift".to_string());
    }
    if modifiers.contains(gtk::gdk::ModifierType::SUPER_MASK) {
        parts.push("Super".to_string());
    }
    let name = key.name().ok_or("Unsupported key")?;
    parts.push(name.to_string());
    parts.join("+").parse()
}

#[cfg(test)]
mod shortcut_capture_tests {
    use super::*;
    #[test]
    fn native_chord_capture_uses_the_existing_shortcut_contract() {
        assert_eq!(
            captured_shortcut(
                gtk::gdk::Key::space,
                gtk::gdk::ModifierType::CONTROL_MASK | gtk::gdk::ModifierType::ALT_MASK
            )
            .unwrap()
            .to_string(),
            "Ctrl+Alt+Space"
        );
        assert!(captured_shortcut(gtk::gdk::Key::a, gtk::gdk::ModifierType::empty()).is_err());
    }
}
