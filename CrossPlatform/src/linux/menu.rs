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
            &[("Ask", "app.ask"), ("Cancel Ask", "app.cancel-ask")][..],
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
    preferences.append(Some("Settings…"), Some("app.settings"));
    preferences.append(Some("About Koett…"), Some("app.about"));
    model.append_section(None, &preferences);
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
