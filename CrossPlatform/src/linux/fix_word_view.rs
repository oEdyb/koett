//! "Fix a word": correct a history entry and review dictionary suggestions.
//!
//! The correction is stored apart from history. Suggestions learned from all
//! corrections are added to the dictionary only by an explicit Add; forms made
//! of common words say so and need "Add anyway".

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::{self as gtk, prelude::*};

use crate::corrections::{Corrections, Proposal, Suggestion};
use crate::history::HistoryRecord;
use crate::settings::Settings;

pub fn show(parent: &gtk::ApplicationWindow, record: &HistoryRecord) {
    let window = gtk::Window::builder()
        .title("Fix a word")
        .transient_for(parent)
        .modal(true)
        .default_width(460)
        .default_height(520)
        .build();
    window.add_css_class("koett-window");
    let header = gtk::HeaderBar::new();
    let title = gtk::Label::new(Some("Fix a word"));
    title.add_css_class("koett-title");
    header.set_title_widget(Some(&title));
    window.set_titlebar(Some(&header));
    let keys = gtk::ShortcutController::new();
    keys.add_shortcut(gtk::Shortcut::new(
        gtk::ShortcutTrigger::parse_string("Escape"),
        Some(gtk::NamedAction::new("window.close")),
    ));
    window.add_controller(keys);
    let page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    page.add_css_class("koett-settings-page");
    let scroll = gtk::ScrolledWindow::builder()
        .child(&page)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();
    window.set_child(Some(&scroll));

    let notice = label("", "koett-setting-help");
    notice.set_selectable(true);
    let corrections = match Corrections::load() {
        Ok(corrections) => Rc::new(RefCell::new(corrections)),
        Err(error) => {
            // Never overwrite a file Koett cannot read.
            page.append(&label(
                &format!("Corrections unavailable: {error}"),
                "error",
            ));
            window.present();
            return;
        }
    };

    page.append(&label("What Koett wrote", "heading"));
    let original = label(&record.text, "koett-transcript");
    page.append(&original);
    let heading = label("What you said", "heading");
    page.append(&heading);
    let edited = gtk::TextView::builder()
        .wrap_mode(gtk::WrapMode::WordChar)
        .accepts_tab(false)
        .height_request(96)
        .build();
    edited.add_css_class("koett-transcript");
    super::menu::set_accessible_label(edited.upcast_ref(), "What you said");
    let current = corrections
        .borrow()
        .get(&record.id)
        .map(|c| c.corrected.clone());
    edited
        .buffer()
        .set_text(current.as_deref().unwrap_or(&record.text));
    page.append(&edited);
    let save = gtk::Button::with_mnemonic("_Save correction");
    save.set_halign(gtk::Align::Start);
    page.append(&save);
    page.append(&notice);

    page.append(&label("Suggested for your dictionary", "heading"));
    let suggestions = Suggestions {
        list: gtk::Box::new(gtk::Orientation::Vertical, 8),
        corrections,
        learned: Rc::default(),
        notice: notice.clone(),
        fallback: save.clone(),
    };
    page.append(&suggestions.list);
    suggestions.refresh(None);

    save.connect_clicked({
        let edited = edited.clone();
        let suggestions = suggestions.clone();
        let record = record.clone();
        move |_| {
            let buffer = edited.buffer();
            let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
            suggestions.update(
                |corrections| corrections.record(&record.id, &record.text, &text, now_ms()),
                "Correction saved. History keeps what Koett wrote.",
                None,
            );
        }
    });
    super::window::present(&window, None);
    edited.grab_focus();
}

/// The suggestion list; every change saves the store and rebuilds the rows.
/// Learning runs only when the corrections change; Add and Dismiss refilter.
#[derive(Clone)]
struct Suggestions {
    list: gtk::Box,
    corrections: Rc<RefCell<Corrections>>,
    learned: Rc<RefCell<Option<Vec<Suggestion>>>>,
    notice: gtk::Label,
    /// Takes focus when the last suggestion is handled.
    fallback: gtk::Button,
}

impl Suggestions {
    /// Apply a change to a copy, save it, and only then keep it. `focus` is the
    /// row whose action should get focus next (after Dismiss).
    fn update(&self, change: impl FnOnce(&mut Corrections), done: &str, focus: Option<usize>) {
        let mut updated = self.corrections.borrow().clone();
        change(&mut updated);
        match updated.save() {
            Ok(()) => {
                if updated.entries != self.corrections.borrow().entries {
                    self.learned.borrow_mut().take();
                }
                *self.corrections.borrow_mut() = updated;
                if !done.is_empty() {
                    self.notice.set_text(done);
                }
            }
            Err(error) => self.notice.set_text(&error),
        }
        self.refresh(focus);
    }

    /// Rebuild the rows; with `focus`, put keyboard focus on that row's first
    /// action (or the last row's, or Save when none are left).
    fn refresh(&self, focus: Option<usize>) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        let dictionary = match Settings::load() {
            Ok(settings) => settings.dictionary,
            Err(error) => {
                self.list
                    .append(&label(&format!("Settings unavailable: {error}"), "error"));
                return;
            }
        };
        let learned = self
            .learned
            .borrow_mut()
            .get_or_insert_with(|| self.corrections.borrow().learned(&dictionary))
            .clone();
        let proposals = self.corrections.borrow().filter(learned, &dictionary);
        if proposals.is_empty() {
            self.list.append(&label(
                "No suggestions yet. Fix a misheard word above and save.",
                "koett-setting-help",
            ));
        }
        let mut actions = Vec::new();
        for (index, proposal) in proposals.iter().enumerate() {
            let (row, add) = self.row(proposal, index);
            self.list.append(&row);
            actions.push(add);
        }
        if let Some(index) = focus {
            match actions.get(index).or(actions.last()) {
                Some(action) => action.grab_focus(),
                None => self.fallback.grab_focus(),
            };
        }
    }

    fn row(&self, proposal: &Proposal, index: usize) -> (gtk::Box, gtk::Button) {
        let suggestion = &proposal.suggestion;
        let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
        row.add_css_class("koett-setting-row");
        let seen = if suggestion.count > 1 {
            format!(" · in {} entries", suggestion.count)
        } else {
            String::new()
        };
        row.append(&label(
            &format!(
                "Write “{}” when Koett hears “{}”{seen}",
                suggestion.term, suggestion.heard
            ),
            "",
        ));
        if proposal.everyday {
            row.append(&label(
                &format!(
                    "“{0}” is an everyday word. Adding it changes every “{0}” you say.",
                    suggestion.heard
                ),
                "koett-setting-help",
            ));
        }
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let add = gtk::Button::with_label(if proposal.everyday {
            "Add anyway"
        } else {
            "Add to dictionary"
        });
        let dismiss = gtk::Button::with_label("Dismiss");
        actions.append(&add);
        actions.append(&dismiss);
        row.append(&actions);
        add.connect_clicked({
            let this = self.clone();
            let suggestion = suggestion.clone();
            move |_| {
                let result = Settings::edit().and_then(|mut edit| {
                    edit.settings.dictionary = crate::dictionary::add_form(
                        &edit.settings.dictionary,
                        &suggestion.term,
                        &suggestion.heard,
                    )?;
                    edit.commit()
                });
                this.notice.set_text(&match result {
                    Ok(()) => format!(
                        "Added “{}”. It applies to your next dictation.",
                        suggestion.term
                    ),
                    Err(error) => format!("Could not add: {error}"),
                });
                this.refresh(Some(index));
            }
        });
        dismiss.connect_clicked({
            let this = self.clone();
            let suggestion = suggestion.clone();
            move |_| {
                this.update(
                    |corrections| corrections.dismiss(&suggestion),
                    "",
                    Some(index),
                )
            }
        });
        (row, add)
    }
}

fn label(text: &str, class: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_wrap(true);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    label.set_xalign(0.0);
    if !class.is_empty() {
        label.add_css_class(class);
    }
    label
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as i64)
}
