use crate::replacements::Replacement;
use crate::settings::{PasteShortcut, RecordingMode, Settings};
use gtk4::{self as gtk, prelude::*};

pub fn show(
    parent: &gtk::ApplicationWindow,
    on_saved: std::rc::Rc<dyn Fn(&Settings)>,
    token: Option<&str>,
) {
    if let Some(window) = parent.application().and_then(|app| {
        app.windows()
            .into_iter()
            .find(|window| window.title().as_deref() == Some("Settings"))
    }) {
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
        .default_width(420)
        .default_height(380)
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
    let stack = gtk::Stack::new();
    stack.set_vexpand(true);
    let navigation = gtk::ComboBoxText::new();
    for (id, label) in [
        ("audio", "Microphone"),
        ("shortcuts", "Recording & shortcuts"),
        ("text", "Text & replacements"),
        ("general", "Koett"),
        ("ask", "Ask"),
    ] {
        navigation.append(Some(id), label);
    }
    super::menu::set_accessible_label(navigation.upcast_ref(), "Settings section");
    navigation.set_active_id(Some("audio"));
    navigation.set_margin_start(16);
    navigation.set_margin_end(16);
    navigation.set_margin_top(12);
    navigation.connect_changed({
        let stack = stack.clone();
        move |choice| {
            if let Some(id) = choice.active_id() {
                stack.set_visible_child_name(&id);
            }
        }
    });
    outer.append(&navigation);
    outer.append(&stack);
    let audio = section(&stack, "audio", "Audio");
    let shortcuts = section(&stack, "shortcuts", "Shortcuts");
    let shortcut_error = gtk::Label::new(None);
    shortcut_error.set_wrap(true);
    shortcut_error.set_selectable(true);
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
    let text = section(&stack, "text", "Text");
    let general = section(&stack, "general", "General");
    let ask = section(&stack, "ask", "Ask");
    super::ask_settings::build(&ask);
    stack.set_visible_child_name("audio");
    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    footer.add_css_class("koett-settings-footer");
    outer.append(&footer);
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
    row(&audio, "Microphone", &input);
    super::input_preview::attach(&window, &input, &audio, &stack);
    let mode = gtk::ComboBoxText::new();
    mode.append(Some("toggle"), "Toggle");
    mode.append(Some("hold"), "Hold to record");
    mode.set_active_id(Some(if settings.recording_mode == RecordingMode::Hold {
        "hold"
    } else {
        "toggle"
    }));
    row(&shortcuts, "Recording mode", &mode);
    let shortcut = std::rc::Rc::new(std::cell::RefCell::new(settings.shortcut.clone()));
    let ask_shortcut = std::rc::Rc::new(std::cell::RefCell::new(settings.ask_shortcut.clone()));
    let media_shortcut = std::rc::Rc::new(std::cell::RefCell::new(settings.media_shortcut.clone()));
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    for (label, shortcut, default) in [
        ("Dictation", shortcut.clone(), "Ctrl+Shift+Space"),
        ("Ask", ask_shortcut.clone(), "Ctrl+Alt+Space"),
        ("Media", media_shortcut.clone(), "Ctrl+Shift+T"),
    ] {
        if wayland {
            let trigger = gtk::Label::new(None);
            row(&shortcuts, label, &trigger);
            let id = match label {
                "Dictation" => super::SHORTCUT_ID,
                "Ask" => "ask",
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
            let button = gtk::Button::with_label(&shortcut.borrow());
            row(&shortcuts, label, &button);
            let capture_error = gtk::Label::new(None);
            capture_error.set_wrap(true);
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
                let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
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
                    actions.append(&reset);
                }
                shortcuts.append(&actions);
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
        let setup = gtk::Expander::new(Some("Compositor shortcut setup"));
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
    row(&text, "Paste shortcut", &paste);
    let replacements_title = gtk::Label::new(Some("Word replacements"));
    replacements_title.set_xalign(0.0);
    replacements_title.add_css_class("heading");
    text.append(&replacements_title);
    let explanation = gtk::Label::new(Some(
        "Case-sensitive phrases. Longest match wins; replacements run once.",
    ));
    explanation.set_wrap(true);
    explanation.set_xalign(0.0);
    explanation.add_css_class("koett-setting-help");
    text.append(&explanation);
    let pairs = gtk::Box::new(gtk::Orientation::Vertical, 8);
    text.append(&pairs);
    let entries = std::rc::Rc::new(std::cell::RefCell::new(Vec::<(
        gtk::Entry,
        gtk::Entry,
        gtk::Box,
    )>::new()));
    for rule in &settings.replacements {
        add_pair(&pairs, &entries, &rule.phrase, &rule.replacement);
    }
    let add = gtk::Button::with_label("Add replacement");
    add.set_halign(gtk::Align::Start);
    text.append(&add);
    add.connect_clicked({
        let entries = entries.clone();
        let pairs = pairs.clone();
        move |_| add_pair(&pairs, &entries, "", "")
    });
    let login = gtk::CheckButton::with_label("_Launch at login");
    login.set_use_underline(true);
    login.set_active(settings.start_at_login);
    let behavior = gtk::Box::new(gtk::Orientation::Vertical, 12);
    behavior.add_css_class("koett-setting-row");
    general.append(&behavior);
    behavior.append(&login);
    let pause_media = gtk::CheckButton::with_label("_Pause media while recording");
    pause_media.set_use_underline(true);
    pause_media.set_active(settings.pause_media);
    behavior.append(&pause_media);
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
                    stack.set_visible_child_name("text");
                    phrase.grab_focus();
                    return;
                }
                rules.push(Replacement {
                    phrase: phrase.text().to_string(),
                    replacement: replacement.text().to_string(),
                });
            }
            updated.replacements = rules;
            updated.paste_shortcut = match paste.active_id().as_deref() {
                Some("ctrl_v") => PasteShortcut::CtrlV,
                Some("ctrl_shift_v") => PasteShortcut::CtrlShiftV,
                _ => {
                    error.set_text("Choose a paste shortcut.");
                    stack.set_visible_child_name("text");
                    paste.grab_focus();
                    return;
                }
            };
            updated.pause_media = pause_media.is_active();
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
    let layout = gtk::Box::new(gtk::Orientation::Vertical, 12);
    layout.add_css_class("koett-settings-page");
    layout.set_valign(gtk::Align::Start);
    let scroll = gtk::ScrolledWindow::builder()
        .child(&layout)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    stack.add_titled(&scroll, Some(name), title);
    layout
}
fn row(layout: &gtk::Box, label: &str, widget: &impl IsA<gtk::Widget>) {
    let title = gtk::Label::new(Some(label));
    title.set_xalign(1.0);
    title.set_width_chars(12);
    if !widget.as_ref().is::<gtk::Label>() {
        title.set_mnemonic_widget(Some(widget));
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
