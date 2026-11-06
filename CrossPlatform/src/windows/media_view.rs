mod browser_source;
mod layout;
mod model;
mod options;
use crate::runtime::EngineCommand;
use std::cell::RefCell;
use std::sync::{
    LazyLock, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc::Sender,
};
use windows::{
    Win32::{Foundation::*, System::LibraryLoader::GetModuleHandleW, UI::WindowsAndMessaging::*},
    core::{PCWSTR, w},
};
static MISSING_SOURCE: AtomicBool = AtomicBool::new(false);
static COPIED: AtomicBool = AtomicBool::new(false);
static DISCARDED: AtomicBool = AtomicBool::new(false);
static WINDOW_HANDLE: AtomicUsize = AtomicUsize::new(0);
static OPEN: AtomicBool = AtomicBool::new(false);
static STATE: LazyLock<Mutex<model::Model>> = LazyLock::new(|| Mutex::new(model::Model::default()));
thread_local! {static VIEW: RefCell<Option<View>> = const { RefCell::new(None) };}
struct View {
    input: HWND,
    result: HWND,
    commands: Sender<EngineCommand>,
    seen_revision: Option<u64>,
    layout: layout::Layout,
}
pub(super) fn needs_copy() -> bool {
    STATE.lock().map(|state| state.needs_copy()).unwrap_or(true)
}
pub(super) fn discard() {
    if let Ok(mut state) = STATE.lock() {
        state.discard();
        DISCARDED.store(true, Ordering::Release);
    }
}
pub(super) fn take_copied() -> bool {
    COPIED.swap(false, Ordering::AcqRel)
}
pub(super) fn take_discarded() -> bool {
    DISCARDED.swap(false, Ordering::AcqRel)
}
pub(super) fn available(ready: bool) {
    if let Ok(mut state) = STATE.lock() {
        state.available(ready);
    }
}
pub(super) fn active() -> bool {
    STATE.lock().map(|state| state.active).unwrap_or(false)
}
pub(super) fn finish(
    status: &crate::state::AppStatus,
) -> Option<Result<model::Completion, String>> {
    STATE
        .lock()
        .ok()?
        .status(*status == crate::state::AppStatus::Ready)
}
pub(super) fn copy(hwnd: HWND) -> Result<(), String> {
    let text = STATE
        .lock()
        .map_err(|_| "Media result unavailable")?
        .result
        .clone();
    if text.is_empty() {
        return Err("No media result to copy".into());
    }
    super::copy_text(hwnd, &text)?;
    if let Ok(mut state) = STATE.lock() {
        state.copied();
        COPIED.store(true, Ordering::Release);
    }
    Ok(())
}
pub(super) fn notice(value: &str) {
    if let Ok(mut state) = STATE.lock() {
        state.notice(value);
    }
}
fn local_notice(value: &str) {
    if let Ok(mut state) = STATE.lock() {
        state.local_notice(value);
    }
}
fn request(commands: &Sender<EngineCommand>, source: String) {
    let accepted = STATE
        .lock()
        .map(|mut state| state.request())
        .unwrap_or(false);
    if accepted && commands.send(EngineCommand::Media(source)).is_err() {
        notice("@media-rejected:Engine unavailable");
    }
}
pub(super) fn focused(commands: Sender<EngineCommand>) {
    let expected = unsafe { GetForegroundWindow() }.0 as usize;
    if std::thread::Builder::new()
        .name("koett-media-source".into())
        .spawn(move || {
            if let Some(source) = browser_source::current_for(HWND(expected as *mut _)) {
                request(&commands, source);
            } else {
                MISSING_SOURCE.store(true, Ordering::Release);
                open(commands);
            }
        })
        .is_err()
    {
        local_notice("Could not read browser source");
    }
}
pub(super) fn open(commands: Sender<EngineCommand>) {
    if OPEN.swap(true, Ordering::AcqRel) {
        let window = HWND(WINDOW_HANDLE.load(Ordering::Acquire) as *mut _);
        if !window.is_invalid() {
            unsafe {
                let _ = ShowWindow(window, SW_RESTORE);
                let _ = SetForegroundWindow(window);
            }
        }
        return;
    }
    if std::thread::Builder::new()
        .name("koett-media-window".into())
        .spawn(move || {
            if let Err(error) = run(commands) {
                super::show_fatal_error(&error);
            }
            WINDOW_HANDLE.store(0, Ordering::Release);
            OPEN.store(false, Ordering::Release);
        })
        .is_err()
    {
        OPEN.store(false, Ordering::Release);
    }
}
fn run(commands: Sender<EngineCommand>) -> Result<(), String> {
    let source = browser_source::current();
    unsafe {
        let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
        let class = w!("KoettMediaWindow");
        let _ = RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: instance.into(),
            lpszClassName: class,
            ..Default::default()
        });
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class,
            w!("Transcribe media"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            680,
            460,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .map_err(|e| e.to_string())?;
        WINDOW_HANDLE.store(hwnd.0 as usize, Ordering::Release);
        let dpi = layout::dpi(hwnd) as i32;
        let monitor = windows::Win32::Graphics::Gdi::MonitorFromWindow(
            hwnd,
            windows::Win32::Graphics::Gdi::MONITOR_DEFAULTTONEAREST,
        );
        let mut monitor_info = windows::Win32::Graphics::Gdi::MONITORINFO {
            cbSize: std::mem::size_of::<windows::Win32::Graphics::Gdi::MONITORINFO>() as u32,
            ..Default::default()
        };
        if windows::Win32::Graphics::Gdi::GetMonitorInfoW(monitor, &mut monitor_info).as_bool() {
            let area = monitor_info.rcWork;
            let width = (680 * dpi / 96).min(area.right - area.left);
            let height = (500 * dpi / 96).min(area.bottom - area.top);
            let _ = SetWindowPos(
                hwnd,
                None,
                area.left + (area.right - area.left - width) / 2,
                area.top + (area.bottom - area.top - height) / 2,
                width,
                height,
                SWP_NOACTIVATE | SWP_NOZORDER,
            );
        }
        let _ = control(
            hwnd,
            w!("STATIC"),
            w!("URL or local media file"),
            Default::default(),
            106,
            [20, 0, 620, 20],
        )?;
        let input = control(
            hwnd,
            w!("EDIT"),
            w!(""),
            WS_BORDER | WS_TABSTOP | WINDOW_STYLE(0x80),
            101,
            [20, 20, 620, 30],
        )?;
        if let Some(source) = source {
            let wide: Vec<_> = source.encode_utf16().chain([0]).collect();
            let _ = SetWindowTextW(input, PCWSTR(wide.as_ptr()));
        }
        let _ = control(
            hwnd,
            w!("BUTTON"),
            w!("Browse…"),
            WS_TABSTOP,
            109,
            [520, 40, 100, 32],
        )?;
        let _ = control(
            hwnd,
            w!("BUTTON"),
            w!("Transcribe"),
            WS_TABSTOP,
            1,
            [20, 62, 120, 32],
        )?;
        let _ = control(
            hwnd,
            w!("BUTTON"),
            w!("Cancel"),
            WS_TABSTOP,
            2,
            [150, 62, 100, 32],
        )?;
        let _ = control(
            hwnd,
            w!("BUTTON"),
            w!("Copy"),
            WS_TABSTOP,
            104,
            [260, 62, 100, 32],
        )?;
        let _ = control(
            hwnd,
            w!("BUTTON"),
            w!("Options…"),
            WS_TABSTOP,
            110,
            [380, 82, 120, 32],
        )?;
        let _ = control(
            hwnd,
            w!("STATIC"),
            w!(""),
            Default::default(),
            108,
            [20, 120, 620, 40],
        )?;
        let _ = control(
            hwnd,
            w!("STATIC"),
            w!("Last transcript"),
            Default::default(),
            107,
            [20, 98, 620, 20],
        )?;
        let result = control(
            hwnd,
            w!("EDIT"),
            w!(""),
            WS_BORDER | WS_TABSTOP | WS_VSCROLL | WINDOW_STYLE(0x4 | 0x40 | 0x800),
            105,
            [20, 122, 620, 268],
        )?;
        let layout = layout::Layout::new(hwnd)?;
        VIEW.with(|view| {
            *view.borrow_mut() = Some(View {
                input,
                result,
                commands,
                seen_revision: None,
                layout,
            })
        });
        VIEW.with(|view| {
            if let Some(view) = view.borrow_mut().as_mut() {
                view.layout.resize(hwnd);
            }
        });
        SetTimer(Some(hwnd), 1, 200, None);
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            if message.message == WM_DPICHANGED || !IsDialogMessageW(hwnd, &message).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        VIEW.with(|view| view.borrow_mut().take());
    }
    Ok(())
}
unsafe fn control(
    parent: HWND,
    class: PCWSTR,
    title: PCWSTR,
    style: WINDOW_STYLE,
    id: usize,
    rect: [i32; 4],
) -> Result<HWND, String> {
    unsafe {
        CreateWindowExW(
            Default::default(),
            class,
            title,
            WS_CHILD | WS_VISIBLE | style,
            rect[0],
            rect[1],
            rect[2],
            rect[3],
            Some(parent),
            Some(HMENU(id as *mut _)),
            None,
            None,
        )
    }
    .map_err(|e| e.to_string())
}
unsafe extern "system" fn proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if let Some(result) = layout::Layout::paint(hwnd, message, wparam) {
            return result;
        }
        match message {
            WM_GETMINMAXINFO => {
                if lparam.0 != 0 {
                    let info = &mut *(lparam.0 as *mut MINMAXINFO);
                    let dpi = layout::dpi(hwnd) as i32;
                    info.ptMinTrackSize = POINT {
                        x: 560 * dpi / 96,
                        y: 360 * dpi / 96,
                    };
                }
                LRESULT(0)
            }
            WM_DPICHANGED => {
                if lparam.0 != 0 {
                    let rect = *(lparam.0 as *const RECT);
                    VIEW.with(|view| {
                        if let Ok(mut view) = view.try_borrow_mut()
                            && let Some(view) = view.as_mut()
                        {
                            view.layout
                                .change_dpi(hwnd, (wparam.0 & 0xffff) as u32, Some(rect));
                        }
                    });
                }
                LRESULT(0)
            }
            WM_SIZE | WM_THEMECHANGED | WM_SETTINGCHANGE => {
                VIEW.with(|view| {
                    if let Ok(mut view) = view.try_borrow_mut()
                        && let Some(view) = view.as_mut()
                    {
                        if matches!(message, WM_THEMECHANGED | WM_SETTINGCHANGE)
                            && let Ok(theme) = crate::windows::native_theme::Theme::new()
                        {
                            view.layout.theme = theme;
                        }
                        view.layout.resize(hwnd);
                    }
                });
                LRESULT(0)
            }

            WM_COMMAND => {
                VIEW.with(|view| {
                    if let Ok(view) = view.try_borrow()
                        && let Some(view) = view.as_ref()
                    {
                        match wparam.0 & 0xffff {
                            1 => {
                                let mut text =
                                    vec![0u16; GetWindowTextLengthW(view.input) as usize + 1];
                                let count = GetWindowTextW(view.input, &mut text);
                                let source = String::from_utf16_lossy(&text[..count as usize]);
                                if source.trim().is_empty() {
                                    local_notice("Enter an HTTP(S) URL or local media path");
                                } else {
                                    request(&view.commands, source);
                                }
                            }
                            110 => { options::show(hwnd); }
                            109 => {
                                use windows::Win32::UI::Controls::Dialogs::*;
                                let mut path=[0u16;32768];
                                let mut dialog=OPENFILENAMEW {lStructSize:std::mem::size_of::<OPENFILENAMEW>() as u32,hwndOwner:hwnd,lpstrFile:windows::core::PWSTR(path.as_mut_ptr()),nMaxFile:path.len() as u32,lpstrFilter:w!("Media files\0*.wav;*.mp3;*.mp4;*.mkv;*.webm;*.m4a;*.flac;*.ogg\0All files\0*.*\0\0"),Flags:OFN_FILEMUSTEXIST|OFN_PATHMUSTEXIST|OFN_NOCHANGEDIR,..Default::default()};
                                if GetOpenFileNameW(&mut dialog).as_bool(){let _=SetWindowTextW(view.input,PCWSTR(path.as_ptr()));}
                            }
                            2 => {
                                let active=STATE.lock().map(|state|state.active).unwrap_or(false);
                                if active {let _=view.commands.send(EngineCommand::CancelMedia);}else{let _=PostMessageW(Some(hwnd),WM_CLOSE,WPARAM(0),LPARAM(0));}
                            }
                            104 => {let _=copy(hwnd);}
                            _ => {}
                        }
                    }
                });
                LRESULT(0)
            }
            WM_TIMER => {
                VIEW.with(|view| {
                    if let Ok(mut view) = view.try_borrow_mut()
                        && let Some(view) = view.as_mut()
                    {
                        view.layout.reconcile_dpi(hwnd);
                        if MISSING_SOURCE.swap(false, Ordering::AcqRel) {
                            let _ = SetWindowTextW(view.input, w!(""));
                            if let Ok(mut state) = STATE.lock()
                                && !state.active
                                && !state.needs_copy()
                            {
                                state.local_notice(
                                    "Browser source unavailable · enter a URL or choose a file",
                                );
                            }
                        }
                        let snapshot = STATE.lock().ok().and_then(|state| {
                            (view.seen_revision != Some(state.revision)).then(|| state.clone())
                        });
                        if let Some(state) = snapshot {
                            let text: Vec<_> = state.result.encode_utf16().chain([0]).collect();
                            let _ = SetWindowTextW(view.result, PCWSTR(text.as_ptr()));
                            let status: Vec<_> = state.message.encode_utf16().chain([0]).collect();
                            if let Ok(label) = GetDlgItem(Some(hwnd), 108) {
                                let _ = SetWindowTextW(label, PCWSTR(status.as_ptr()));
                            }
                            for (id, enabled) in [
                                (1, state.ready && !state.active && !state.needs_copy()),
                                (2, true),
                                (104, !state.result.is_empty()),
                            ] {
                                if let Ok(button) = GetDlgItem(Some(hwnd), id) {
                                    let _ =
                                        windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow(
                                            button, enabled,
                                        );
                                }
                            }
                            if let Ok(button) = GetDlgItem(Some(hwnd), 2) {
                                let _ = SetWindowTextW(
                                    button,
                                    if state.active {
                                        w!("Cancel")
                                    } else {
                                        w!("Close")
                                    },
                                );
                            }
                            view.seen_revision = Some(state.revision);
                        }
                    }
                });
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}
