use crate::{
    history::{HistoryStore, HistorySummary},
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

static HISTORY_OPEN: AtomicBool = AtomicBool::new(false);
static SETTINGS_OPEN: AtomicBool = AtomicBool::new(false);
static HISTORY_HWND: AtomicUsize = AtomicUsize::new(0);
static SETTINGS_HWND: AtomicUsize = AtomicUsize::new(0);
thread_local! { static WINDOW: RefCell<Option<Window>> = const { RefCell::new(None) }; }
const SEARCH: usize = 10;
const LIST: usize = 11;
const COPY: usize = 12;
const EXPORT: usize = 13;
const SHORTCUT: usize = 20;
const STARTUP: usize = 21;
const SAVE: usize = 22;
const CHANGE_SHORTCUT: usize = 23;
enum Query {
    Search(u64, String),
    Get(u64, String),
}
enum Reply {
    Rows(u64, Result<Vec<HistorySummary>, String>),
    Text(u64, Result<String, String>),
}
struct Window {
    theme: super::native_theme::Theme,
    font: HFONT,
    controls: Vec<(HWND, [i32; 4])>,
    hwnd: HWND,
    owner: usize,
    settings: bool,
    search: HWND,
    list: HWND,
    detail: HWND,
    startup: HWND,
    rows: Vec<HistorySummary>,
    selected_key: Option<String>,
    selected_text: Option<String>,
    query: mpsc::Sender<Query>,
    replies: mpsc::Receiver<Reply>,
    generation: u64,
}

pub(super) fn open(settings: bool, owner: HWND) {
    let flag = if settings {
        &SETTINGS_OPEN
    } else {
        &HISTORY_OPEN
    };
    let slot = if settings {
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
        if let Err(error) = run(settings, owner) {
            super::show_fatal_error(&error);
        }
    });
}

fn run(settings: bool, owner: usize) -> Result<(), String> {
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
    let (send, receive) = mpsc::channel();
    let (reply, replies) = mpsc::channel();
    std::thread::spawn(move || {
        let store = HistoryStore::open_default();
        for query in receive {
            let result = match query {
                Query::Search(id, text) => Reply::Rows(
                    id,
                    store
                        .as_ref()
                        .map_err(Clone::clone)
                        .and_then(|s| s.page(&text, None, 50)),
                ),
                Query::Get(id, key) => Reply::Text(
                    id,
                    store
                        .as_ref()
                        .map_err(Clone::clone)
                        .and_then(|s| s.get(&key))
                        .map(|r| r.map(|r| r.text).unwrap_or_default()),
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
            if settings {
                w!("Koett Settings")
            } else {
                w!("Koett History")
            },
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            if settings { 560 } else { 960 },
            if settings { 300 } else { 680 },
            None,
            None,
            Some(instance.into()),
            None,
        )
        .map_err(|e| e.to_string())?;
        fit_window(hwnd, None, settings);
        let mut window = Window {
            theme: super::native_theme::Theme::new()?,
            font: HFONT::default(),
            controls: vec![],
            hwnd,
            owner,
            settings,
            search: HWND::default(),
            list: HWND::default(),
            detail: HWND::default(),
            startup: HWND::default(),
            rows: vec![],
            selected_key: None,
            selected_text: None,
            query: send,
            replies,
            generation: 0,
        };
        (if settings {
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
            self.startup = self.control(
                w!("BUTTON"),
                "Start at login",
                WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                STARTUP,
                [24, 112, 300, 32],
            )?;
            self.theme.apply_checkbox(self.startup);
            unsafe {
                SendMessageW(
                    self.startup,
                    BM_SETCHECK,
                    Some(WPARAM(usize::from(super::start_at_login_registered()?))),
                    None,
                );
            }
            self.control(
                w!("BUTTON"),
                "Save",
                WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                SAVE,
                [24, 176, 100, 32],
            )?;
            self.detail =
                self.control(w!("STATIC"), "", Default::default(), 0, [24, 224, 600, 64])?;
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
                0,
                [24, 66, 280, 20],
            )?;
            self.list = self.control(
                w!("LISTBOX"),
                "Transcripts",
                WS_TABSTOP
                    | WS_BORDER
                    | WS_VSCROLL
                    | WINDOW_STYLE(LBS_NOTIFY as u32 | LBS_NOINTEGRALHEIGHT as u32),
                LIST,
                [24, 88, 280, 480],
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
        let _ = self
            .query
            .send(Query::Search(self.generation, read(self.search)));
    }
    fn actions_enabled(&self, enabled: bool) {
        if self.settings {
            return;
        }
        for id in [COPY, EXPORT] {
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
                let _ = self.query.send(Query::Get(self.generation, key));
            }
        }
    }
    fn poll(&mut self) {
        self.selection_changed();
        while let Ok(reply) = self.replies.try_recv() {
            match reply {
                Reply::Rows(id, result) if id == self.generation => match result {
                    Ok(rows) => {
                        self.rows = rows;
                        unsafe {
                            SendMessageW(self.list, LB_RESETCONTENT, None, None);
                        }
                        for row in &self.rows {
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
                    Err(e) => set(self.detail, &e),
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
        match id {
            SEARCH if notification == EN_CHANGE as u16 => self.search(),
            LIST if notification == LBN_SELCHANGE as u16 => self.selection_changed(),
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
                    let mut edit = Settings::edit()?;
                    let old = super::start_at_login_registered()?;
                    super::set_start_at_login(enabled)?;
                    edit.settings.start_at_login = enabled;
                    if let Err(e) = edit.commit() {
                        let _ = super::set_start_at_login(old);
                        return Err(e);
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
                let height = (r.bottom - 112).max(80);
                for (id, x, y, w, h) in [
                    (COPY, left + 24, 32, 100, 32),
                    (EXPORT, left + 136, 32, 100, 32),
                    (24, left + 24, 66, (r.right - left - 48).max(80), 20),
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
                    self.scale(88),
                    self.scale(left - 24),
                    self.scale(height),
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
fn fit_window(hwnd: HWND, suggested: Option<&RECT>, settings: bool) {
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
            .unwrap_or(if settings { 300 } else { 680 } * dpi / 96)
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
    ) && let Some(result) = WINDOW.with(|cell| {
        cell.try_borrow().ok().and_then(|slot| {
            slot.as_ref().map(|window| unsafe {
                let dc = HDC(wparam.0 as *mut _);
                if message == WM_ERASEBKGND {
                    let mut bounds = RECT::default();
                    let _ = GetClientRect(hwnd, &mut bounds);
                    FillRect(dc, &bounds, window.theme.brush);
                    LRESULT(1)
                } else {
                    SetTextColor(dc, window.theme.foreground);
                    SetBkColor(dc, window.theme.background);
                    LRESULT(window.theme.brush.0 as isize)
                }
            })
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
                        window.theme = theme;
                        window.theme.apply(hwnd);
                        if window.settings {
                            window.theme.apply_checkbox(window.startup);
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
                    fit_window(hwnd, Some(suggested), window.settings);
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
fn export(hwnd: HWND, text: &str) -> Result<(), String> {
    let mut file = [0u16; 32768];
    for (dest, src) in file.iter_mut().zip("Transcript.txt".encode_utf16()) {
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
        std::fs::write(String::from_utf16_lossy(&file[..end]), text).map_err(|e| e.to_string())?;
    }
    Ok(())
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
