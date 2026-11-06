use super::TrayAction;
use gtk4::{self as gtk, prelude::*};
use std::{cell::RefCell, rc::Rc, sync::mpsc::Sender};

pub(super) fn show(
    window: gtk::ApplicationWindow,
    actions: Sender<TrayAction>,
    latest_completed: Rc<RefCell<Option<String>>>,
    results: Rc<std::cell::Cell<u64>>,
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
    let choose = gtk::Button::with_label("Choose File…");
    let cancel = gtk::Button::with_label("Cancel");
    content.append(&entry);
    content.append(&start);
    content.append(&choose);
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
    // Which result was showing when the user started a new job. It stays hidden, so a
    // failure can't sit next to an older transcript. Results are counted rather than
    // compared, because the same file gives the same text twice.
    let replaced = std::rc::Rc::new(std::cell::Cell::new(None::<u64>));
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
        let choose = choose.clone();
        let replaced = replaced.clone();
        let results = results.clone();
        move || {
            let current = latest
                .borrow()
                .clone()
                .filter(|_| replaced.get() != Some(results.get()));
            copy.set_sensitive(current.is_some());
            if current.is_none() {
                result.buffer().set_text("");
            }
            if dialog.upgrade().is_none() {
                return gtk::glib::ControlFlow::Break;
            }
            if let Some(text) = current.as_ref()
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
            let idle = status.text().starts_with("Ready") || status.text().starts_with("Error");
            start.set_sensitive(idle);
            choose.set_sensitive(idle);
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
        let replaced = replaced.clone();
        let results = results.clone();
        move || {
            replaced.set(Some(results.get()));
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
    choose.connect_clicked({
        let actions = actions.clone();
        let dialog = dialog.clone();
        let phase = phase.clone();
        let replaced = replaced.clone();
        let results = results.clone();
        move |button| {
            let replaced = replaced.clone();
            let results = results.clone();
            let picker = gtk::FileChooserNative::new(
                Some("Transcribe Media File"),
                Some(&dialog),
                gtk::FileChooserAction::Open,
                Some("Transcribe"),
                Some("Cancel"),
            );
            picker.set_modal(true);
            let actions = actions.clone();
            let button = button.clone();
            let phase = phase.clone();
            // Keep the native chooser alive until it answers.
            let held = std::rc::Rc::new(std::cell::RefCell::new(Some(picker.clone())));
            picker.connect_response(move |picker, response| {
                if response == gtk::ResponseType::Accept
                    && let Some(path) = picker.file().and_then(|file| file.path())
                {
                    let path = path.to_string_lossy().into_owned();
                    match crate::media_transcription::validate_file(&path) {
                        Ok(()) => {
                            replaced.set(Some(results.get()));
                            let _ = actions.send(TrayAction::Media(path));
                            button.set_sensitive(false);
                        }
                        Err(error) => phase.set_text(&error),
                    }
                }
                picker.destroy();
                held.borrow_mut().take();
            });
            picker.show();
        }
    });
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
