//! Settings page for Olle's own words: add, edit, remove, import and export.

use crate::dictionary::{self, DictionaryEntry};
use gtk4::{self as gtk, prelude::*};
use std::cell::RefCell;
use std::rc::Rc;

const MAX_IMPORT_BYTES: u64 = 1024 * 1024;

type Rows = Rc<RefCell<Vec<(gtk::Entry, gtk::Entry, gtk::Box)>>>;

/// The dictionary editor inside the Settings window; `entries` reads its current rows.
#[derive(Clone)]
pub struct DictionaryEditor {
    list: gtk::Box,
    rows: Rows,
}

impl DictionaryEditor {
    pub fn build(page: &gtk::Box, window: &gtk::Window, entries: &[DictionaryEntry]) -> Self {
        let title = gtk::Label::new(Some("Dictionary"));
        title.set_xalign(0.0);
        title.add_css_class("heading");
        page.append(&title);
        let help = gtk::Label::new(Some(
            "Names, products and jargon Koett should always spell your way. Under \
             Sounds like, add what Koett writes instead, separated by commas.",
        ));
        help.set_wrap(true);
        help.set_xalign(0.0);
        help.add_css_class("koett-setting-help");
        page.append(&help);
        let editor = Self {
            list: gtk::Box::new(gtk::Orientation::Vertical, 8),
            rows: Rc::default(),
        };
        page.append(&editor.list);
        for entry in entries {
            editor.add_row(entry);
        }
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let add = gtk::Button::with_mnemonic("_Add word");
        let import = gtk::Button::with_mnemonic("_Import…");
        let export = gtk::Button::with_mnemonic("E_xport…");
        for button in [&add, &import, &export] {
            actions.append(button);
        }
        page.append(&actions);
        let notice = gtk::Label::new(None);
        notice.set_wrap(true);
        notice.set_xalign(0.0);
        notice.set_selectable(true);
        notice.add_css_class("koett-setting-help");
        page.append(&notice);
        add.connect_clicked({
            let editor = editor.clone();
            move |_| {
                let term = editor.add_row(&DictionaryEntry::default());
                term.grab_focus();
            }
        });
        import.connect_clicked({
            let editor = editor.clone();
            let window = window.clone();
            let notice = notice.clone();
            move |_| editor.choose_import(&window, &notice)
        });
        export.connect_clicked({
            let editor = editor.clone();
            let window = window.clone();
            let notice = notice.clone();
            move |_| editor.choose_export(&window, &notice)
        });
        editor
    }

    /// Current rows, trimmed and merged. On error, focus moves to the first empty term.
    pub fn entries(&self) -> Result<Vec<DictionaryEntry>, String> {
        let mut entries = Vec::new();
        for (term, forms, row) in self.rows.borrow().iter() {
            if row.parent().is_none() {
                continue;
            }
            if term.text().trim().is_empty() && !forms.text().trim().is_empty() {
                term.grab_focus();
                return Err("Enter a word for each \"sounds like\" list, or remove it.".into());
            }
            entries.push(DictionaryEntry {
                term: term.text().to_string(),
                sounds_like: forms.text().split(',').map(str::to_string).collect(),
            });
        }
        dictionary::normalize(entries)
    }

    fn replace_all(&self, entries: &[DictionaryEntry]) {
        for (_, _, row) in self.rows.borrow_mut().drain(..) {
            self.list.remove(&row);
        }
        for entry in entries {
            self.add_row(entry);
        }
    }

    fn add_row(&self, entry: &DictionaryEntry) -> gtk::Entry {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.add_css_class("koett-setting-row");
        let term = labeled_entry(&row, "Word", &entry.term);
        let forms = labeled_entry(&row, "Sounds like", &entry.sounds_like.join(", "));
        term.set_width_chars(10);
        forms.set_width_chars(14);
        let remove = gtk::Button::with_label("Remove");
        remove.set_valign(gtk::Align::End);
        super::menu::set_accessible_label(remove.upcast_ref(), "Remove word");
        row.append(&remove);
        self.list.append(&row);
        remove.connect_clicked({
            let row = row.clone();
            let list = self.list.clone();
            move |_| list.remove(&row)
        });
        self.rows
            .borrow_mut()
            .push((term.clone(), forms, row.clone()));
        term
    }

    fn choose_import(&self, window: &gtk::Window, notice: &gtk::Label) {
        let dialog = chooser(window, "Import dictionary", gtk::FileChooserAction::Open);
        let editor = self.clone();
        let notice = notice.clone();
        run(dialog, move |path| {
            let result = read_import(&path).and_then(|imported| {
                let count = imported.len();
                let merged = dictionary::merge(&editor.entries()?, imported)?;
                editor.replace_all(&merged);
                Ok(count)
            });
            notice.set_text(&match result {
                Ok(count) => format!("Imported {count} words. Save to keep them."),
                Err(error) => format!("Import failed: {error}"),
            });
        });
    }

    fn choose_export(&self, window: &gtk::Window, notice: &gtk::Label) {
        let entries = match self.entries() {
            Ok(entries) => entries,
            Err(error) => {
                notice.set_text(&error);
                return;
            }
        };
        let dialog = chooser(window, "Export dictionary", gtk::FileChooserAction::Save);
        dialog.set_current_name("Koett dictionary.txt");
        let notice = notice.clone();
        run(dialog, move |path| {
            notice.set_text(
                &match std::fs::write(&path, dictionary::to_text(&entries)) {
                    Ok(()) => format!("Exported {} words.", entries.len()),
                    Err(error) => format!("Export failed: {error}"),
                },
            );
        });
    }
}

fn labeled_entry(row: &gtk::Box, label: &str, text: &str) -> gtk::Entry {
    let entry = gtk::Entry::builder()
        .placeholder_text(label)
        .text(text)
        .hexpand(true)
        .build();
    let title = gtk::Label::new(Some(label));
    title.set_xalign(0.0);
    title.set_mnemonic_widget(Some(&entry));
    let group = gtk::Box::new(gtk::Orientation::Vertical, 4);
    group.append(&title);
    group.append(&entry);
    row.append(&group);
    entry
}

fn chooser(
    window: &gtk::Window,
    title: &str,
    action: gtk::FileChooserAction,
) -> gtk::FileChooserNative {
    let accept = if action == gtk::FileChooserAction::Save {
        "Export"
    } else {
        "Import"
    };
    let dialog = gtk::FileChooserNative::new(
        Some(title),
        Some(window),
        action,
        Some(accept),
        Some("Cancel"),
    );
    let text = gtk::FileFilter::new();
    text.set_name(Some("Plain text (.txt)"));
    text.add_pattern("*.txt");
    text.add_mime_type("text/plain");
    dialog.add_filter(&text);
    if let Some(documents) = directories::UserDirs::new()
        .and_then(|dirs| dirs.document_dir().map(std::path::Path::to_path_buf))
    {
        // A missing folder only leaves the chooser at its own default.
        let _ = dialog.set_current_folder(Some(&gtk::gio::File::for_path(documents)));
    }
    dialog
}

/// Keep the native chooser alive until it answers, then hand back the chosen path.
fn run(dialog: gtk::FileChooserNative, on_path: impl Fn(std::path::PathBuf) + 'static) {
    let hold = Rc::new(RefCell::new(Some(dialog.clone())));
    dialog.connect_response(move |dialog, response| {
        if response == gtk::ResponseType::Accept
            && let Some(path) = dialog.file().and_then(|file| file.path())
        {
            on_path(path);
        }
        dialog.destroy();
        hold.borrow_mut().take();
    });
    dialog.show();
}

fn read_import(path: &std::path::Path) -> Result<Vec<DictionaryEntry>, String> {
    let size = std::fs::metadata(path)
        .map_err(|error| error.to_string())?
        .len();
    if size > MAX_IMPORT_BYTES {
        return Err("the file is larger than 1 MB".into());
    }
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    dictionary::parse_text(&text)
}
