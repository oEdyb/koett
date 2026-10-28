use crate::history::{HistoryCursor, HistoryRecord, HistoryStore, HistorySummary};
use gtk4::{self as gtk, prelude::*};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::mpsc;

enum Query {
    Page(u64, String, Option<HistoryCursor>),
    Get(String),
}
enum Reply {
    Page(u64, Result<Vec<HistorySummary>, String>),
    Record(Result<Option<HistoryRecord>, String>),
    ImportError(String),
}

pub struct HistoryView {
    pub root: gtk::Box,
    pub search: gtk::SearchEntry,
    pub record_button: gtk::Button,
    refresh: Rc<dyn Fn()>,
}
impl HistoryView {
    pub fn refresh(&self) {
        (self.refresh)();
    }
    pub fn new(window: &gtk::ApplicationWindow, actions: mpsc::Sender<super::TrayAction>) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("koett-history");
        let search = gtk::SearchEntry::builder()
            .placeholder_text("Search transcripts")
            .hexpand(true)
            .build();
        search.add_css_class("koett-search");
        search.set_width_chars(18);
        search.set_max_width_chars(24);
        search.set_hexpand(false);
        let search_group = gtk::Box::new(gtk::Orientation::Vertical, 4);
        let search_label = gtk::Label::new(Some("_Search transcripts"));
        search_label.set_use_underline(true);
        search_label.set_mnemonic_widget(Some(&search));
        search_label.set_xalign(0.0);
        search_label.add_css_class("koett-metadata");
        search.set_placeholder_text(None);
        search_group.append(&search_label);
        search_group.append(&search);
        let search_row = gtk::Box::new(gtk::Orientation::Horizontal, 24);
        search_row.add_css_class("koett-history-heading");
        let heading = gtk::Label::new(Some("History"));
        heading.add_css_class("koett-title");
        heading.set_hexpand(true);
        heading.set_xalign(0.0);
        search_row.append(&heading);
        search_row.append(&search_group);
        root.append(&search_row);
        let stack = gtk::Stack::new();
        stack.set_vexpand(true);
        root.append(&stack);
        let empty = gtk::Box::new(gtk::Orientation::Vertical, 16);
        empty.add_css_class("koett-empty");
        empty.set_halign(gtk::Align::Center);
        let brand = gtk::Label::new(Some("Koett"));
        brand.add_css_class("koett-empty-brand");
        empty.append(&brand);
        empty.set_valign(gtk::Align::Center);
        let empty_title = gtk::Label::new(Some("Loading history…"));
        empty_title.add_css_class("koett-empty-title");
        empty_title.set_max_width_chars(42);
        empty_title.set_wrap(true);
        empty.append(&empty_title);
        let empty_record = gtk::Button::with_label("Record");
        empty_record.add_css_class("koett-record");
        empty_record.set_halign(gtk::Align::Center);
        empty_record.connect_clicked(move |_| {
            let _ = actions.send(super::TrayAction::Toggle);
        });
        empty.append(&empty_record);
        let clear = gtk::Button::with_label("Clear search");
        clear.set_halign(gtk::Align::Center);
        empty.append(&clear);
        clear.connect_clicked({
            let search = search.clone();
            move |_| search.set_text("")
        });
        let retry = gtk::Button::with_label("Retry");
        retry.set_halign(gtk::Align::Center);
        empty.append(&retry);
        stack.add_named(&empty, Some("empty"));
        let workspace = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        workspace.set_vexpand(true);
        stack.add_named(&workspace, Some("history"));
        stack.set_visible_child_name("empty");
        empty_record.set_visible(false);
        clear.set_visible(false);
        retry.set_visible(false);
        let navigation = gtk::Box::new(gtk::Orientation::Vertical, 0);
        navigation.set_size_request(300, -1);
        navigation.add_css_class("koett-navigation");
        let list = gtk::ListBox::new();
        list.add_css_class("koett-history-list");
        let scroll = gtk::ScrolledWindow::builder()
            .child(&list)
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        navigation.append(&scroll);
        let newest = gtk::Button::with_label("Newest transcripts");
        let paging = gtk::Box::new(gtk::Orientation::Vertical, 4);
        paging.add_css_class("koett-paging");
        paging.append(&newest);
        let older = gtk::Button::with_label("Older transcripts");
        older.set_sensitive(false);
        paging.append(&older);
        navigation.append(&paging);
        workspace.append(&navigation);
        let separator = gtk::Separator::new(gtk::Orientation::Vertical);
        workspace.append(&separator);
        let detail = gtk::Box::new(gtk::Orientation::Vertical, 12);
        detail.set_hexpand(true);
        detail.add_css_class("koett-reading");
        let back = gtk::Button::with_label("Back to history");
        back.set_halign(gtk::Align::Start);
        back.set_visible(false);
        detail.append(&back);
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let transcript_heading = gtk::Label::new(Some("Select a transcript"));
        transcript_heading.add_css_class("koett-reading-heading");
        transcript_heading.set_xalign(0.0);
        transcript_heading.set_hexpand(true);
        transcript_heading.set_ellipsize(gtk::pango::EllipsizeMode::End);
        toolbar.append(&transcript_heading);
        detail.append(&toolbar);
        let meta = gtk::Label::new(None);
        meta.set_xalign(0.0);
        meta.add_css_class("koett-metadata");
        meta.set_wrap(true);
        meta.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        meta.set_width_chars(1);
        meta.set_max_width_chars(48);
        detail.append(&meta);
        let text = gtk::TextView::builder()
            .editable(false)
            .cursor_visible(true)
            .wrap_mode(gtk::WrapMode::WordChar)
            .vexpand(true)
            .build();
        text.add_css_class("koett-transcript");
        text.set_pixels_above_lines(4);
        text.set_pixels_below_lines(8);
        text.set_pixels_inside_wrap(3);
        let text_scroll = gtk::ScrolledWindow::builder()
            .child(&text)
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        detail.append(&text_scroll);
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let copy = gtk::Button::with_label("Copy");
        let export = gtk::Button::with_label("Export…");
        copy.set_sensitive(false);
        export.set_sensitive(false);
        buttons.append(&copy);
        buttons.append(&export);
        toolbar.append(&buttons);
        workspace.append(&detail);
        let notice = gtk::Label::new(None);
        notice.add_css_class("koett-notice");
        notice.set_wrap(true);
        notice.set_visible(false);
        notice.connect_label_notify(|notice| notice.set_visible(!notice.text().is_empty()));
        notice.set_margin_top(8);
        notice.set_margin_bottom(8);
        root.append(&notice);
        let (sender, requests) = mpsc::channel();
        let (responses, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let mut store = HistoryStore::open_default();
            if let Ok(store) = &store
                && let Err(error) = store.import_default_legacy()
            {
                let _ = responses.send(Reply::ImportError(format!("Legacy import: {error}")));
            }
            while let Ok(request) = requests.recv() {
                if store.is_err() {
                    store = HistoryStore::open_default();
                }
                let result = store.as_ref().map_err(Clone::clone);
                let reply = match request {
                    Query::Page(generation, query, cursor) => Reply::Page(
                        generation,
                        result.and_then(|store| store.page(&query, cursor.as_ref(), 50)),
                    ),
                    Query::Get(id) => Reply::Record(result.and_then(|store| store.get(&id))),
                };
                if responses.send(reply).is_err() {
                    break;
                }
            }
        });
        let generation = Rc::new(Cell::new(0_u64));
        let cursor = Rc::new(RefCell::new(None::<HistoryCursor>));
        let selected = Rc::new(RefCell::new(None::<HistoryRecord>));
        let selected_id = Rc::new(RefCell::new(None::<String>));
        let rows = Rc::new(RefCell::new(Vec::<HistorySummary>::new()));
        let refresh: Rc<dyn Fn()> = Rc::new({
            let sender = sender.clone();
            let search = search.clone();
            let generation = generation.clone();
            move || {
                generation.set(generation.get() + 1);
                let _ = sender.send(Query::Page(
                    generation.get(),
                    search.text().to_string(),
                    None,
                ));
            }
        });
        retry.connect_clicked({
            let refresh = refresh.clone();
            move |_| refresh()
        });
        newest.connect_clicked({
            let refresh = refresh.clone();
            move |_| refresh()
        });
        search.connect_search_changed({
            let refresh = refresh.clone();
            let previous = RefCell::new(String::new());
            move |search| {
                let query = search.text().to_string();
                if *previous.borrow() != query {
                    *previous.borrow_mut() = query;
                    refresh();
                }
            }
        });
        older.connect_clicked({
            let sender = sender.clone();
            let cursor = cursor.clone();
            let search = search.clone();
            let generation = generation.clone();
            move |_| {
                generation.set(generation.get() + 1);
                let _ = sender.send(Query::Page(
                    generation.get(),
                    search.text().to_string(),
                    cursor.borrow().clone(),
                ));
            }
        });
        list.connect_row_selected({
            let sender = sender.clone();
            let rows = rows.clone();
            let selected_id = selected_id.clone();
            let navigation = navigation.clone();
            let detail = detail.clone();
            let back = back.clone();
            let workspace = workspace.clone();
            move |_, row| {
                if let Some(row) = row
                    && let Some(summary) = rows.borrow().get(row.index() as usize)
                {
                    *selected_id.borrow_mut() = Some(summary.id.clone());
                    let _ = sender.send(Query::Get(summary.id.clone()));
                    if workspace.width() < 650 {
                        navigation.set_visible(false);
                        detail.set_visible(true);
                        back.set_visible(true);
                    }
                }
            }
        });
        back.connect_clicked({
            let navigation = navigation.clone();
            let detail = detail.clone();
            move |_| {
                navigation.set_visible(true);
                detail.set_visible(false);
            }
        });
        // Adapt when tiled or resized. Preserve the user's pane on narrow windows.
        gtk::glib::timeout_add_local(std::time::Duration::from_millis(150), {
            let root = root.downgrade();
            let navigation = navigation.clone();
            let detail = detail.clone();
            let back = back.clone();
            let separator = separator.clone();
            move || {
                let Some(root) = root.upgrade() else {
                    return gtk::glib::ControlFlow::Break;
                };
                navigation.set_hexpand(root.width() < 650);
                if root.width() >= 650 {
                    navigation.set_visible(true);
                    detail.set_visible(true);
                    back.set_visible(false);
                    separator.set_visible(true);
                } else {
                    separator.set_visible(false);
                    if navigation.is_visible() {
                        detail.set_visible(false);
                    } else {
                        back.set_visible(true);
                    }
                }
                gtk::glib::ControlFlow::Continue
            }
        });
        copy.connect_clicked({
            let selected = selected.clone();
            move |button| {
                if let Some(record) = selected.borrow().as_ref() {
                    button.clipboard().set_text(&record.text);
                    button.set_label("Copied");
                    let button = button.clone();
                    gtk::glib::timeout_add_local_once(
                        std::time::Duration::from_secs(2),
                        move || button.set_label("Copy"),
                    );
                }
            }
        });
        export.connect_clicked({
            let selected = selected.clone();
            let window = window.clone();
            let notice = notice.clone();
            move |_| {
                let Some(record) = selected.borrow().clone() else {
                    return;
                };
                let dialog = gtk::FileChooserNative::new(
                    Some("Export transcript"),
                    Some(&window),
                    gtk::FileChooserAction::Save,
                    Some("Export"),
                    Some("Cancel"),
                );
                let plain = gtk::FileFilter::new();
                plain.set_name(Some("Plain text (.txt)"));
                plain.add_pattern("*.txt");
                let markdown = gtk::FileFilter::new();
                markdown.set_name(Some("Markdown (.md)"));
                markdown.add_pattern("*.md");
                dialog.add_filter(&plain);
                dialog.add_filter(&markdown);
                dialog.set_filter(&plain);
                dialog.set_current_name("Transcript.txt");
                dialog.connect_filter_notify(|dialog| {
                    if let Some(name) = dialog.current_name() {
                        let extension =
                            if dialog.filter().and_then(|filter| filter.name()).as_deref()
                                == Some("Markdown (.md)")
                            {
                                "md"
                            } else {
                                "txt"
                            };
                        let name = std::path::Path::new(name.as_str()).with_extension(extension);
                        if let Some(name) = name.file_name().and_then(|name| name.to_str()) {
                            dialog.set_current_name(name);
                        }
                    }
                });
                if let Some(directory) = directories::UserDirs::new()
                    .and_then(|dirs| dirs.document_dir().map(std::path::Path::to_path_buf))
                    && let Err(error) =
                        dialog.set_current_folder(Some(&gtk::gio::File::for_path(directory)))
                {
                    notice.set_text(&format!("Could not open Documents: {error}"));
                }
                let chooser_hold = Rc::new(RefCell::new(Some(dialog.clone())));
                dialog.connect_response({
                    let notice = notice.clone();
                    let chooser_hold = chooser_hold.clone();
                    move |dialog, response| {
                        if response == gtk::ResponseType::Accept
                            && let Some(path) = dialog.file().and_then(|f| f.path())
                        {
                            let content = if path.extension().is_some_and(|e| e == "md") {
                                format!(
                                    "# Transcript\n\n{} · {}\n\n{}\n",
                                    timestamp(record.created_at_ms),
                                    record.model,
                                    record.text
                                )
                            } else {
                                record.text.clone()
                            };
                            let (sender, receiver) = mpsc::channel();
                            std::thread::spawn(move || {
                                let _ = sender
                                    .send(std::fs::write(path, content).map_err(|e| e.to_string()));
                            });
                            let notice = notice.clone();
                            gtk::glib::timeout_add_local(
                                std::time::Duration::from_millis(50),
                                move || {
                                    match receiver.try_recv() {
                                        Ok(Ok(())) => notice.set_text("Exported"),
                                        Ok(Err(e)) => {
                                            notice.set_text(&format!("Export failed: {e}"))
                                        }
                                        Err(mpsc::TryRecvError::Empty) => {
                                            return gtk::glib::ControlFlow::Continue;
                                        }
                                        Err(_) => notice.set_text("Export interrupted"),
                                    }
                                    gtk::glib::ControlFlow::Break
                                },
                            );
                        }
                        dialog.destroy();
                        chooser_hold.borrow_mut().take();
                    }
                });
                dialog.show();
            }
        });
        let record_button = empty_record.clone();
        let mut import_error = None::<String>;
        gtk::glib::timeout_add_local(std::time::Duration::from_millis(40), {
            let root = root.downgrade();
            let search = search.clone();
            move || {
                if root.upgrade().is_none() {
                    return gtk::glib::ControlFlow::Break;
                }
                for reply in receiver.try_iter() {
                    match reply {
                        Reply::ImportError(error) => {
                            notice.set_text(&error);
                            import_error = Some(error);
                        }
                        Reply::Page(g, result) if g == generation.get() => match result {
                            Ok(page) => {
                                while let Some(child) = list.first_child() {
                                    list.remove(&child);
                                }
                                *cursor.borrow_mut() = page.last().map(|row| HistoryCursor {
                                    id: row.id.clone(),
                                    created_at_ms: row.created_at_ms,
                                });
                                older.set_sensitive(page.len() == 50);
                                notice.set_text(import_error.as_deref().unwrap_or(""));
                                if page.is_empty() {
                                    stack.set_visible_child_name("empty");
                                    empty_title.set_text(if search.text().is_empty() {
                                        "No transcripts yet"
                                    } else {
                                        "No matches"
                                    });
                                    empty_record.set_visible(search.text().is_empty());
                                    clear.set_visible(!search.text().is_empty());
                                    retry.set_visible(false);
                                } else {
                                    stack.set_visible_child_name("history");
                                }
                                for row in &page {
                                    let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
                                    content.set_margin_top(14);
                                    content.set_margin_bottom(14);
                                    content.set_margin_start(16);
                                    content.set_margin_end(16);
                                    let date = gtk::Label::new(Some(&timestamp(row.created_at_ms)));
                                    date.set_xalign(0.0);
                                    date.add_css_class("koett-metadata");
                                    let preview =
                                        gtk::Label::new(Some(&row.preview.replace('\n', " ")));
                                    preview.add_css_class("koett-excerpt");
                                    preview.set_wrap(true);
                                    preview.set_lines(2);
                                    preview.set_ellipsize(gtk::pango::EllipsizeMode::End);
                                    preview.set_xalign(0.0);
                                    preview.set_max_width_chars(32);
                                    content.append(&date);
                                    content.append(&preview);
                                    list.append(&content);
                                }
                                let restore = selected_id
                                    .borrow()
                                    .as_ref()
                                    .and_then(|id| page.iter().position(|row| &row.id == id));
                                let first_load = selected_id.borrow().is_none();
                                *rows.borrow_mut() = page;
                                if let Some(index) = restore.or_else(|| {
                                    (first_load && !rows.borrow().is_empty()).then_some(0)
                                }) && let Some(row) = list.row_at_index(index as i32)
                                {
                                    list.select_row(Some(&row));
                                }
                            }
                            Err(error) => {
                                stack.set_visible_child_name("empty");
                                empty_title.set_text(&format!("History unavailable: {error}"));
                                empty_record.set_visible(false);
                                clear.set_visible(false);
                                retry.set_visible(true);
                            }
                        },
                        Reply::Record(Ok(Some(record)))
                            if selected_id.borrow().as_ref() == Some(&record.id) =>
                        {
                            transcript_heading.set_text(&timestamp(record.created_at_ms));
                            meta.set_text(&record.model);
                            text.buffer().set_text(&record.text);
                            copy.set_sensitive(true);
                            export.set_sensitive(true);
                            *selected.borrow_mut() = Some(record);
                        }
                        Reply::Record(Err(error)) => {
                            notice.set_text(&format!("Could not load transcript: {error}"))
                        }
                        _ => {}
                    }
                }
                gtk::glib::ControlFlow::Continue
            }
        });
        refresh();
        Self {
            root,
            search,
            record_button,
            refresh,
        }
    }
}
fn timestamp(ms: i64) -> String {
    if ms == 0 {
        return "Unknown date".into();
    }
    time::OffsetDateTime::from_unix_timestamp(ms / 1000)
        .ok()
        .map(|date| {
            let date = date
                .to_offset(time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC));
            format!(
                "{:04}-{:02}-{:02}  {:02}:{:02}",
                date.year(),
                u8::from(date.month()),
                date.day(),
                date.hour(),
                date.minute()
            )
        })
        .unwrap_or_else(|| "Unknown date".into())
}
