use super::TrayAction;
use gtk4::{self as gtk, prelude::*};
use std::{cell::RefCell, rc::Rc, sync::mpsc::Sender};

pub(super) fn show(
    window: gtk::ApplicationWindow,
    actions: Sender<TrayAction>,
    latest_completed: Rc<RefCell<Option<String>>>,
    status: gtk::Label,
) {
    let dialog = gtk::Window::builder()
        .title("Transcribe media")
        .application(
            &window
                .application()
                .expect("media parent belongs to application"),
        )
        .transient_for(&window)
        .modal(true)
        .default_width(360)
        .build();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(16);
    content.set_margin_end(16);
    let entry = gtk::Entry::builder()
        .placeholder_text("HTTP(S) URL")
        .build();
    super::menu::set_accessible_label(entry.upcast_ref(), "Media URL");
    let start = gtk::Button::with_label("Transcribe");
    let cancel = gtk::Button::with_label("Cancel");
    content.append(&entry);
    content.append(&start);
    content.append(&cancel);
    let phase = gtk::Label::new(None);
    phase.set_wrap(true);
    status
        .bind_property("label", &phase, "label")
        .sync_create()
        .build();
    content.append(&phase);
    let result = gtk::TextView::builder()
        .editable(false)
        .wrap_mode(gtk::WrapMode::WordChar)
        .vexpand(true)
        .build();
    let scroll = gtk::ScrolledWindow::builder()
        .min_content_height(120)
        .child(&result)
        .build();
    content.append(&scroll);
    let copy = gtk::Button::with_label("Copy");
    content.append(&copy);
    let latest = latest_completed.clone();
    copy.set_sensitive(latest.borrow().is_some());
    copy.connect_clicked(move |button| {
        if let Some(text) = latest.borrow().as_ref()
            && super::feedback::copy_transcript(&button.clipboard(), text).is_ok()
        {
            button.set_label("Copied");
        }
    });
    gtk::glib::timeout_add_local(std::time::Duration::from_millis(100), {
        let latest = latest_completed.clone();
        let status = status.clone();
        let dialog = dialog.downgrade();
        let start = start.clone();
        let copy = copy.clone();
        move || {
            copy.set_sensitive(latest.borrow().is_some());
            if latest.borrow().is_none() {
                result.buffer().set_text("");
            }
            if dialog.upgrade().is_none() {
                return gtk::glib::ControlFlow::Break;
            }
            if let Some(text) = latest.borrow().as_ref()
                && result
                    .buffer()
                    .text(
                        &result.buffer().start_iter(),
                        &result.buffer().end_iter(),
                        true,
                    )
                    .as_str()
                    != text
            {
                result.buffer().set_text(text);
            }
            start.set_sensitive(
                status.text().starts_with("Ready") || status.text().starts_with("Error"),
            );
            gtk::glib::ControlFlow::Continue
        }
    });
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed({
        let dialog = dialog.clone();
        move |_, key, _, _| {
            if key == gtk::gdk::Key::Escape {
                dialog.close();
                gtk::glib::Propagation::Stop
            } else {
                gtk::glib::Propagation::Proceed
            }
        }
    });
    dialog.add_controller(keys);
    dialog.set_child(Some(&content));
    let submit = {
        let actions = actions.clone();
        let entry = entry.clone();
        let start = start.clone();
        move || {
            let _ = actions.send(TrayAction::Media(entry.text().to_string()));
            start.set_sensitive(false);
        }
    };
    let submit = std::rc::Rc::new(submit);
    start.connect_clicked({
        let submit = submit.clone();
        move |_| submit()
    });
    entry.connect_activate(move |_| submit());
    cancel.connect_clicked({
        let actions = actions.clone();
        let dialog = dialog.clone();
        move |_| {
            let _ = actions.send(TrayAction::CancelMedia);
            dialog.close();
        }
    });
    dialog.connect_close_request({
        let actions = actions.clone();
        move |_| {
            let _ = actions.send(TrayAction::CancelMedia);
            gtk::glib::Propagation::Proceed
        }
    });
    dialog.present();
    entry.grab_focus();
}
