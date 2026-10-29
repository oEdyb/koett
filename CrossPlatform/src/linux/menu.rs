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
    for items in [
        &[
            ("Transcribe media…", "app.media"),
            ("History", "app.history"),
            ("Copy last transcript", "app.copy-last"),
            ("Failed recordings…", "app.failed-recordings"),
        ][..],
        &[
            ("Local models…", "app.models"),
            ("Formatting…", "app.formatting"),
            ("Settings…", "app.settings"),
        ][..],
        &[("Hide window", "app.hide"), ("Quit Koett", "app.quit")][..],
    ] {
        let section = gtk::gio::Menu::new();
        for (label, action) in items {
            section.append(Some(label), Some(action));
        }
        model.append_section(None, &section);
    }
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
