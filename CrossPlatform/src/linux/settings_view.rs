use crate::replacements::Replacement;
use crate::settings::{RecordingMode, Settings};
use gtk4::{self as gtk, prelude::*};

pub fn show(parent: &gtk::ApplicationWindow, on_saved: std::rc::Rc<dyn Fn(&Settings)>) {
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
        .default_width(460)
        .default_height(600)
        .build();
    window.set_application(parent.application().as_ref());
    window.connect_is_active_notify(|window| {
        super::output::WINDOW_ACTIVE.store(window.is_active(), std::sync::atomic::Ordering::Release)
    });
    let header = gtk::HeaderBar::new();
    header.set_title_widget(Some(&gtk::Label::new(Some("Settings"))));
    window.set_titlebar(Some(&header));
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let layout = gtk::Box::new(gtk::Orientation::Vertical, 16);
    layout.set_margin_top(20);
    layout.set_margin_bottom(20);
    layout.set_margin_start(24);
    layout.set_margin_end(24);
    let scroll = gtk::ScrolledWindow::builder()
        .child(&layout)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    scroll.set_vexpand(true);
    outer.append(&scroll);
    let footer = gtk::Box::new(gtk::Orientation::Vertical, 8);
    footer.set_margin_top(12);
    footer.set_margin_bottom(12);
    footer.set_margin_start(24);
    footer.set_margin_end(24);
    outer.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    outer.append(&footer);
    window.set_child(Some(&outer));
    let input = gtk::ComboBoxText::new();
    input.append(Some(""), "System default microphone");
    input.set_active_id(Some(""));
    if let Some(name) = &settings.microphone {
        input.append(Some(name), name);
        input.set_active_id(Some(name));
    }
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
    group(&layout, "Input");
    row(&layout, "Microphone", &input);
    group(&layout, "Recording");
    let mode = gtk::ComboBoxText::new();
    mode.append(Some("toggle"), "Toggle");
    mode.append(Some("hold"), "Hold to record");
    mode.set_active_id(Some(if settings.recording_mode == RecordingMode::Hold {
        "hold"
    } else {
        "toggle"
    }));
    row(&layout, "Recording mode", &mode);
    let shortcut = std::rc::Rc::new(std::cell::RefCell::new(settings.shortcut.clone()));
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    if wayland {
        row(
            &layout,
            "Shortcut",
            &gtk::Label::new(Some("Managed by desktop")),
        );
        let portal = gtk::Button::with_label("Change desktop shortcut…");
        layout.append(&portal);
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
Hold release: koett stop",
        ));
        instructions.set_wrap(true);
        instructions.set_xalign(0.0);
        instructions.set_selectable(true);
        setup.set_child(Some(&instructions));
        layout.append(&setup);
    } else {
        let button = gtk::Button::with_label(&shortcut.borrow());
        row(&layout, "Shortcut", &button);
        let capture_error = gtk::Label::new(None);
        capture_error.set_wrap(true);
        layout.append(&capture_error);
        let capturing = std::rc::Rc::new(std::cell::Cell::new(false));
        button.connect_clicked({
            let capturing = capturing.clone();
            move |button| {
                capturing.set(true);
                super::SHORTCUT_EDITING.store(true, std::sync::atomic::Ordering::Release);
                button.set_label("Press your shortcut…");
                button.grab_focus();
            }
        });
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed({
            let button = button.clone();
            let capturing = capturing.clone();
            let shortcut = shortcut.clone();
            move |_, key, _, modifiers| {
                if !capturing.get() {
                    return gtk::glib::Propagation::Proceed;
                }
                if key == gtk::gdk::Key::Escape {
                    capturing.set(false);
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
                        button.set_label(&value.to_string());
                        capture_error.set_text("");
                    }
                    Err(error) => capture_error.set_text(&error),
                }
                gtk::glib::Propagation::Stop
            }
        });
        button.add_controller(keys);
    }
    window.connect_close_request(|_| {
        super::SHORTCUT_EDITING.store(false, std::sync::atomic::Ordering::Release);
        gtk::glib::Propagation::Proceed
    });
    group(&layout, "Text");
    let replacements_title = gtk::Label::new(Some("Word replacements"));
    replacements_title.set_xalign(0.0);
    replacements_title.add_css_class("heading");
    layout.append(&replacements_title);
    let explanation = gtk::Label::new(Some(
        "Case-sensitive phrases. Longest match wins; replacements run once.",
    ));
    explanation.set_wrap(true);
    explanation.set_xalign(0.0);
    explanation.add_css_class("dim-label");
    layout.append(&explanation);
    let pairs = gtk::Box::new(gtk::Orientation::Vertical, 8);
    layout.append(&pairs);
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
    layout.append(&add);
    add.connect_clicked({
        let entries = entries.clone();
        let pairs = pairs.clone();
        move |_| add_pair(&pairs, &entries, "", "")
    });
    group(&layout, "General");
    let login = gtk::CheckButton::with_label("_Launch at login");
    login.set_use_underline(true);
    login.set_active(settings.start_at_login);
    layout.append(&login);
    let pause_media = gtk::CheckButton::with_label("_Pause media while recording");
    pause_media.set_use_underline(true);
    pause_media.set_active(settings.pause_media);
    layout.append(&pause_media);
    let error = gtk::Label::new(None);
    error.set_wrap(true);
    error.add_css_class("error");
    footer.append(&error);
    let save = gtk::Button::with_label("Save");
    save.add_css_class("suggested-action");
    save.set_halign(gtk::Align::End);
    footer.append(&save);
    save.connect_clicked({
        let window = window.clone();
        move |_| {
            let mut updated = settings.clone();
            updated.shortcut = shortcut.borrow().clone();
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
                    phrase.grab_focus();
                    return;
                }
                rules.push(Replacement {
                    phrase: phrase.text().to_string(),
                    replacement: replacement.text().to_string(),
                });
            }
            updated.replacements = rules;
            updated.pause_media = pause_media.is_active();
            let result =
                super::set_start_at_login(updated.start_at_login).and_then(|()| updated.save());
            match result {
                Ok(()) => {
                    on_saved(&updated);
                    window.close();
                }
                Err(message) => error.set_text(&message),
            }
        }
    });
    window.present();
}
fn group(layout: &gtk::Box, name: &str) {
    let title = gtk::Label::new(Some(name));
    title.add_css_class("heading");
    title.set_xalign(0.0);
    layout.append(&title);
}
fn row(layout: &gtk::Box, label: &str, widget: &impl IsA<gtk::Widget>) {
    let title = gtk::Label::new(Some(label));
    title.set_xalign(0.0);
    title.set_mnemonic_widget(Some(widget));

    layout.append(&title);
    layout.append(widget);
}
type Pairs = std::rc::Rc<std::cell::RefCell<Vec<(gtk::Entry, gtk::Entry, gtk::Box)>>>;
fn add_pair(pairs: &gtk::Box, entries: &Pairs, phrase: &str, replacement: &str) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
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
    let remove = gtk::Button::with_label("Remove");
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
