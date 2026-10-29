use gtk4::{self as gtk, glib::translate::ToGlibPtr, prelude::*};

// Present in GTK 4.6; gtk-rs 0.11 gates its Accessible wrappers on 4.10.
// https://gitlab.gnome.org/GNOME/gtk/-/blob/4.6.9/gtk/gtkaccessible.h
unsafe extern "C" {
    fn gtk_accessible_reset_relation(
        accessible: *mut gtk::ffi::GtkAccessible,
        relation: gtk::ffi::GtkAccessibleRelation,
    );
    fn gtk_accessible_update_property_value(
        accessible: *mut gtk::ffi::GtkAccessible,
        count: i32,
        properties: *mut gtk::ffi::GtkAccessibleProperty,
        values: *const gtk::glib::gobject_ffi::GValue,
    );
}

pub(super) struct RecordingAction {
    pub label: &'static str,
    pub enabled: bool,
}

pub(super) fn recording_action(status: &str) -> RecordingAction {
    let (label, enabled) = if status.starts_with("Recording") {
        ("Stop", true)
    } else if status == "Ready" {
        ("Record", true)
    } else if status.starts_with("Error:") {
        ("Retry", true)
    } else if status.starts_with("Transcribing") {
        ("Transcribing…", false)
    } else {
        ("Starting…", false)
    };
    RecordingAction { label, enabled }
}

pub(super) fn is_control_message(message: &str) -> bool {
    message.starts_with('@')
}

pub(super) fn button() -> gtk::MenuButton {
    let model = gtk::gio::Menu::new();
    for (title, items) in [
        (
            "Ask",
            &[
                ("Ask", "app.ask"),
                ("Cancel Ask", "app.cancel-ask"),
                ("Settings…", "app.settings"),
            ][..],
        ),
        (
            "Transcripts",
            &[
                ("Transcribe media…", "app.media"),
                ("History", "app.history"),
                ("Copy last transcript", "app.copy-last"),
                ("Paste last transcript", "app.paste-last"),
            ][..],
        ),
        (
            "Recovery",
            &[
                ("Copy unsaved transcripts", "app.copy-unsaved"),
                ("Retry Save", "app.retry-unsaved"),
                ("Recovery status", "app.recovery-status"),
                ("Media transcripts…", "app.media-transcripts"),
                ("Failed transcripts…", "app.failed-transcripts"),
                ("Failed recordings…", "app.failed-recordings"),
            ][..],
        ),
    ] {
        let submenu = gtk::gio::Menu::new();
        for (label, action) in items {
            submenu.append(Some(label), Some(action));
        }
        if title == "Ask" {
            let submenu = submenu.clone();
            let mut previous = "Ask";
            gtk::glib::timeout_add_local(std::time::Duration::from_millis(250), move || {
                let label = super::menu_preferences::ask_state().0;
                if label != previous {
                    previous = label;
                    submenu.remove(0);
                    submenu.insert(0, Some(label), Some("app.ask"));
                }
                gtk::glib::ControlFlow::Continue
            });
        }
        model.append_submenu(Some(title), &submenu);
    }
    let preferences = gtk::gio::Menu::new();
    let models = gtk::gio::Menu::new();
    for model in crate::model::LocalModel::ALL {
        models.append(
            Some(model.label()),
            Some(&format!("app.menu-model::{}", model.id())),
        );
    }
    preferences.append_submenu(Some("Model…"), &models);
    gtk::glib::timeout_add_local(std::time::Duration::from_millis(500), {
        let preferences = preferences.clone();
        let mut previous = String::new();
        move || {
            let label = super::menu_preferences::active_label();
            if previous == label {
                return gtk::glib::ControlFlow::Continue;
            }
            previous = label;
            preferences.remove(0);
            preferences.insert_submenu(
                0,
                Some(&format!(
                    "Model: {}…",
                    super::menu_preferences::active_label()
                )),
                &models,
            );
            gtk::glib::ControlFlow::Continue
        }
    });
    let format = gtk::gio::Menu::new();
    format.append(Some("Raw"), Some("app.menu-format::raw"));
    format.append(Some("S1-mini"), Some("app.menu-format::s1-mini"));
    format.append(Some("Formatting controls…"), Some("app.formatting"));
    preferences.append_submenu(Some("Formatting…"), &format);

    let microphones = gtk::gio::Menu::new();
    preferences.append_submenu(Some("Microphone"), &microphones);
    let mut previous = String::new();
    gtk::glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
        let inputs = super::menu_preferences::INPUTS.read().ok();
        let selected = crate::settings::Settings::load()
            .ok()
            .and_then(|s| s.microphone);
        let signature = format!("{inputs:?}{selected:?}");
        if signature != previous {
            previous = signature;
            microphones.remove_all();
            let append = |label: &str, id: &str| {
                let item = gtk::gio::MenuItem::new(Some(label), None);
                item.set_action_and_target_value(
                    Some("app.menu-microphone"),
                    Some(&id.to_variant()),
                );
                microphones.append_item(&item);
            };
            append("System default", "default");
            if let Some(inputs) = inputs.as_deref() {
                match inputs {
                    Ok(inputs) => {
                        for input in inputs {
                            append(&input.label, &input.id);
                        }
                        if let Some(selected) = &selected
                            && !inputs.iter().any(|d| &d.id == selected)
                        {
                            append("Saved microphone unavailable", selected);
                        }
                    }
                    Err(message) => microphones.append(Some(message), None),
                }
            }
        }
        gtk::glib::ControlFlow::Continue
    });
    let mode = gtk::gio::Menu::new();
    mode.append(Some("Toggle"), Some("app.menu-mode::toggle"));
    mode.append(Some("Hold"), Some("app.menu-mode::hold"));
    preferences.append_submenu(Some("Recording mode"), &mode);
    preferences.append(Some("Start at login"), Some("app.menu-login"));
    preferences.append(Some("Settings / Shortcuts…"), Some("app.settings"));
    model.append_submenu(Some("Preferences"), &preferences);
    model.append(Some("About Koett…"), Some("app.menu-about"));
    let section = gtk::gio::Menu::new();
    section.append(Some("Hide window"), Some("app.hide"));
    section.append(Some("Quit Koett"), Some("app.quit"));
    model.append_section(None, &section);
    let button = gtk::MenuButton::builder()
        .label("Menu")
        .menu_model(&model)
        .build();
    if let Some(popover) = button.popover() {
        popover.connect_map(|popover| {
            let popover = popover.downgrade();
            gtk::glib::idle_add_local_once(move || {
                if let Some(popover) = popover.upgrade() {
                    name_menu_items(popover.upcast_ref());
                }
            });
        });
    }
    button
}

fn name_menu_items(widget: &gtk::Widget) {
    if widget.property::<gtk::AccessibleRole>("accessible-role") == gtk::AccessibleRole::MenuItem
        && let Some(label) = menu_label(widget)
    {
        set_accessible_label(widget, &label);
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        name_menu_items(&widget);
        child = widget.next_sibling();
    }
}

pub(super) fn set_accessible_label(widget: &gtk::Widget, label: &str) {
    let mut property = gtk::ffi::GTK_ACCESSIBLE_PROPERTY_LABEL;
    let value = label.to_value();
    unsafe {
        gtk_accessible_reset_relation(
            widget.as_ptr().cast(),
            gtk::ffi::GTK_ACCESSIBLE_RELATION_LABELLED_BY,
        );
        gtk_accessible_update_property_value(
            widget.as_ptr().cast(),
            1,
            &raw mut property,
            value.to_glib_none().0,
        );
    }
}

fn menu_label(widget: &gtk::Widget) -> Option<gtk::glib::GString> {
    if let Some(label) = widget.downcast_ref::<gtk::Label>() {
        return Some(label.label());
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        if let Some(label) = menu_label(&widget) {
            return Some(label);
        }
        child = widget.next_sibling();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_actions_match_phase_and_block_busy_capture() {
        for (status, label, enabled) in [
            ("Ready", "Record", true),
            ("Recording…", "Stop", true),
            ("Error: microphone unavailable", "Retry", true),
            ("Starting…", "Starting…", false),
            ("Downloading model… 40%", "Starting…", false),
            ("Loading model…", "Starting…", false),
            ("Transcribing…", "Transcribing…", false),
        ] {
            let action = recording_action(status);
            assert_eq!((action.label, action.enabled), (label, enabled), "{status}");
        }
    }
}
