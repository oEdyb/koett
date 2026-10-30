use gtk4::{self as gtk, prelude::*};
use std::sync::mpsc::Sender;

pub(super) struct ModelsView {
    window: gtk::Window,
    active: gtk::Label,
    status: gtk::Label,
    progress: gtk::ProgressBar,
    notice: gtk::Label,
    choices: Vec<gtk::Button>,
    cancel: gtk::Button,
    selected: std::cell::Cell<Option<crate::model::LocalModel>>,
    ready: std::cell::Cell<bool>,
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
        for model in crate::model::LocalModel::ALL {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            row.add_css_class("koett-setting-row");
            let label = gtk::Label::new(Some(model.label()));
            label.set_xalign(0.0);
            label.set_wrap(true);
            label.set_hexpand(true);
            let button = gtk::Button::with_label("Use");
            button.set_valign(gtk::Align::Center);
            super::menu::set_accessible_label(
                button.upcast_ref(),
                &format!("Use {}", model.label()),
            );
            row.append(&label);
            row.append(&button);
            let sender = actions.clone();
            button.connect_clicked(move |_| {
                let _ = sender.send(super::TrayAction::SwitchModel(model));
            });
            content.append(&row);
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
            "Downloads: 110M 105 MB · v2 483 MB · v3 488 MB · Nemotron 464 MB + v2 backup",
        ));
        sizes.set_wrap(true);
        sizes.set_xalign(0.0);
        content.append(&sizes);
        let status = gtk::Label::new(None);
        status.set_wrap(true);
        status.set_xalign(0.0);
        let progress = gtk::ProgressBar::new();
        progress.set_show_text(true);
        progress.set_visible(false);
        super::menu::set_accessible_label(progress.upcast_ref(), "Model download progress");
        let notice = gtk::Label::new(None);
        notice.set_wrap(true);
        notice.set_xalign(0.0);
        notice.add_css_class("error");
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
        for widget in [
            status.upcast_ref::<gtk::Widget>(),
            progress.upcast_ref(),
            notice.upcast_ref(),
        ] {
            widget.set_margin_start(16);
            widget.set_margin_end(16);
            widget.set_margin_top(8);
            outer.append(widget);
        }
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
            progress,
            notice,
            choices,
            cancel,
            selected: Default::default(),
            ready: Default::default(),
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
        } else if name == "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8" {
            crate::model::LocalModel::ParakeetV3.label()
        } else if name == crate::paths::DEFAULT_MODEL_ID {
            "Parakeet 110M · CTC int8"
        } else {
            name
        };
        self.active.set_text(&format!("Active: {label}"));
        self.active.set_tooltip_text(Some(name));
        self.selected
            .set(crate::model::LocalModel::ALL.into_iter().find(|model| {
                model
                    .directory()
                    .ok()
                    .and_then(|path| {
                        path.file_name()
                            .map(|value| value.to_string_lossy().into_owned())
                    })
                    .as_deref()
                    == Some(name)
            }));
        self.refresh_choices();
    }
    fn refresh_choices(&self) {
        for (button, model) in self.choices.iter().zip(crate::model::LocalModel::ALL) {
            let selected = self.selected.get() == Some(model);
            let label = if selected {
                "Selected"
            } else if model.directory().is_ok_and(|path| path.is_dir()) {
                "Use"
            } else {
                "Download"
            };
            button.set_label(label);
            super::menu::set_accessible_label(
                button.upcast_ref(),
                &format!("{label} {}", model.label()),
            );
            button.set_sensitive(self.ready.get() && !selected);
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
        let percent = message
            .strip_prefix("Downloading model… ")
            .and_then(|value| value.strip_suffix('%'))
            .and_then(|value| value.parse::<u32>().ok());
        self.progress.set_visible(percent.is_some());
        if let Some(percent) = percent {
            self.progress
                .set_fraction(f64::from(percent.min(100)) / 100.0);
            self.progress.set_text(Some(&format!("{percent}%")));
            self.status.set_text("Downloading model…");
        } else {
            self.status.set_text(message);
        }
        self.ready.set(message == "Ready");
        self.refresh_choices();
        self.cancel
            .set_sensitive(message.contains("model") || message.starts_with("Starting"));
    }
}
