use gtk4::{self as gtk, prelude::*};
use std::sync::mpsc::Sender;

pub(super) struct ModelsView {
    window: gtk::Window,
    active: gtk::Label,
    status: gtk::Label,
    notice: gtk::Label,
    choices: Vec<gtk::Button>,
    cancel: gtk::Button,
}

impl ModelsView {
    pub fn new(parent: &gtk::ApplicationWindow, actions: Sender<super::TrayAction>) -> Self {
        let window = gtk::Window::builder()
            .title("Local models")
            .transient_for(parent)
            .application(&parent.application().expect("application"))
            .default_width(380)
            .default_height(400)
            .hide_on_close(true)
            .build();
        window.add_css_class("koett-window");
        let header = gtk::HeaderBar::new();
        header.set_title_widget(Some(&gtk::Label::new(Some("Koett · Local models"))));
        window.set_titlebar(Some(&header));
        let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
        {
            let margin = &content;
            margin.set_margin_start(16);
            margin.set_margin_end(16);
            margin.set_margin_top(16);
            margin.set_margin_bottom(16);
        }
        let active = gtk::Label::new(Some("Loading active model…"));
        active.set_wrap(true);
        active.set_xalign(0.0);
        content.append(&active);
        let mut choices = Vec::new();
        for model in [
            crate::model::LocalModel::Compact,
            crate::model::LocalModel::ParakeetV2,
            crate::model::LocalModel::Nemotron,
        ] {
            let installed = model.directory().is_ok_and(|path| path.is_dir());
            let verb = if installed {
                "Use / reload"
            } else {
                "Download & use"
            };
            let button = gtk::Button::with_label(&format!("{verb} {}", model.label()));
            if let Some(label) = button.child().and_downcast::<gtk::Label>() {
                label.set_wrap(true);
            }
            button.set_tooltip_text(Some("Verify or download, load, then save selection. Existing engine stays available if setup fails."));
            let sender = actions.clone();
            button.connect_clicked(move |_| {
                let _ = sender.send(super::TrayAction::SwitchModel(model));
            });
            content.append(&button);
            choices.push(button);
        }
        let provenance = gtk::LinkButton::with_label(
            "https://k2-fsa.github.io/sherpa/onnx/pretrained_models/offline-transducer/nemo-transducer-models.html",
            "NVIDIA · sherpa-onnx · CC BY 4.0",
        );
        if let Some(label) = provenance.child().and_downcast::<gtk::Label>() {
            label.set_wrap(true);
            label.set_max_width_chars(32);
        }
        content.append(&provenance);
        let nemotron_license = gtk::LinkButton::with_label(
            "https://huggingface.co/nvidia/nemotron-speech-streaming-en-0.6b",
            "Nemotron · NVIDIA Open Model License",
        );
        if let Some(label) = nemotron_license.child().and_downcast::<gtk::Label>() {
            label.set_wrap(true);
            label.set_max_width_chars(32);
        }
        content.append(&nemotron_license);
        let sizes = gtk::Label::new(Some(
            "Downloads: 110M 105 MB · v2 483 MB · Nemotron 464 MB + v2 backup",
        ));
        sizes.set_wrap(true);
        sizes.set_xalign(0.0);
        content.append(&sizes);
        let status = gtk::Label::new(None);
        status.set_wrap(true);
        status.set_xalign(0.0);
        content.append(&status);
        let notice = gtk::Label::new(None);
        notice.set_wrap(true);
        notice.set_xalign(0.0);
        notice.add_css_class("error");
        content.append(&notice);
        let cancel = gtk::Button::with_label("Cancel model setup");
        cancel.set_sensitive(false);
        cancel.connect_clicked(|_| {
            crate::model::SWITCH_CANCELLED.store(true, std::sync::atomic::Ordering::Release)
        });

        let scroll = gtk::ScrolledWindow::builder()
            .child(&content)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        scroll.set_vexpand(true);
        let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
        outer.append(&scroll);
        cancel.set_margin_start(16);
        cancel.set_margin_end(16);
        cancel.set_margin_top(8);
        cancel.set_margin_bottom(16);
        outer.append(&cancel);
        window.set_child(Some(&outer));
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed({
            let window = window.clone();
            move |_, key, _, _| {
                if key == gtk::gdk::Key::Escape {
                    window.close();
                    gtk::glib::Propagation::Stop
                } else {
                    gtk::glib::Propagation::Proceed
                }
            }
        });
        window.add_controller(keys);
        Self {
            window,
            active,
            status,
            notice,
            choices,
            cancel,
        }
    }
    pub fn show(&self) {
        self.window.present();
    }
    pub fn active(&self, name: &str) {
        let label = if name.starts_with("sherpa-onnx-nemotron-speech-streaming") {
            "Nemotron 0.6B · 560 ms · Experimental"
        } else if name == "sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8" {
            "Parakeet v2 600M · TDT int8"
        } else if name == crate::paths::DEFAULT_MODEL_ID {
            "Parakeet 110M · CTC int8"
        } else {
            name
        };
        self.active.set_text(&format!("Active: {label}"));
        self.active.set_tooltip_text(Some(name));
        for (button, model) in self.choices.iter().zip([
            crate::model::LocalModel::Compact,
            crate::model::LocalModel::ParakeetV2,
            crate::model::LocalModel::Nemotron,
        ]) {
            let verb = if model.directory().is_ok_and(|path| path.is_dir()) {
                "Use / reload"
            } else {
                "Download & use"
            };
            button.set_label(&format!("{verb} {}", model.label()));
        }
    }
    pub fn update(&self, message: &str) {
        if let Some(notice) = message.strip_prefix("@notice:") {
            self.notice.set_text(notice);
            return;
        }
        if message == "Starting…" {
            self.notice.set_text("");
        }
        self.status.set_text(message);
        for button in &self.choices {
            button.set_sensitive(message == "Ready");
        }
        self.cancel
            .set_sensitive(message.contains("model") || message.starts_with("Starting"));
    }
}
