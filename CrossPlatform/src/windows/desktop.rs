mod correction;
mod microphone;
mod paste_shortcuts;
pub(super) mod processing;
#[path = "words.rs"]
mod words;
use crate::{
    history::{HistoryCursor, HistoryStore, HistorySummary},
    settings::Settings,
};
use std::cell::RefCell;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc,
};
use windows::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{Controls::Dialogs::*, WindowsAndMessaging::*},
};
use windows::core::{PCWSTR, w};

static CORRECTION_OPEN: AtomicBool = AtomicBool::new(false);
static CORRECTION_HWND: AtomicUsize = AtomicUsize::new(0);
static PROCESSING_OPEN: AtomicBool = AtomicBool::new(false);
static PROCESSING_HWND: AtomicUsize = AtomicUsize::new(0);
static WORDS_OPEN: AtomicBool = AtomicBool::new(false);
static WORDS_HWND: AtomicUsize = AtomicUsize::new(0);
static HISTORY_OPEN: AtomicBool = AtomicBool::new(false);
static SETTINGS_OPEN: AtomicBool = AtomicBool::new(false);
static HISTORY_HWND: AtomicUsize = AtomicUsize::new(0);
static SETTINGS_HWND: AtomicUsize = AtomicUsize::new(0);
thread_local! { static WINDOW: RefCell<Option<Window>> = const { RefCell::new(None) }; }
#[derive(Clone, Copy)]
struct PaintTheme {
    background: COLORREF,
    foreground: COLORREF,
    brush: HBRUSH,
}
thread_local! { static PAINT: std::cell::Cell<Option<PaintTheme>> = const { std::cell::Cell::new(None) }; }
impl From<&super::native_theme::Theme> for PaintTheme {
    fn from(theme: &super::native_theme::Theme) -> Self {
        Self {
            background: theme.background,
            foreground: theme.foreground,
            brush: theme.brush,
        }
    }
}

const SEARCH: usize = 10;
const LIST: usize = 11;
const COPY: usize = 12;
const EXPORT: usize = 13;
const NEXT_PAGE: usize = 14;
const EXPORT_RESULTS: usize = 15;
const EXPORT_STATUS: usize = 16;
const SHOW_RAW: usize = 17;
const FIX_WORD: usize = 18;
const SHORTCUT: usize = 20;
const STARTUP: usize = 21;
const SAVE: usize = 22;
const CHANGE_SHORTCUT: usize = 23;
enum Query {
    Search(u64, String, Option<HistoryCursor>),
    Get(u64, String, bool),
    Correct(u64, String),
    Export(String, std::path::PathBuf, bool),
}
enum Reply {
    Rows(u64, Result<Page, String>),
    Text(u64, Result<String, String>),
    Correction(u64, Result<Option<crate::history::HistoryRecord>, String>),
    Exported(Result<usize, String>),
}
struct Page {
    rows: Vec<HistorySummary>,
    next: Option<HistoryCursor>,
}
fn history_page(
    store: &HistoryStore,
    text: &str,
    before: Option<&HistoryCursor>,
) -> Result<Page, String> {
    let rows = store.page(text, before, 50)?;
    let last = rows.last().map(|row| HistoryCursor {
        id: row.id.clone(),
        created_at_ms: row.created_at_ms,
    });
    let next = match last {
        Some(cursor) if !store.page(text, Some(&cursor), 1)?.is_empty() => Some(cursor),
        _ => None,
    };
    Ok(Page { rows, next })
}
fn record_text(record: crate::history::HistoryRecord, raw: bool) -> String {
    if raw {
        record.raw_text.unwrap_or(record.text)
    } else {
        record.text
    }
}
// Read every matching page on the history worker, independent of the visible page.
// Bound the in-memory export explicitly; exceeding it never creates a partial file.
fn export_results(store: &HistoryStore, query: &str, raw: bool) -> Result<(String, usize), String> {
    use std::fmt::Write as _;
    let mut text = String::new();
    let mut count = 0;
    let mut cursor = None;
    loop {
        let page = history_page(store, query, cursor.as_ref())?;
        for row in page.rows {
            let record = store
                .get(&row.id)?
                .ok_or("Transcript changed during export; retry")?;
            let date = timestamp(record.created_at_ms);
            let content = record_text(record, raw);
            if text.len().saturating_add(content.len()).saturating_add(256) > 8 * 1024 * 1024 {
                return Err("Export exceeds 8 MiB; narrow the search and export again".into());
            }
            writeln!(text, "{date}\n{content}\n").map_err(|e| e.to_string())?;
            count += 1;
        }
        cursor = page.next;
        if cursor.is_none() {
            return Ok((text, count));
        }
    }
}
struct Window {
    theme: super::native_theme::Theme,
    font: HFONT,
    controls: Vec<(HWND, [i32; 4])>,
    hwnd: HWND,
    owner: usize,
    settings: bool,
    words: bool,
    processing_view: bool,
    processing: Option<processing::Controls>,
    correction: Option<correction::Editor>,
    word_entries: Vec<(String, String)>,
    words_selected: Option<usize>,
    words_category: bool,
    search: HWND,
    list: HWND,
    detail: HWND,
    startup: HWND,
    mode: HWND,
    recording_limit: HWND,
    microphone: Option<microphone::Controls>,
    rows: Vec<HistorySummary>,
    selected_key: Option<String>,
    selected_text: Option<String>,
    show_raw: bool,
    query: mpsc::Sender<Query>,
    replies: mpsc::Receiver<Reply>,
    generation: u64,
    page_generation: u64,
    next_cursor: Option<HistoryCursor>,
    page_pending: bool,
}

pub(super) fn open_words(owner: HWND) {
    open_kind(false, true, false, owner, None);
}
pub(super) fn open(settings: bool, owner: HWND) {
    open_kind(settings, false, false, owner, None);
}
fn open_kind(
    settings: bool,
    words: bool,
    processing_view: bool,
    owner: HWND,
    record: Option<crate::history::HistoryRecord>,
) {
    let flag = if record.is_some() {
        &CORRECTION_OPEN
    } else if processing_view {
        &PROCESSING_OPEN
    } else if words {
        &WORDS_OPEN
    } else if settings {
        &SETTINGS_OPEN
    } else {
        &HISTORY_OPEN
    };
    let slot = if record.is_some() {
        &CORRECTION_HWND
    } else if processing_view {
        &PROCESSING_HWND
    } else if words {
        &WORDS_HWND
    } else if settings {
        &SETTINGS_HWND
    } else {
        &HISTORY_HWND
    };
    if flag.swap(true, Ordering::AcqRel) {
        let existing = HWND(slot.load(Ordering::Acquire) as *mut _);
        if !existing.is_invalid() {
            unsafe {
                let _ = ShowWindow(existing, SW_SHOWNORMAL);
                let _ = SetForegroundWindow(existing);
            }
        }
        return;
    }
    let owner = owner.0 as usize;
    std::thread::spawn(move || {
        struct Reset(&'static AtomicBool, &'static AtomicUsize);
        impl Drop for Reset {
            fn drop(&mut self) {
                self.1.store(0, Ordering::Release);
                self.0.store(false, Ordering::Release);
            }
        }
        let _reset = Reset(flag, slot);
        if let Err(error) = run(settings, words, processing_view, owner, record) {
            super::show_fatal_error(&error);
        }
    });
}

fn run(
    settings: bool,
    words: bool,
    processing_view: bool,
    owner: usize,
    record: Option<crate::history::HistoryRecord>,
) -> Result<(), String> {
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() }.map_err(|e| e.to_string())?;
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    let _apartment = Apartment;
    let correction = record.map(correction::Editor::new).transpose()?;
    let correction_view = correction.is_some();
    struct RestoreOwner(HWND);
    impl Drop for RestoreOwner {
        fn drop(&mut self) {
            unsafe {
                let _ = windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow(self.0, true);
            }
        }
    }
    let _restore_owner = if correction_view && owner != 0 {
        let parent = HWND(owner as *mut _);
        unsafe {
            let _ = windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow(parent, false);
        }
        Some(RestoreOwner(parent))
    } else {
        None
    };
    let (send, receive) = mpsc::channel();
    let (reply, replies) = mpsc::channel();
    std::thread::spawn(move || {
        let store = HistoryStore::open_default();
        for query in receive {
            let result = match query {
                Query::Correct(id, key) => Reply::Correction(
                    id,
                    store
                        .as_ref()
                        .map_err(Clone::clone)
                        .and_then(|s| s.get(&key)),
                ),
                Query::Export(text, path, raw) => {
                    Reply::Exported(store.as_ref().map_err(Clone::clone).and_then(|store| {
                        let (text, count) = export_results(store, &text, raw)?;
                        write_export(&path, &text)?;
                        Ok(count)
                    }))
                }
                Query::Search(id, text, before) => Reply::Rows(
                    id,
                    store
                        .as_ref()
                        .map_err(Clone::clone)
                        .and_then(|s| history_page(s, &text, before.as_ref())),
                ),
                Query::Get(id, key, raw) => Reply::Text(
                    id,
                    store
                        .as_ref()
                        .map_err(Clone::clone)
                        .and_then(|s| s.get(&key))
                        .map(|r| r.map(|r| record_text(r, raw)).unwrap_or_default()),
                ),
            };
            if reply.send(result).is_err() {
                break;
            }
        }
    });
    unsafe {
        let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
        let class = w!("KoettNativeDesktop");
        let registration = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(proc),
            hInstance: instance.into(),
            lpszClassName: class,
            hCursor: LoadCursorW(None, IDC_ARROW).map_err(|e| e.to_string())?,
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as usize as *mut _),
            ..Default::default()
        };
        RegisterClassExW(&registration);
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            class,
            if correction_view {
                w!("Koett Fix a word")
            } else if processing_view {
                w!("Koett Model and Formatting")
            } else if words {
                w!("Koett Words")
            } else if settings {
                w!("Koett Settings")
            } else {
                w!("Koett History")
            },
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            if settings { 560 } else { 960 },
            if settings { 600 } else { 680 },
            if correction_view && owner != 0 {
                Some(HWND(owner as *mut _))
            } else {
                None
            },
            None,
            Some(instance.into()),
            None,
        )
        .map_err(|e| e.to_string())?;
        fit_window(hwnd, None, settings, processing_view);
        let mut window = Window {
            theme: super::native_theme::Theme::new()?,
            font: HFONT::default(),
            controls: vec![],
            hwnd,
            owner,
            settings,
            words,
            processing_view,
            processing: None,
            correction,
            word_entries: vec![],
            words_selected: None,
            words_category: false,
            search: HWND::default(),
            list: HWND::default(),
            detail: HWND::default(),
            startup: HWND::default(),
            mode: HWND::default(),
            recording_limit: HWND::default(),
            microphone: None,
            rows: vec![],
            selected_key: None,
            selected_text: None,
            show_raw: false,
            query: send,
            replies,
            generation: 0,
            page_generation: 0,
            next_cursor: None,
            page_pending: false,
        };
        PAINT.with(|paint| paint.set(Some(PaintTheme::from(&window.theme))));
        (if correction_view {
            &CORRECTION_HWND
        } else if processing_view {
            &PROCESSING_HWND
        } else if words {
            &WORDS_HWND
        } else if settings {
            &SETTINGS_HWND
        } else {
            &HISTORY_HWND
        })
        .store(hwnd.0 as usize, Ordering::Release);
        window.theme.apply(hwnd);
        window.refresh_font();
        window.create()?;
        window.actions_enabled(false);
        window.layout();
        WINDOW.with(|w| *w.borrow_mut() = Some(window));
        SetTimer(Some(hwnd), 1, 50, None);
        let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
        let mut message = MSG::default();
        loop {
            let result = GetMessageW(&mut message, None, 0, 0).0;
            if result == 0 {
                break;
            }
            if result == -1 {
                return Err("Windows message loop failed".into());
            }
            if tab_from_multiline(hwnd, &message) {
                continue;
            }
            if !IsDialogMessageW(hwnd, &message).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        WINDOW.with(|w| *w.borrow_mut() = None);
    }
    Ok(())
}

impl Drop for Window {
    fn drop(&mut self) {
        PAINT.with(|paint| paint.set(None));
        if !self.font.is_invalid() {
            unsafe {
                let _ = DeleteObject(HGDIOBJ(self.font.0));
            }
        }
    }
}
impl Window {
    fn refresh_font(&mut self) {
        unsafe {
            let font = CreateFontW(
                -self.scale(14),
                0,
                0,
                0,
                400,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                DEFAULT_PITCH.0 as u32,
                w!("Segoe UI"),
            );
            if !font.is_invalid() {
                let old = self.font;
                self.font = font;
                for (child, _) in &self.controls {
                    SendMessageW(
                        *child,
                        WM_SETFONT,
                        Some(WPARAM(font.0 as usize)),
                        Some(LPARAM(1)),
                    );
                }
                if !old.is_invalid() {
                    let _ = DeleteObject(HGDIOBJ(old.0));
                }
            }
        }
    }

    fn scale(&self, value: i32) -> i32 {
        value * unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(self.hwnd) }.max(96) as i32 / 96
    }

    fn control(
        &mut self,
        class: PCWSTR,
        label: &str,
        style: WINDOW_STYLE,
        id: usize,
        bounds: [i32; 4],
    ) -> Result<HWND, String> {
        let [x, y, width, height] = bounds;
        let text = super::wide(label);
        unsafe {
            let child = CreateWindowExW(
                Default::default(),
                class,
                PCWSTR(text.as_ptr()),
                WS_CHILD | WS_VISIBLE | style,
                self.scale(x),
                self.scale(y),
                self.scale(width),
                self.scale(height),
                Some(self.hwnd),
                Some(HMENU(id as *mut _)),
                None,
                None,
            )
            .map_err(|e| e.to_string())?;
            let _ = SendMessageW(
                child,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                Some(LPARAM(1)),
            );
            self.controls.push((child, bounds));
            Ok(child)
        }
    }
    fn create(&mut self) -> Result<(), String> {
        if let Some(mut editor) = self.correction.take() {
            let result = editor.create(self);
            self.correction = Some(editor);
            return result;
        }
        if self.words {
            return self.create_words();
        }
        if self.processing_view {
            self.processing = Some(processing::Controls::new(self, &Settings::load()?)?);
            return Ok(());
        }
        if self.settings {
            let settings = Settings::load()?;
            self.control(
                w!("STATIC"),
                "Shortcut",
                Default::default(),
                0,
                [24, 24, 200, 24],
            )?;
            self.control(
                w!("STATIC"),
                &settings.shortcut,
                Default::default(),
                SHORTCUT,
                [24, 54, 260, 24],
            )?;
            self.control(
                w!("BUTTON"),
                "Change shortcut…",
                WS_TABSTOP,
                CHANGE_SHORTCUT,
                [300, 48, 180, 32],
            )?;
            self.microphone = Some(microphone::Controls::new(self, &settings)?);
            self.control(
                w!("STATIC"),
                "Recording mode",
                Default::default(),
                0,
                [300, 224, 200, 24],
            )?;
            self.mode = self.control(
                w!("COMBOBOX"),
                "",
                WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                33,
                [300, 252, 212, 140],
            )?;
            for label in ["Toggle", "Hold"] {
                let label = super::wide(label);
                unsafe {
                    SendMessageW(
                        self.mode,
                        CB_ADDSTRING,
                        None,
                        Some(LPARAM(label.as_ptr() as isize)),
                    );
                }
            }
            unsafe {
                SendMessageW(
                    self.mode,
                    CB_SETCURSEL,
                    Some(WPARAM(usize::from(
                        settings.recording_mode == crate::settings::RecordingMode::Hold,
                    ))),
                    None,
                );
            }
            self.control(
                w!("STATIC"),
                "Recording limit",
                Default::default(),
                0,
                [528, 224, 200, 24],
            )?;
            self.recording_limit = self.control(
                w!("COMBOBOX"),
                "",
                WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                35,
                [528, 252, 212, 140],
            )?;
            for minutes in crate::settings::RECORDING_LIMIT_CHOICES {
                let label = super::wide(&format!("{minutes} minutes"));
                unsafe {
                    SendMessageW(
                        self.recording_limit,
                        CB_ADDSTRING,
                        None,
                        Some(LPARAM(label.as_ptr() as isize)),
                    );
                }
            }
            let selected = crate::settings::RECORDING_LIMIT_CHOICES
                .iter()
                .position(|minutes| *minutes == settings.recording_limit_minutes)
                .unwrap_or(2);
            unsafe {
                SendMessageW(
                    self.recording_limit,
                    CB_SETCURSEL,
                    Some(WPARAM(selected)),
                    None,
                );
            }
            self.startup = self.control(
                w!("BUTTON"),
                "Start at login",
                WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                STARTUP,
                [24, 252, 240, 32],
            )?;
            self.theme.apply_checkbox(self.startup);
            let draw = self.control(
                w!("BUTTON"),
                "Draw while dictating",
                WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                40,
                [24, 300, 488, 32],
            )?;
            self.theme.apply_checkbox(draw);
            unsafe {
                SendMessageW(
                    draw,
                    BM_SETCHECK,
                    Some(WPARAM(usize::from(settings.draw_while_dictating))),
                    None,
                );
            }
            unsafe {
                SendMessageW(
                    self.startup,
                    BM_SETCHECK,
                    Some(WPARAM(usize::from(super::start_at_login_registered()?))),
                    None,
                );
            }
            self.control(
                w!("STATIC"),
                "Draw key",
                Default::default(),
                0,
                [24, 344, 148, 20],
            )?;
            let key = self.control(
                w!("COMBOBOX"),
                "Draw key",
                WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32 | WS_VSCROLL.0),
                41,
                [24, 368, 148, 140],
            )?;
            for label in ["Shift", "Control", "Alt"] {
                let label = label.encode_utf16().chain([0]).collect::<Vec<_>>();
                unsafe {
                    SendMessageW(
                        key,
                        CB_ADDSTRING,
                        None,
                        Some(LPARAM(label.as_ptr() as isize)),
                    );
                }
            }
            let index = match settings.draw_key {
                crate::settings::DrawKey::Shift => 0,
                crate::settings::DrawKey::Control => 1,
                crate::settings::DrawKey::Alt => 2,
            };
            unsafe {
                SendMessageW(key, CB_SETCURSEL, Some(WPARAM(index)), None);
            }
            self.control(
                w!("STATIC"),
                "Draw lock shortcut",
                Default::default(),
                0,
                [192, 344, 320, 20],
            )?;
            self.control(
                w!("EDIT"),
                &settings.draw_lock_shortcut,
                WS_TABSTOP | WS_BORDER | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                42,
                [192, 368, 320, 30],
            )?;
            let selected = self.control(
                w!("BUTTON"),
                "Read selected text at pins",
                WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                43,
                [24, 412, 264, 32],
            )?;
            self.theme.apply_checkbox(selected);
            self.control(
                w!("BUTTON"),
                "Paste shortcuts…",
                WS_TABSTOP,
                44,
                [300, 412, 212, 32],
            )?;
            unsafe {
                SendMessageW(
                    selected,
                    BM_SETCHECK,
                    Some(WPARAM(usize::from(settings.pin_selected_text))),
                    None,
                );
            }
            self.control(
                w!("BUTTON"),
                "Model and formatting…",
                WS_TABSTOP,
                34,
                [300, 460, 212, 32],
            )?;
            self.control(
                w!("BUTTON"),
                "Save",
                WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                SAVE,
                [24, 460, 100, 32],
            )?;
            self.detail =
                self.control(w!("STATIC"), "", Default::default(), 0, [24, 504, 488, 48])?;
        } else {
            self.control(
                w!("STATIC"),
                "Search transcripts",
                Default::default(),
                0,
                [24, 16, 280, 24],
            )?;
            self.search = self.control(
                w!("EDIT"),
                "",
                WS_TABSTOP | WS_BORDER | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                SEARCH,
                [24, 44, 280, 30],
            )?;
            self.control(
                w!("STATIC"),
                "Transcripts",
                Default::default(),
                25,
                [24, 88, 280, 20],
            )?;
            self.list = self.control(
                w!("LISTBOX"),
                "Transcripts",
                WS_TABSTOP
                    | WS_BORDER
                    | WS_VSCROLL
                    | WINDOW_STYLE(LBS_NOTIFY as u32 | LBS_NOINTEGRALHEIGHT as u32),
                LIST,
                [24, 116, 280, 452],
            )?;
            self.control(
                w!("STATIC"),
                "Transcript",
                Default::default(),
                24,
                [328, 66, 200, 20],
            )?;
            self.detail = self.control(
                w!("EDIT"),
                "No transcripts yet",
                WS_TABSTOP
                    | WS_BORDER
                    | WS_VSCROLL
                    | WINDOW_STYLE(
                        ES_MULTILINE as u32 | ES_READONLY as u32 | ES_AUTOVSCROLL as u32,
                    ),
                0,
                [328, 88, 580, 480],
            )?;
            self.control(w!("BUTTON"), "Copy", WS_TABSTOP, COPY, [328, 32, 100, 32])?;
            self.control(
                w!("BUTTON"),
                "Export…",
                WS_TABSTOP,
                EXPORT,
                [440, 32, 100, 32],
            )?;
            self.control(
                w!("BUTTON"),
                "Load more",
                WS_TABSTOP,
                NEXT_PAGE,
                [24, 580, 120, 32],
            )?;
            self.control(
                w!("BUTTON"),
                "Export results…",
                WS_TABSTOP,
                EXPORT_RESULTS,
                [160, 580, 156, 32],
            )?;
            self.control(
                w!("STATIC"),
                "",
                Default::default(),
                EXPORT_STATUS,
                [328, 580, 580, 40],
            )?;
            let raw = self.control(
                w!("BUTTON"),
                "Raw transcript",
                WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                SHOW_RAW,
                [552, 32, 180, 32],
            )?;
            self.theme.apply_checkbox(raw);
            self.control(
                w!("BUTTON"),
                "Fix a word…",
                WS_TABSTOP,
                FIX_WORD,
                [744, 32, 160, 32],
            )?;
            self.search();
        }
        Ok(())
    }
    fn search(&mut self) {
        self.rows.clear();
        self.selected_key = None;
        self.selected_text = None;
        unsafe {
            let _ = SendMessageW(self.list, LB_RESETCONTENT, None, None);
        }
        self.actions_enabled(false);
        self.generation += 1;
        self.next_cursor = None;
        set(self.detail, "Searching…");
        self.load_page(None);
    }
    fn load_page(&mut self, before: Option<HistoryCursor>) {
        self.page_generation += 1;
        self.page_pending = true;
        self.page_enabled(false);
        let _ = self.query.send(Query::Search(
            self.page_generation,
            read(self.search),
            before,
        ));
    }
    fn page_enabled(&self, enabled: bool) {
        if let Ok(button) = unsafe { GetDlgItem(Some(self.hwnd), NEXT_PAGE as i32) } {
            let _ = unsafe {
                windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow(button, enabled)
            };
        }
    }
    fn actions_enabled(&self, enabled: bool) {
        if self.settings {
            return;
        }
        for id in [COPY, EXPORT, FIX_WORD] {
            if let Ok(child) = unsafe { GetDlgItem(Some(self.hwnd), id as i32) } {
                let _ = unsafe {
                    windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow(child, enabled)
                };
            }
        }
    }
    fn selection_changed(&mut self) {
        if self.settings {
            return;
        }
        let index = unsafe { SendMessageW(self.list, LB_GETCURSEL, None, None) }.0;
        let key = usize::try_from(index)
            .ok()
            .and_then(|i| self.rows.get(i))
            .map(|r| r.id.clone());
        if self.selected_key != key {
            self.selected_key = key.clone();
            self.selected_text = None;
            self.actions_enabled(false);
            if let Some(key) = key {
                self.generation += 1;
                let _ = self
                    .query
                    .send(Query::Get(self.generation, key, self.show_raw));
            }
        }
    }
    fn poll(&mut self) {
        if self.words {
            self.words_poll();
        }
        if let Some(mut editor) = self.correction.take() {
            editor.poll(self);
            self.correction = Some(editor);
        }
        if let Some(microphone) = &mut self.microphone {
            microphone.poll();
        }
        if let Some(processing) = &mut self.processing {
            processing.poll();
        }
        self.selection_changed();
        while let Ok(reply) = self.replies.try_recv() {
            match reply {
                Reply::Rows(id, result) if id == self.page_generation => {
                    self.page_pending = false;
                    match result {
                        Ok(page) => {
                            self.next_cursor = page.next;
                            self.page_enabled(self.next_cursor.is_some());
                            for row in &page.rows {
                                let label = super::wide(&format!(
                                    "{}  {}",
                                    timestamp(row.created_at_ms),
                                    row.preview.replace(['\n', '\r'], " ")
                                ));
                                unsafe {
                                    SendMessageW(
                                        self.list,
                                        LB_ADDSTRING,
                                        None,
                                        Some(LPARAM(label.as_ptr() as isize)),
                                    );
                                }
                            }
                            self.rows.extend(page.rows);
                            if self.selected_key.is_none() {
                                set(
                                    self.detail,
                                    if self.rows.is_empty() {
                                        if read(self.search).is_empty() {
                                            "No transcripts yet"
                                        } else {
                                            "No matches"
                                        }
                                    } else {
                                        "Select a transcript"
                                    },
                                );
                            }
                        }
                        Err(e) => {
                            self.page_enabled(self.next_cursor.is_some());
                            set(self.detail, &e);
                        }
                    }
                }
                Reply::Exported(result) => unsafe {
                    if let Ok(button) = GetDlgItem(Some(self.hwnd), EXPORT_RESULTS as i32) {
                        let _ =
                            windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow(button, true);
                    }
                    if let Ok(status) = GetDlgItem(Some(self.hwnd), EXPORT_STATUS as i32) {
                        set(
                            status,
                            &match result {
                                Ok(count) => format!("Exported {count} transcripts"),
                                Err(error) => error,
                            },
                        );
                    }
                },
                Reply::Correction(id, result) if id == self.generation => match result {
                    Ok(Some(record)) => open_kind(false, false, false, self.hwnd, Some(record)),
                    Ok(None) => set(self.detail, "Transcript no longer exists"),
                    Err(error) => set(self.detail, &error),
                },
                Reply::Text(id, result) if id == self.generation => match result {
                    Ok(text) => {
                        set(
                            self.detail,
                            &text.replace("\r\n", "\n").replace('\n', "\r\n"),
                        );
                        self.selected_text = Some(text);
                        self.actions_enabled(true);
                    }
                    Err(error) => {
                        set(self.detail, &error);
                        self.selected_text = None;
                        self.actions_enabled(false);
                    }
                },
                _ => {}
            }
        }
    }
    fn command(&mut self, id: usize, notification: u16) {
        if let Some(mut editor) = self.correction.take() {
            editor.command(self, id, notification);
            self.correction = Some(editor);
            return;
        }
        if self.words {
            self.words_command(id, notification);
            return;
        }
        if self.processing.as_mut().is_some_and(|p| p.command(id)) {
            return;
        }
        if self
            .microphone
            .as_mut()
            .is_some_and(|m| m.command(id, notification))
        {
            return;
        }
        match id {
            SEARCH if notification == EN_CHANGE as u16 => self.search(),
            LIST if notification == LBN_SELCHANGE as u16 => self.selection_changed(),
            NEXT_PAGE if !self.page_pending => {
                if let Some(cursor) = self.next_cursor.clone() {
                    self.load_page(Some(cursor));
                }
            }
            FIX_WORD => {
                if let Some(key) = self.selected_key.clone() {
                    let _ = self.query.send(Query::Correct(self.generation, key));
                }
            }
            SHOW_RAW => {
                if let Ok(button) = unsafe { GetDlgItem(Some(self.hwnd), SHOW_RAW as i32) } {
                    self.show_raw = unsafe { SendMessageW(button, BM_GETCHECK, None, None) }.0 != 0;
                    self.selected_key = None;
                    self.selection_changed();
                }
            }
            COPY => {
                if let Some(text) = &self.selected_text
                    && let Err(e) = super::copy_text(self.hwnd, text)
                {
                    super::show_fatal_error(&e);
                }
            }
            EXPORT => {
                if let Some(text) = &self.selected_text
                    && let Err(e) = export(self.hwnd, text)
                {
                    super::show_fatal_error(&e);
                }
            }
            EXPORT_RESULTS => match export_path(self.hwnd) {
                Ok(Some(path)) => {
                    if self
                        .query
                        .send(Query::Export(read(self.search), path, self.show_raw))
                        .is_ok()
                    {
                        unsafe {
                            if let Ok(button) = GetDlgItem(Some(self.hwnd), EXPORT_RESULTS as i32) {
                                let _ = windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow(
                                    button, false,
                                );
                            }
                            if let Ok(status) = GetDlgItem(Some(self.hwnd), EXPORT_STATUS as i32) {
                                set(status, "Exporting…");
                            }
                        }
                    }
                }
                Ok(None) => {}
                Err(error) => super::show_fatal_error(&error),
            },
            34 => open_kind(true, false, true, HWND(self.owner as *mut _), None),
            44 => {
                let result = unsafe { GetDlgItem(Some(self.hwnd), 44) }
                    .map_err(|e| e.to_string())
                    .and_then(|anchor| paste_shortcuts::show(self.hwnd, anchor));
                set(
                    self.detail,
                    match &result {
                        Ok(()) => "",
                        Err(error) => error,
                    },
                );
            }
            CHANGE_SHORTCUT => unsafe {
                let _ = PostMessageW(
                    Some(HWND(self.owner as *mut _)),
                    WM_COMMAND,
                    WPARAM(super::MENU_CHANGE_SHORTCUT),
                    LPARAM(0),
                );
            },
            SAVE => {
                let enabled = unsafe { SendMessageW(self.startup, BM_GETCHECK, None, None) }.0 != 0;
                let result = (|| {
                    let microphone = self.microphone.as_ref().map(|m| m.selected()).transpose()?;
                    let mut edit = Settings::edit()?;
                    let draw =
                        unsafe { GetDlgItem(Some(self.hwnd), 40) }.map_err(|e| e.to_string())?;
                    edit.settings.draw_while_dictating =
                        unsafe { SendMessageW(draw, BM_GETCHECK, None, None) }.0 != 0;
                    let key =
                        unsafe { GetDlgItem(Some(self.hwnd), 41) }.map_err(|e| e.to_string())?;
                    edit.settings.draw_key =
                        match unsafe { SendMessageW(key, CB_GETCURSEL, None, None) }.0 {
                            0 => crate::settings::DrawKey::Shift,
                            1 => crate::settings::DrawKey::Control,
                            2 => crate::settings::DrawKey::Alt,
                            _ => return Err("Choose a draw key".into()),
                        };
                    let shortcut = read(
                        unsafe { GetDlgItem(Some(self.hwnd), 42) }.map_err(|e| e.to_string())?,
                    );
                    let shortcut = shortcut.trim();
                    if !shortcut.is_empty() {
                        let draw = shortcut.parse::<crate::shortcut::Shortcut>()?;
                        if edit
                            .settings
                            .shortcut
                            .parse::<crate::shortcut::Shortcut>()
                            .ok()
                            == Some(draw)
                        {
                            return Err("Draw lock and dictation need different shortcuts".into());
                        }
                    }
                    edit.settings.draw_lock_shortcut = shortcut.to_owned();
                    let selected =
                        unsafe { GetDlgItem(Some(self.hwnd), 43) }.map_err(|e| e.to_string())?;
                    edit.settings.pin_selected_text =
                        unsafe { SendMessageW(selected, BM_GETCHECK, None, None) }.0 != 0;
                    if let Some(microphone) = microphone {
                        edit.settings.microphone = microphone;
                    }
                    let old = super::start_at_login_registered()?;
                    edit.settings.start_at_login = enabled;
                    let mode = unsafe { SendMessageW(self.mode, CB_GETCURSEL, None, None) }.0;
                    edit.settings.recording_mode = match mode {
                        0 => crate::settings::RecordingMode::Toggle,
                        1 => crate::settings::RecordingMode::Hold,
                        _ => return Err("Choose a recording mode".into()),
                    };
                    let limit =
                        unsafe { SendMessageW(self.recording_limit, CB_GETCURSEL, None, None) }.0;
                    edit.settings.recording_limit_minutes = usize::try_from(limit)
                        .ok()
                        .and_then(|limit| {
                            crate::settings::RECORDING_LIMIT_CHOICES.get(limit).copied()
                        })
                        .ok_or_else(|| "Choose a recording limit".to_string())?;
                    super::set_start_at_login(enabled)?;
                    if let Err(e) = edit.commit() {
                        let _ = super::set_start_at_login(old);
                        return Err(e);
                    }
                    unsafe {
                        let _ = PostMessageW(
                            Some(HWND(self.owner as *mut _)),
                            0x8053,
                            WPARAM(0),
                            LPARAM(0),
                        );
                    }
                    Ok(())
                })();
                set(
                    self.detail,
                    match &result {
                        Ok(()) => "Saved",
                        Err(e) => e,
                    },
                );
            }
            _ => {}
        }
    }
    fn layout(&self) {
        if self.correction.is_some() {
            self.words_layout();
            return;
        }
        if self.words {
            self.words_layout();
            return;
        }
        for (child, [x, y, w, h]) in &self.controls {
            unsafe {
                let _ = MoveWindow(
                    *child,
                    self.scale(*x),
                    self.scale(*y),
                    self.scale(*w),
                    self.scale(*h),
                    true,
                );
            }
        }
        if self.settings {
            return;
        }
        unsafe {
            let mut r = RECT::default();
            if GetClientRect(self.hwnd, &mut r).is_ok() {
                let dpi = windows::Win32::UI::HiDpi::GetDpiForWindow(self.hwnd).max(96) as i32;
                r.right = r.right * 96 / dpi;
                r.bottom = r.bottom * 96 / dpi;
                let left = (r.right / 3).clamp(180, 320);
                let height = (r.bottom - 160).max(80);
                for (id, x, y, w, h) in [
                    (NEXT_PAGE, 24, r.bottom - 56, 120, 32),
                    (EXPORT_RESULTS, 160, r.bottom - 56, 156, 32),
                    (
                        EXPORT_STATUS,
                        left + 24,
                        r.bottom - 56,
                        (r.right - left - 48).max(80),
                        40,
                    ),
                    (COPY, left + 24, 32, 100, 32),
                    (EXPORT, left + 136, 32, 100, 32),
                    (SHOW_RAW, left + 248, 32, 180, 32),
                    (FIX_WORD, left + 432, 32, 160, 32),
                    (24, left + 24, 66, (r.right - left - 48).max(80), 20),
                    (25, 24, 88, left - 24, 20),
                ] {
                    if let Ok(child) = GetDlgItem(Some(self.hwnd), id as i32) {
                        let _ = MoveWindow(
                            child,
                            self.scale(x),
                            self.scale(y),
                            self.scale(w),
                            self.scale(h),
                            true,
                        );
                    }
                }
                let _ = MoveWindow(
                    self.search,
                    self.scale(24),
                    self.scale(44),
                    self.scale(left - 24),
                    self.scale(30),
                    true,
                );
                let _ = MoveWindow(
                    self.list,
                    self.scale(24),
                    self.scale(116),
                    self.scale(left - 24),
                    self.scale((height - 28).max(60)),
                    true,
                );
                let _ = MoveWindow(
                    self.detail,
                    self.scale(left + 24),
                    self.scale(88),
                    self.scale((r.right - left - 48).max(80)),
                    self.scale(height),
                    true,
                );
            }
        }
    }
}
fn fit_window(hwnd: HWND, suggested: Option<&RECT>, settings: bool, processing: bool) {
    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return;
        }
        let dpi = windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd).max(96) as i32;
        let width = suggested
            .map(|r| r.right - r.left)
            .unwrap_or(if settings { 560 } else { 960 } * dpi / 96)
            .min(info.rcWork.right - info.rcWork.left);
        let height = suggested
            .map(|r| r.bottom - r.top)
            .unwrap_or(
                if settings {
                    if processing { 480 } else { 600 }
                } else {
                    680
                } * dpi
                    / 96,
            )
            .min(info.rcWork.bottom - info.rcWork.top);
        let x = suggested
            .map(|r| r.left)
            .unwrap_or(info.rcWork.left + (info.rcWork.right - info.rcWork.left - width) / 2)
            .clamp(info.rcWork.left, info.rcWork.right - width);
        let y = suggested
            .map(|r| r.top)
            .unwrap_or(info.rcWork.top + (info.rcWork.bottom - info.rcWork.top - height) / 2)
            .clamp(info.rcWork.top, info.rcWork.bottom - height);
        let _ = SetWindowPos(
            hwnd,
            None,
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE | SWP_NOZORDER,
        );
    }
}

unsafe extern "system" fn proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_CLOSE {
        WINDOW.with(|cell| {
            if let Ok(window) = cell.try_borrow()
                && let Some(window) = window.as_ref()
                && window.correction.is_some()
                && window.owner != 0
            {
                unsafe {
                    let _ = windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow(
                        HWND(window.owner as *mut _),
                        true,
                    );
                }
            }
        });
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
        return LRESULT(0);
    }
    if message == WM_DESTROY {
        unsafe {
            PostQuitMessage(0);
        }
        return LRESULT(0);
    }
    if matches!(
        message,
        WM_ERASEBKGND | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX | WM_CTLCOLORSTATIC | WM_CTLCOLORBTN
    ) && let Some(result) = PAINT.with(|paint| {
        paint.get().map(|theme| unsafe {
            let dc = HDC(wparam.0 as *mut _);
            if message == WM_ERASEBKGND {
                let mut bounds = RECT::default();
                let _ = GetClientRect(hwnd, &mut bounds);
                FillRect(dc, &bounds, theme.brush);
                LRESULT(1)
            } else {
                SetTextColor(dc, theme.foreground);
                SetBkColor(dc, theme.background);
                LRESULT(theme.brush.0 as isize)
            }
        })
    }) {
        return result;
    }
    WINDOW.with(|cell| {
        if let Ok(mut window) = cell.try_borrow_mut()
            && let Some(window) = window.as_mut()
        {
            match message {
                WM_COMMAND => window.command(wparam.0 & 0xffff, ((wparam.0 >> 16) & 0xffff) as u16),
                WM_TIMER => window.poll(),
                WM_THEMECHANGED | WM_SETTINGCHANGE | WM_SYSCOLORCHANGE => {
                    if let Ok(theme) = super::native_theme::Theme::new() {
                        PAINT.with(|paint| paint.set(Some(PaintTheme::from(&theme))));
                        window.theme = theme;
                        window.theme.apply(hwnd);
                        if let Some(processing) = &window.processing {
                            processing.theme(&window.theme);
                        }
                        if window.settings && !window.processing_view {
                            for id in [STARTUP, 40, 43] {
                                if let Ok(checkbox) = unsafe { GetDlgItem(Some(hwnd), id as i32) } {
                                    window.theme.apply_checkbox(checkbox);
                                }
                            }
                        } else if let Ok(raw) = unsafe { GetDlgItem(Some(hwnd), SHOW_RAW as i32) } {
                            window.theme.apply_checkbox(raw);
                        }
                    }
                    unsafe {
                        let _ = InvalidateRect(Some(hwnd), None, true);
                        for (child, _) in &window.controls {
                            let _ = InvalidateRect(Some(*child), None, true);
                        }
                    }
                }
                WM_SIZE => window.layout(),
                WM_DPICHANGED => {
                    let suggested = unsafe { &*(lparam.0 as *const RECT) };
                    fit_window(
                        hwnd,
                        Some(suggested),
                        window.settings,
                        window.processing_view,
                    );
                    window.refresh_font();
                    window.layout();
                }
                _ => {}
            }
        }
    });
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}
fn read(hwnd: HWND) -> String {
    unsafe {
        let mut buffer = vec![0u16; GetWindowTextLengthW(hwnd).max(0) as usize + 1];
        let count = GetWindowTextW(hwnd, &mut buffer);
        String::from_utf16_lossy(&buffer[..count.max(0) as usize])
    }
}
fn set(hwnd: HWND, text: &str) {
    let text = super::wide(text);
    unsafe {
        let _ = SetWindowTextW(hwnd, PCWSTR(text.as_ptr()));
    }
}
fn write_export(path: &std::path::Path, text: &str) -> Result<(), String> {
    use std::io::Write as _;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let temporary =
        path.with_file_name(format!(".koett-export-{}-{nonce}.part", std::process::id()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        std::fs::rename(&temporary, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}
fn export(hwnd: HWND, text: &str) -> Result<(), String> {
    if let Some(path) = export_path(hwnd)? {
        write_export(&path, text)?;
    }
    Ok(())
}
fn export_path(hwnd: HWND) -> Result<Option<std::path::PathBuf>, String> {
    export_path_named(hwnd, "Transcript.txt")
}
fn export_path_named(hwnd: HWND, filename: &str) -> Result<Option<std::path::PathBuf>, String> {
    let mut file = [0u16; 32768];
    for (dest, src) in file.iter_mut().zip(filename.encode_utf16()) {
        *dest = src;
    }
    let initial = directories::UserDirs::new()
        .and_then(|d| d.document_dir().map(|p| p.to_path_buf()))
        .map(|p| super::wide(&p.to_string_lossy()));
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: hwnd,
        lpstrFile: windows::core::PWSTR(file.as_mut_ptr()),
        lpstrInitialDir: initial
            .as_ref()
            .map(|p| PCWSTR(p.as_ptr()))
            .unwrap_or_default(),
        nMaxFile: file.len() as u32,
        lpstrFilter: w!("Text files\0*.txt\0All files\0*.*\0\0"),
        lpstrDefExt: w!("txt"),
        Flags: OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
        ..Default::default()
    };
    if unsafe { GetSaveFileNameW(&mut dialog) }.as_bool() {
        let end = file.iter().position(|x| *x == 0).unwrap_or(file.len());
        return Ok(Some(std::path::PathBuf::from(String::from_utf16_lossy(
            &file[..end],
        ))));
    }
    let error = unsafe { CommDlgExtendedError() }.0;
    if error != 0 {
        return Err(format!("Could not choose export file: {error}"));
    }
    Ok(None)
}

fn timestamp(milliseconds: i64) -> String {
    time::OffsetDateTime::from_unix_timestamp_nanos(i128::from(milliseconds) * 1_000_000)
        .ok()
        .and_then(|d| {
            d.format(time::macros::format_description!(
                "[year]-[month]-[day] [hour]:[minute]"
            ))
            .ok()
        })
        .unwrap_or_default()
}

// Multiline EDIT requests Tab as text even inside IsDialogMessage. These are
// form fields: Tab/Shift+Tab must reach the next action, including in History.
fn tab_from_multiline(window: HWND, message: &MSG) -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetKeyState, SetFocus, VK_CONTROL, VK_MENU, VK_SHIFT, VK_TAB,
    };
    if message.message != WM_KEYDOWN || message.wParam.0 != VK_TAB.0 as usize {
        return false;
    }
    unsafe {
        if GetKeyState(VK_CONTROL.0 as i32) < 0 || GetKeyState(VK_MENU.0 as i32) < 0 {
            return false;
        }
        let mut class = [0u16; 16];
        let length = GetClassNameW(message.hwnd, &mut class);
        if length != 4
            || !String::from_utf16_lossy(&class[..4]).eq_ignore_ascii_case("Edit")
            || GetWindowLongPtrW(message.hwnd, GWL_STYLE) & ES_MULTILINE as isize == 0
        {
            return false;
        }
        if let Ok(next) = GetNextDlgTabItem(
            window,
            Some(message.hwnd),
            GetKeyState(VK_SHIFT.0 as i32) < 0,
        ) {
            let _ = SetFocus(Some(next));
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    #[test]
    fn multiline_tab_reaches_the_next_enabled_form_action() {
        use super::*;
        unsafe {
            let parent = CreateWindowExW(
                Default::default(),
                w!("STATIC"),
                w!("Form"),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                0,
                0,
                320,
                200,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            let edit = CreateWindowExW(
                Default::default(),
                w!("EDIT"),
                w!("Text"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_MULTILINE as u32),
                0,
                0,
                100,
                40,
                Some(parent),
                None,
                None,
                None,
            )
            .unwrap();
            let _disabled = CreateWindowExW(
                Default::default(),
                w!("BUTTON"),
                w!("Disabled"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_DISABLED,
                0,
                50,
                100,
                30,
                Some(parent),
                None,
                None,
                None,
            )
            .unwrap();
            let button = CreateWindowExW(
                Default::default(),
                w!("BUTTON"),
                w!("Save"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                0,
                90,
                100,
                30,
                Some(parent),
                None,
                None,
                None,
            )
            .unwrap();
            let message = MSG {
                hwnd: edit,
                message: WM_KEYDOWN,
                wParam: WPARAM(9),
                ..MSG::default()
            };
            assert!(tab_from_multiline(parent, &message));
            assert_eq!(
                windows::Win32::UI::Input::KeyboardAndMouse::GetFocus(),
                button
            );
            let letter = MSG {
                wParam: WPARAM(65),
                ..message
            };
            assert!(!tab_from_multiline(parent, &letter));
            DestroyWindow(parent).unwrap();
        }
    }
    use super::*;
    use crate::history::HistoryRecord;

    #[test]
    fn raw_history_is_explicit_and_legacy_rows_remain_readable() {
        let mut record = HistoryRecord {
            id: "raw".into(),
            created_at_ms: 0,
            model: "fixture".into(),
            text: "Formatted".into(),
            raw_text: Some("raw words".into()),
        };
        assert_eq!(record_text(record.clone(), false), "Formatted");
        assert_eq!(record_text(record.clone(), true), "raw words");
        record.raw_text = None;
        assert_eq!(record_text(record, true), "Formatted");
    }
    #[test]
    fn history_pages_reach_all_rows_and_restart_for_new_search() {
        let path = std::env::temp_dir().join(format!(
            "koett-windows-pages-{}.sqlite3",
            std::process::id()
        ));
        let store = HistoryStore::open(&path).unwrap();
        for i in 0..110 {
            store
                .save(&HistoryRecord {
                    id: format!("page-{i:03}"),
                    created_at_ms: 100,
                    model: "fixture".into(),
                    text: format!("entry {i:03}"),
                    raw_text: None,
                })
                .unwrap();
        }
        let mut cursor = None;
        let mut ids = vec![];
        let mut sizes = vec![];
        loop {
            let page = history_page(&store, "entry", cursor.as_ref()).unwrap();
            sizes.push(page.rows.len());
            ids.extend(page.rows.into_iter().map(|r| r.id));
            cursor = page.next;
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(sizes, [50, 50, 10]);
        assert_eq!(ids.len(), 110);
        assert_eq!(
            ids.iter().collect::<std::collections::HashSet<_>>().len(),
            110
        );
        let (exported, count) = export_results(&store, "", false).unwrap();
        assert_eq!(count, 110);
        assert_eq!(exported.matches("entry ").count(), 110);
        let output = path.with_extension("txt");
        std::fs::write(&output, "previous export").unwrap();
        write_export(&output, &exported).unwrap();
        assert_eq!(std::fs::read_to_string(&output).unwrap(), exported);
        std::fs::remove_file(output).unwrap();
        let (filtered_export, count) = export_results(&store, "entry 003", false).unwrap();
        assert_eq!(count, 1);
        assert!(filtered_export.contains("entry 003"));
        assert_eq!(
            export_results(&store, "no-such-entry", false).unwrap(),
            (String::new(), 0)
        );
        let filtered = history_page(&store, "entry 003", None).unwrap();
        assert_eq!(filtered.rows.len(), 1);
        assert_eq!(filtered.rows[0].id, "page-003");
        assert!(filtered.next.is_none());
        drop(store);
        std::fs::remove_file(path).unwrap();
    }
}
