use crate::formatting::{Context, Formatting, Structure, Style};
use gtk4::{self as gtk, prelude::*};
use std::sync::mpsc::Sender;

pub(super) struct FormattingView {
    pub button: gtk::MenuButton,
    controls: gtk::Grid,
    output: gtk::ToggleButton,
    raw: gtk::ToggleButton,
    style: gtk::ComboBoxText,
    structure: gtk::ComboBoxText,
    context: gtk::ComboBoxText,
    status: gtk::Label,
    cancel: gtk::Button,
    refreshing: std::rc::Rc<std::cell::Cell<bool>>,
    setup: std::cell::Cell<bool>,
    idle: std::cell::Cell<bool>,
}
impl FormattingView {
    pub fn new(actions: Sender<super::TrayAction>) -> Self {
        let button = gtk::MenuButton::builder().label("Formatting").build();
        let popover = gtk::Popover::new();
        popover.add_css_class("koett-formatting");
        let body = gtk::Box::new(gtk::Orientation::Vertical, 10);
        for set in [
            gtk::Widget::set_margin_start,
            gtk::Widget::set_margin_end,
            gtk::Widget::set_margin_top,
            gtk::Widget::set_margin_bottom,
        ] {
            set(body.upcast_ref(), 16);
        }
        body.set_size_request(268, 158);
        let title = gtk::Label::new(Some("Formatting"));
        title.set_xalign(0.0);
        title.add_css_class("heading");
        body.append(&title);
        let grid = gtk::Grid::builder()
            .row_spacing(8)
            .column_spacing(12)
            .build();
        let output = gtk::ToggleButton::with_label("S1-mini");
        let raw = gtk::ToggleButton::with_label("Raw");
        output.set_group(Some(&raw));
        let segments = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        segments.add_css_class("linked");
        segments.set_homogeneous(true);
        segments.set_size_request(160, -1);
        segments.append(&raw);
        segments.append(&output);
        let label = gtk::Label::new(Some("Output"));
        label.set_halign(gtk::Align::End);
        grid.attach(&label, 0, 0, 1, 1);
        grid.attach(&segments, 1, 0, 1, 1);
        let mut controls = Vec::new();
        for (row, (name, choices)) in [
            (
                "Styling",
                &["Casual", "Semi-casual", "Semi-formal", "Formal"][..],
            ),
            ("Structure", &["Prose", "Lists"][..]),
            ("Context", &["General", "Email"][..]),
        ]
        .iter()
        .enumerate()
        {
            let label = gtk::Label::new(Some(name));
            label.set_halign(gtk::Align::End);
            let control = gtk::ComboBoxText::new();
            control.set_size_request(160, -1);
            for choice in *choices {
                control.append_text(choice);
            }
            super::menu::set_accessible_label(control.upcast_ref(), name);
            grid.attach(&label, 0, row as i32 + 1, 1, 1);
            grid.attach(&control, 1, row as i32 + 1, 1, 1);
            controls.push(control);
        }
        body.append(&grid);
        let status = gtk::Label::new(None);
        status.set_visible(false);
        status.set_wrap(true);
        status.set_max_width_chars(35);
        status.set_xalign(0.0);
        body.append(&status);
        let cancel = gtk::Button::with_label("Cancel setup");
        cancel.set_visible(false);
        cancel.connect_clicked(|_| {
            crate::formatting::CANCELLED.store(true, std::sync::atomic::Ordering::Release)
        });
        body.append(&cancel);
        popover.set_child(Some(&body));
        button.set_popover(Some(&popover));
        let view = Self {
            button,
            controls: grid,
            output: output.clone(),
            raw,
            style: controls[0].clone(),
            structure: controls[1].clone(),
            context: controls[2].clone(),
            status,
            cancel,
            refreshing: Default::default(),
            setup: Default::default(),
            idle: Default::default(),
        };
        view.refresh();
        let dispatch: std::rc::Rc<dyn Fn()> = {
            let controls = controls.clone();
            let actions = actions.clone();
            let refreshing = view.refreshing.clone();
            let output = output.downgrade();
            std::rc::Rc::new(move || {
                if refreshing.get() {
                    return;
                }
                let value = Formatting {
                    enabled: output.upgrade().is_some_and(|output| output.is_active()),
                    style: match controls[0].active() {
                        Some(0) => Style::Casual,
                        Some(2) => Style::SemiFormal,
                        Some(3) => Style::Formal,
                        _ => Style::SemiCasual,
                    },
                    structure: if controls[1].active() == Some(1) {
                        Structure::Lists
                    } else {
                        Structure::Prose
                    },
                    context: if controls[2].active() == Some(1) {
                        Context::Email
                    } else {
                        Context::General
                    },
                };
                let _ = actions.send(super::TrayAction::Formatting(value));
            })
        };
        for control in &controls {
            let dispatch = dispatch.clone();
            control.connect_changed(move |_| dispatch());
        }
        view.output.connect_toggled(move |_| dispatch());
        view
    }
    fn refresh(&self) {
        self.refreshing.set(true);
        let value = crate::settings::Settings::load()
            .unwrap_or_default()
            .formatting;
        self.raw.set_active(!value.enabled);
        self.output.set_active(value.enabled);
        self.style.set_active(Some(match value.style {
            Style::Casual => 0,
            Style::SemiCasual => 1,
            Style::SemiFormal => 2,
            Style::Formal => 3,
        }));
        self.structure
            .set_active(Some(u32::from(value.structure == Structure::Lists)));
        self.context
            .set_active(Some(u32::from(value.context == Context::Email)));
        self.refreshing.set(false);
    }
    pub fn update(&self, message: &str) {
        if message == "@format-busy" {
            self.setup.set(true);
        } else if message == "@format-idle" {
            self.setup.set(false);
            self.refresh();
        } else if let Some(message) = message.strip_prefix("@notice:")
            && (message.contains("S1-mini") || message.starts_with("Raw active:"))
        {
            self.status.set_text(message);
            self.status.set_visible(true);
        } else if !message.starts_with('@') {
            self.idle.set(message == "Ready");
        }
        let editable = self.idle.get() && !self.setup.get();
        self.controls.set_sensitive(editable);
        self.raw.set_sensitive(editable);
        self.output.set_sensitive(editable);
        for control in [&self.style, &self.structure, &self.context] {
            if !editable {
                control.popdown();
            }
            control.set_sensitive(editable);
        }
        self.cancel.set_visible(self.setup.get());
    }
}
