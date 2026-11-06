pub mod automation;
mod capture;
mod clipboard;
mod contact_sheet;
mod desktop;
mod draw_input;
mod drawing;
mod drawing_render;
mod foreground;
pub(crate) mod formatting_job;
pub mod media_cli;
pub(crate) mod media_control;
mod media_view;
mod meter;
mod native_theme;
mod paste_chord;
mod paste_guard;
mod paste_security;
mod paste_timing;
mod pill;
mod pill_buttons;
mod pill_state;
mod pin_policy;
mod pin_reader;
mod pixels;
mod recovery;
#[path = "linux/sounds.rs"]
mod sounds;

use std::cell::{Cell, RefCell};
use std::mem::size_of;
use std::ptr;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{
    COLORREF, CloseHandle, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, GetLastError,
    HANDLE, HWND, POINT, RECT,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateRoundRectRgn, CreateSolidBrush, DT_CENTER, DT_VCENTER, DeleteObject,
    DrawTextW, EndPaint, FillRect, GetMonitorInfoW, HGDIOBJ, InvalidateRect,
    MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow, PAINTSTRUCT, SetBkMode, SetTextColor,
    SetWindowRgn, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RRF_RT_REG_SZ,
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegGetValueW, RegSetValueExW,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForMonitor, MDT_EFFECTIVE_DPI,
    SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyState, HOT_KEY_MODIFIERS, INPUT, INPUT_0, INPUT_KEYBOARD,
    KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT,
    MOD_WIN, RegisterHotKey, SendInput, UnregisterHotKey, VIRTUAL_KEY, VK_CONTROL, VK_LWIN,
    VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::Shell::{
    NIF_GUID, NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NIM_SETFOCUS, NIM_SETVERSION, NIN_SELECT, NINF_KEY, NOTIFYICON_VERSION_4, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CW_USEDEFAULT, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
    DestroyWindow, DispatchMessageW, GetClientRect, GetCursorPos, GetForegroundWindow, GetMessageW,
    GetWindowThreadProcessId, HWND_TOPMOST, IDI_APPLICATION, IsWindowVisible, KillTimer, LoadIconW,
    MA_NOACTIVATE, MF_CHECKED, MF_DISABLED, MF_SEPARATOR, MF_STRING, MSG, MessageBoxW,
    PostQuitMessage, RegisterClassExW, RegisterWindowMessageW, SW_HIDE, SW_SHOWNOACTIVATE,
    SW_SHOWNORMAL, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetForegroundWindow, SetTimer, SetWindowPos,
    ShowWindow, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_CLOSE, WM_CONTEXTMENU, WM_DESTROY, WM_HOTKEY, WM_KEYDOWN, WM_LBUTTONUP,
    WM_MOUSEACTIVATE, WM_PAINT, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_TIMER, WNDCLASSEXW,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_OVERLAPPEDWINDOW, WS_POPUP,
};
use windows::core::{GUID, PCWSTR, w};

use crate::audio::AudioLevels;
use crate::history;
use crate::runtime::{self, EngineCommand, EngineHandle, EngineUpdate};
use crate::settings::Settings;
use crate::shortcut::{Key, Modifiers, Shortcut};
use crate::state::{AppStatus, RecordingIntent, ShortcutLatch};

const HOTKEY_ID: i32 = 1;
const MEDIA_HOTKEY_ID: i32 = 5;
const POLL_TIMER_ID: usize = 1;
const WM_CHANGE_MODEL: u32 = 0x8054;
const WM_CHANGE_FORMATTING: u32 = 0x8055;
const WM_CANCEL_SETUP: u32 = 0x8056;
static WM_OPEN_MEDIA: std::sync::LazyLock<u32> =
    std::sync::LazyLock::new(|| unsafe { RegisterWindowMessageW(w!("Koett.OpenMedia.v1")) });
const TRAY_MESSAGE: u32 = 0x8001;
const NIN_KEYSELECT: u32 = NIN_SELECT | NINF_KEY;
const MENU_CHANGE_SHORTCUT: usize = 1;
const MENU_OPEN_HISTORY: usize = 2;
const MENU_START_AT_LOGIN: usize = 3;
const MENU_QUIT: usize = 4;
const MENU_SETTINGS: usize = 5;
const MENU_COPY_LAST: usize = 6;
const MENU_PAUSE: usize = 7;
const MENU_CANCEL: usize = 8;
const MENU_RECORD: usize = 9;
const MENU_WORDS: usize = 10;
const MENU_DRAW: usize = 11;
const MENU_UNDO_DRAW: usize = 12;
const MENU_PASTE_LAST: usize = 13;
const MENU_MEDIA: usize = 14;
const MENU_CHANGE_MEDIA_SHORTCUT: usize = 15;
const MENU_RECOVERY: usize = 16;
const TRAY_GUID: GUID = GUID::from_u128(0xc65ddae4_119c_486e_8bed_e30a24f2fb1f);

// All Koett windows run on the thread that owns this RefCell. Win32 can re-enter
// a window procedure while a menu or dialog is open, so callbacks use
// try_borrow instead of creating overlapping references from a raw pointer.
static APP: AtomicPtr<RefCell<WindowsApp>> = AtomicPtr::new(ptr::null_mut());

#[derive(Clone, Copy, PartialEq, Eq)]
struct PasteTarget {
    hwnd: HWND,
    pid: u32,
}

struct WindowsApp {
    hwnd: HWND,
    engine: Option<EngineHandle>,
    desktop_lab: bool,
    settings: Settings,
    status: AppStatus,
    paste_target: Option<PasteTarget>,
    last_target: Option<PasteTarget>,
    overlay: HWND,
    pill_renderer: RefCell<Option<pill::Renderer>>,
    last_render_error: Cell<Option<Instant>>,
    pill_buttons: Option<pill_buttons::Buttons>,
    meter: meter::Meter,
    pill_state: pill_state::State,
    clock: Instant,
    last_transcript: String,
    pill_media: bool,
    last_pill_redraw: Cell<Option<Instant>>,
    last_pill_key: Cell<Option<pill_state::PaintKey>>,
    shortcut_window: HWND,
    registered_shortcut: Option<Shortcut>,
    registered_media_shortcut: Option<Shortcut>,
    changing_media_shortcut: bool,
    shortcut_latch: ShortcutLatch,
    levels: Option<AudioLevels>,
    recording_started: Option<Instant>,
    draw_input: Option<draw_input::Monitor>,
    drawing: Option<drawing::Board>,
    draw_locked: bool,
    draw_lock_pending: bool,
    pill_pressed: bool,
    pending_result: Option<EngineUpdate>,
    staged: Option<StagedPaste>,
    last_bundle: Option<Rc<clipboard::Bundle>>,
    clipboard_restore: Option<ClipboardRestore>,
    draw_pointer: POINT,
    capture_lab: Option<PendingCapture>,
    capture_pending: bool,
    tray: NOTIFYICONDATAW,
    taskbar_created: u32,
}

struct PendingCapture {
    started: Instant,
    clipboard_sequence: u32,
    result: std::sync::mpsc::Receiver<Result<pixels::Pixels, String>>,
}

struct StagedPaste {
    offer: clipboard::Offer,
    target: PasteTarget,
    started: Instant,
    model: String,
    text: String,
    raw: Option<String>,
    saved: Option<bool>,
    text_shortcut: crate::settings::PasteShortcut,
}

struct ClipboardRestore {
    bundle: Rc<clipboard::Bundle>,
    offer: clipboard::Offer,
    target_pid: u32,
    started: Instant,
    posted: bool,
}

pub fn run() -> Result<(), String> {
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() }
        .map_err(|e| format!("Could not initialize the Windows desktop: {e}"))?;
    struct ComApartment;
    impl Drop for ComApartment {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    let _apartment = ComApartment;
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    let controls = windows::Win32::UI::Controls::INITCOMMONCONTROLSEX {
        dwSize: size_of::<windows::Win32::UI::Controls::INITCOMMONCONTROLSEX>() as u32,
        dwICC: windows::Win32::UI::Controls::ICC_STANDARD_CLASSES,
    };
    if !unsafe { windows::Win32::UI::Controls::InitCommonControlsEx(&controls) }.as_bool() {
        return Err("Could not initialize Windows controls".into());
    }
    let Some(mutex) = single_instance()? else {
        return Ok(());
    };
    let settings = Settings::load()?;
    let shortcut = Shortcut::from_str(&settings.shortcut)?;
    let startup_warning: Option<String> = None;
    history::recovery::initialize_last()?;
    let restored_last = history::recovery::last().unwrap_or_default();
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let desktop_lab = arguments.as_slice() == [std::ffi::OsString::from("--desktop-lab")];
    let engine = match arguments.as_slice() {
        [] => Some(runtime::start(settings.clone())),
        [flag] if flag == "media" => Some(runtime::start(settings.clone())),
        [flag] if flag == "--microphone-open-timeout-lab" => Some(runtime::start(settings.clone())),
        [_] if desktop_lab => None,
        [flag, path] if flag == "--dictation-lab-wav" => {
            Some(runtime::start_wav_lab(settings.clone(), path.into(), false))
        }
        [flag, path] if flag == "--dictation-lab-asr-failure" => {
            Some(runtime::start_wav_lab(settings.clone(), path.into(), true))
        }
        _ => return Err("Use Koett.exe, --desktop-lab, --dictation-lab-wav FILE.wav, or --dictation-lab-asr-failure FILE.wav".into()),
    };
    let app = Box::new(RefCell::new(WindowsApp {
        hwnd: HWND::default(),
        engine,
        desktop_lab,
        settings,
        status: if desktop_lab {
            AppStatus::Ready
        } else {
            AppStatus::Starting
        },
        paste_target: None,
        last_target: None,
        overlay: HWND::default(),
        pill_renderer: RefCell::new(None),
        last_render_error: Cell::new(None),
        pill_buttons: None,
        meter: meter::Meter::default(),
        pill_state: pill_state::State::default(),
        clock: Instant::now(),
        last_transcript: restored_last,
        pill_media: false,
        last_pill_redraw: Cell::new(None),
        last_pill_key: Cell::new(None),
        shortcut_window: HWND::default(),
        registered_shortcut: None,
        registered_media_shortcut: None,
        changing_media_shortcut: false,
        shortcut_latch: ShortcutLatch::default(),
        levels: None,
        recording_started: None,
        draw_input: None,
        drawing: None,
        draw_locked: false,
        draw_lock_pending: false,
        pill_pressed: false,
        pending_result: None,
        staged: None,
        last_bundle: None,
        clipboard_restore: None,
        draw_pointer: POINT::default(),
        capture_lab: None,
        capture_pending: false,
        tray: NOTIFYICONDATAW::default(),
        taskbar_created: 0,
    }));

    let result = unsafe { run_message_loop(&app, shortcut, startup_warning.as_deref()) };
    APP.store(ptr::null_mut(), Ordering::Release);
    let mut app = app.borrow_mut();
    if let Some(engine) = app.engine.take() {
        engine.stop();
    }
    unsafe {
        app.remove_tray();
        if !app.hwnd.is_invalid() {
            let _ = DestroyWindow(app.hwnd);
        }
        if !app.overlay.is_invalid() {
            let _ = DestroyWindow(app.overlay);
        }
        if !app.shortcut_window.is_invalid() {
            let _ = DestroyWindow(app.shortcut_window);
        }
        let _ = CloseHandle(mutex);
    }
    result
}

pub fn show_fatal_error(message: &str) {
    let message = wide(message);
    unsafe {
        let _ = MessageBoxW(
            None,
            PCWSTR::from_raw(message.as_ptr()),
            w!("Koett"),
            Default::default(),
        );
    }
}

fn single_instance() -> Result<Option<HANDLE>, String> {
    let handle = unsafe { CreateMutexW(None, true, w!("Local\\Koett")) }
        .map_err(|error| format!("could not create the Koett process lock: {error}"))?;
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe {
            let _ = CloseHandle(handle);
            if std::env::args().nth(1).as_deref() == Some("media")
                && let Ok(owner) = windows::Win32::UI::WindowsAndMessaging::FindWindowW(
                    w!("KoettOwnerWindow"),
                    None,
                )
            {
                let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                    Some(owner),
                    *WM_OPEN_MEDIA,
                    Default::default(),
                    Default::default(),
                );
            }
        }
        return Ok(None);
    }
    Ok(Some(handle))
}

unsafe fn run_message_loop(
    app_cell: &RefCell<WindowsApp>,
    shortcut: Shortcut,
    startup_warning: Option<&str>,
) -> Result<(), String> {
    let mut app = app_cell.borrow_mut();
    let instance = unsafe { GetModuleHandleW(None) }
        .map_err(|error| format!("could not find the Koett module: {error}"))?;
    let class = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(window_proc),
        hInstance: instance.into(),
        lpszClassName: w!("KoettOwnerWindow"),
        ..Default::default()
    };
    if unsafe { RegisterClassExW(&class) } == 0 {
        return Err(format!(
            "could not register the Koett window: {:?}",
            unsafe { GetLastError() }
        ));
    }
    let overlay_class = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(overlay_proc),
        hInstance: instance.into(),
        lpszClassName: w!("KoettOverlayWindow"),
        ..Default::default()
    };
    if unsafe { RegisterClassExW(&overlay_class) } == 0 {
        return Err(format!(
            "could not register the Koett overlay: {:?}",
            unsafe { GetLastError() }
        ));
    }
    let shortcut_class = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(shortcut_proc),
        hInstance: instance.into(),
        lpszClassName: w!("KoettShortcutWindow"),
        ..Default::default()
    };
    if unsafe { RegisterClassExW(&shortcut_class) } == 0 {
        return Err(format!(
            "could not register the Koett shortcut window: {:?}",
            unsafe { GetLastError() }
        ));
    }

    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("KoettOwnerWindow"),
            w!("Koett"),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance.into()),
            None,
        )
    }
    .map_err(|error| format!("could not create the Koett window: {error}"))?;
    app.hwnd = hwnd;
    app.overlay = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW
                | WS_EX_TOPMOST
                | WS_EX_NOACTIVATE
                | windows::Win32::UI::WindowsAndMessaging::WS_EX_NOREDIRECTIONBITMAP,
            w!("KoettOverlayWindow"),
            w!("Koett Recording"),
            WS_POPUP,
            0,
            0,
            300,
            48,
            None,
            None,
            Some(instance.into()),
            None,
        )
    }
    .map_err(|error| format!("could not create the Koett recording pill: {error}"))?;
    app.pill_buttons = Some(pill_buttons::Buttons::new(app.overlay)?);
    app.shortcut_window = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            w!("KoettShortcutWindow"),
            w!("Change Koett Shortcut"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            390,
            130,
            Some(hwnd),
            None,
            Some(instance.into()),
            None,
        )
    }
    .map_err(|error| format!("could not create the Koett shortcut window: {error}"))?;
    APP.store(ptr::from_ref(app_cell).cast_mut(), Ordering::Release);
    app.taskbar_created = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
    app.add_tray()?;

    let (modifiers, virtual_key) = windows_shortcut(shortcut);
    if unsafe { RegisterHotKey(Some(hwnd), HOTKEY_ID, modifiers | MOD_NOREPEAT, virtual_key) }
        .is_ok()
    {
        app.registered_shortcut = Some(shortcut);
    }
    if let Ok(media_shortcut) = Shortcut::from_str(&app.settings.media_shortcut) {
        let (modifiers, key) = windows_shortcut(media_shortcut);
        if unsafe { RegisterHotKey(Some(hwnd), MEDIA_HOTKEY_ID, modifiers | MOD_NOREPEAT, key) }
            .is_ok()
        {
            app.registered_media_shortcut = Some(media_shortcut);
        } else {
            app.pill_state.set(
                pill_state::Phase::Error {
                    message: "Media shortcut unavailable · use tray".into(),
                    rescue: false,
                },
                0,
            );
        }
    }
    app.update_tray();
    if unsafe { SetTimer(Some(hwnd), POLL_TIMER_ID, 33, None) } == 0 {
        return Err("could not start the Koett event timer".to_string());
    }
    drop(app);
    let _foreground = match foreground::Tracker::start() {
        Ok(tracker) => Some(tracker),
        Err(error) => {
            eprintln!("Koett: {error}");
            app_cell
                .borrow_mut()
                .notify_drawing("Tray Paste unavailable. Use Paste in the pill.");
            None
        }
    };
    if let Some(error) = startup_warning {
        show_fatal_error(&format!(
            "Koett started, but start at login could not be enabled: {error}"
        ));
    }

    if std::env::args().nth(1).as_deref() == Some("media")
        && let Some(engine) = &app_cell.borrow().engine
    {
        media_view::open(engine.commands.clone());
    }
    let mut message = MSG::default();
    loop {
        let result = unsafe { GetMessageW(&mut message, None, 0, 0) }.0;
        if result == 0 {
            break;
        }
        if result == -1 {
            return Err("the Windows message loop failed".to_string());
        }
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }

    unsafe {
        let _ = KillTimer(Some(hwnd), POLL_TIMER_ID);
        if app_cell.borrow().registered_shortcut.is_some() {
            let _ = UnregisterHotKey(Some(hwnd), HOTKEY_ID);
        }
    }
    Ok(())
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    if let Some(result) = automation::dispatch(hwnd, message, wparam, lparam) {
        return result;
    }
    if *WM_OPEN_MEDIA != 0 && message == *WM_OPEN_MEDIA {
        let pointer = APP.load(Ordering::Acquire);
        if let Some(cell) = unsafe { pointer.as_ref() }
            && let Ok(app) = cell.try_borrow()
            && let Some(engine) = &app.engine
        {
            media_view::open(engine.commands.clone());
        }
        return Default::default();
    }
    if message == windows::Win32::UI::WindowsAndMessaging::WM_RENDERFORMAT {
        clipboard::render(wparam.0 as u32);
        return Default::default();
    }
    if message == windows::Win32::UI::WindowsAndMessaging::WM_RENDERALLFORMATS {
        clipboard::render_all(hwnd);
        return Default::default();
    }
    if message == windows::Win32::UI::WindowsAndMessaging::WM_DESTROYCLIPBOARD {
        clipboard::forget_delayed();
    }
    let app_cell = APP.load(Ordering::Acquire);
    if !app_cell.is_null() {
        let app_cell = unsafe { &*app_cell };
        match message {
            0x8058 => {
                if let Ok(mut app) = app_cell.try_borrow_mut()
                    && app.desktop_lab
                    && app.capture_lab.is_none()
                {
                    let sequence = unsafe {
                        windows::Win32::System::DataExchange::GetClipboardSequenceNumber()
                    };
                    if let Err(error) = capture::exclude_own_windows() {
                        let label = wide(&error);
                        let _ = unsafe {
                            windows::Win32::UI::WindowsAndMessaging::SetWindowTextW(
                                hwnd,
                                PCWSTR(label.as_ptr()),
                            )
                        };
                        return Default::default();
                    }
                    let monitor = unsafe {
                        MonitorFromWindow(GetForegroundWindow(), MONITOR_DEFAULTTONEAREST)
                    }
                    .0 as usize;
                    let (tx, rx) = std::sync::mpsc::sync_channel(1);
                    app.capture_lab = Some(PendingCapture {
                        started: Instant::now(),
                        clipboard_sequence: sequence,
                        result: rx,
                    });
                    std::thread::spawn(move || {
                        let _ = tx.send(capture::monitor(monitor));
                    });
                }
                return Default::default();
            }
            0x8059 => {
                if let Ok(mut app) = app_cell.try_borrow_mut()
                    && app.desktop_lab
                {
                    if wparam.0 == 0 {
                        app.draw_input = None;
                    } else if app.status == AppStatus::Recording {
                        app.draw_input =
                            draw_input::Monitor::start(hwnd, crate::settings::DrawKey::Shift).ok();
                        app.drawing = drawing::Board::new(
                            true,
                            app.recording_started.unwrap_or_else(Instant::now),
                        )
                        .ok();
                    }
                }
                return Default::default();
            }
            windows::Win32::UI::WindowsAndMessaging::WM_INPUT => {
                if let Ok(mut app) = app_cell.try_borrow_mut()
                    && let Some(input) = app.draw_input.as_mut()
                {
                    input.input(lparam, Instant::now());
                }
                if let Ok(mut app) = app_cell.try_borrow_mut() {
                    app.poll_drawing();
                    app.poll_staged();
                }
            }
            windows::Win32::UI::WindowsAndMessaging::WM_INPUT_DEVICE_CHANGE => {
                if let Ok(mut app) = app_cell.try_borrow_mut()
                    && let Some(input) = app.draw_input.as_mut()
                {
                    input.device_changed();
                }
                if let Ok(mut app) = app_cell.try_borrow_mut()
                    && app.draw_locked
                {
                    app.unlock_drawing();
                    app.notify_drawing("Keyboard changed; draw lock ended");
                }
            }
            0x8057 => {
                if let Ok(app) = app_cell.try_borrow()
                    && app.desktop_lab
                {
                    let label = wide(&match clipboard::lab_bundle(hwnd, wparam.0) {
                        Ok(()) => "Clipboard ready".into(),
                        Err(error) => error,
                    });
                    let _ = unsafe {
                        windows::Win32::UI::WindowsAndMessaging::SetWindowTextW(
                            hwnd,
                            PCWSTR(label.as_ptr()),
                        )
                    };
                }
                return Default::default();
            }
            WM_CHANGE_MODEL | WM_CHANGE_FORMATTING | WM_CANCEL_SETUP => {
                if let Ok(mut app) = app_cell.try_borrow_mut() {
                    app.processing_request(message, wparam.0);
                }
                return Default::default();
            }
            0x8053 => {
                if let Ok(mut app) = app_cell.try_borrow_mut()
                    && let Ok(settings) = Settings::load()
                {
                    app.settings = settings;
                }
                return Default::default();
            }
            0x8052 => {
                if let Ok(mut app) = app_cell.try_borrow_mut()
                    && app.desktop_lab
                {
                    app.lab_phase(wparam.0);
                }
                return Default::default();
            }
            windows::Win32::UI::WindowsAndMessaging::WM_COMMAND => {
                if let Ok(mut app) = app_cell.try_borrow_mut() {
                    app.handle_menu(wparam.0 & 0xffff);
                }
                return Default::default();
            }
            WM_HOTKEY
                if (wparam.0 == HOTKEY_ID as usize || wparam.0 == MEDIA_HOTKEY_ID as usize)
                    && app_cell.try_borrow().is_ok_and(|app| {
                        unsafe { IsWindowVisible(app.shortcut_window) }.as_bool()
                    }) =>
            {
                if let Ok(mut app) = app_cell.try_borrow_mut() {
                    let shortcut = if wparam.0 == HOTKEY_ID as usize {
                        app.registered_shortcut
                    } else {
                        app.registered_media_shortcut
                    };
                    if let Some(shortcut) = shortcut {
                        if app.changing_media_shortcut {
                            app.change_media_shortcut(shortcut);
                        } else {
                            app.change_shortcut(shortcut);
                        }
                    }
                }
                return Default::default();
            }
            WM_HOTKEY if wparam.0 == MEDIA_HOTKEY_ID as usize => {
                if let Ok(app) = app_cell.try_borrow()
                    && let Some(engine) = &app.engine
                {
                    if app.status == AppStatus::Ready && !media_view::active() {
                        media_view::focused(engine.commands.clone());
                    } else {
                        media_view::open(engine.commands.clone());
                    }
                }
                return Default::default();
            }
            WM_HOTKEY if (2..=4).contains(&wparam.0) => {
                if let Ok(mut app) = app_cell.try_borrow_mut() {
                    match wparam.0 {
                        2 if app.status == AppStatus::Recording && app.drawing.is_some() => {
                            if app.draw_locked || app.draw_lock_pending {
                                app.unlock_drawing();
                            } else {
                                app.draw_lock_pending = true;
                            }
                        }
                        3 if app.draw_locked || app.draw_lock_pending => app.unlock_drawing(),
                        3 => app.handle_menu(MENU_CANCEL),
                        4 if app.draw_locked => {
                            if let Some(board) = app.drawing.as_mut() {
                                board.undo_stroke();
                            }
                        }
                        _ => {}
                    }
                }
                return Default::default();
            }
            WM_HOTKEY if wparam.0 == HOTKEY_ID as usize => {
                if let Ok(mut app) = app_cell.try_borrow_mut()
                    && matches!(
                        app.status,
                        AppStatus::Ready | AppStatus::Recording | AppStatus::Error(_)
                    )
                {
                    let mode = app.settings.recording_mode;
                    if let Some(intent) = app.shortcut_latch.press(mode) {
                        app.recording_intent(intent);
                    }
                }
                return Default::default();
            }
            WM_TIMER if wparam.0 == POLL_TIMER_ID => {
                if let Ok(mut app) = app_cell.try_borrow_mut() {
                    let app = &mut *app;
                    app.poll_updates();
                    app.poll_drawing();
                    app.poll_staged();
                    media_view::available(app.status == AppStatus::Ready);
                    if media_view::take_copied()
                        && app.pill_media
                        && !media_view::active()
                        && matches!(app.pill_state.phase, pill_state::Phase::Done(_))
                        && !app.pill_state.copied
                    {
                        let now = app.clock.elapsed().as_millis() as u64;
                        app.pill_state.copy(now);
                        app.update_overlay();
                    }
                    if media_view::take_discarded() && app.pill_media && !media_view::active() {
                        app.pill_state.set(
                            pill_state::Phase::Hidden,
                            app.clock.elapsed().as_millis() as u64,
                        );
                        app.update_overlay();
                    }
                    if let Some(input) = app.draw_input.as_mut() {
                        let mut point = POINT::default();
                        let _ = unsafe { GetCursorPos(&mut point) };
                        if point != app.draw_pointer {
                            input.moved(Instant::now());
                            app.draw_pointer = point;
                        }
                        let state = input.state(Instant::now());
                        if app.desktop_lab {
                            let label = wide(&format!(
                                "Draw key: clicks={} ink={} audio_ms={}",
                                state.protect_clicks,
                                state.ink,
                                app.levels.as_ref().map_or(0, AudioLevels::recorded_ms)
                            ));
                            let _ = unsafe {
                                windows::Win32::UI::WindowsAndMessaging::SetWindowTextW(
                                    hwnd,
                                    windows::core::PCWSTR(label.as_ptr()),
                                )
                            };
                        }
                    }
                    if let Some(result) = app.capture_lab.as_ref().and_then(|pending| {
                        pending.result.try_recv().ok().or_else(|| {
                            (pending.started.elapsed() >= Duration::from_secs(3))
                                .then(|| Err("Screen capture timed out".into()))
                        })
                    }) {
                        let sequence = app
                            .capture_lab
                            .as_ref()
                            .map(|pending| pending.clipboard_sequence)
                            .unwrap_or(0);
                        app.capture_lab = None;
                        let result = result
                            .and_then(|image| {
                                clipboard::Bundle::new(&image, "M4 captured display".into())
                            })
                            .and_then(|b| b.copy_if_unchanged(hwnd, sequence).map(|_| ()));
                        let label = wide(&match result {
                            Ok(()) => "Capture ready".into(),
                            Err(e) => e,
                        });
                        let _ = unsafe {
                            windows::Win32::UI::WindowsAndMessaging::SetWindowTextW(
                                hwnd,
                                windows::core::PCWSTR(label.as_ptr()),
                            )
                        };
                    }
                    if app
                        .registered_shortcut
                        .is_none_or(|shortcut| !shortcut_is_down(shortcut))
                        && let Some(intent) = app.shortcut_latch.release()
                    {
                        app.recording_intent(intent);
                    }
                    if app.status == AppStatus::Recording {
                        let elapsed = app
                            .recording_started
                            .map(|s| s.elapsed().as_millis() as u64)
                            .unwrap_or(0);
                        let rms = app
                            .levels
                            .as_ref()
                            .map(AudioLevels::current)
                            .map(|v| v.0)
                            .unwrap_or(0.0);
                        app.meter.sample(elapsed, rms);
                    }
                    app.tick_pill();
                }
                return Default::default();
            }
            TRAY_MESSAGE => {
                let event = lparam.0 as u32 & 0xffff;
                let anchor =
                    matches!(event, NIN_SELECT | NIN_KEYSELECT).then(|| tray_anchor(wparam.0));
                if matches!(
                    event,
                    WM_RBUTTONUP | WM_LBUTTONUP | WM_CONTEXTMENU | NIN_SELECT | NIN_KEYSELECT
                ) && let Ok(mut app) = app_cell.try_borrow_mut()
                {
                    app.show_menu(anchor);
                }
                return Default::default();
            }
            windows::Win32::UI::WindowsAndMessaging::WM_QUERYENDSESSION => {
                let busy = app_cell
                    .try_borrow()
                    .is_ok_and(|app| app.dictation_in_flight());
                return windows::Win32::Foundation::LRESULT(isize::from(
                    !busy && !media_view::needs_copy() && history::recovery::pending_count() == 0,
                ));
            }
            WM_CLOSE => {
                if let Ok(app) = app_cell.try_borrow() {
                    app.request_quit();
                }
                return Default::default();
            }
            WM_DESTROY => {
                unsafe { PostQuitMessage(0) };
                return Default::default();
            }
            _ => {}
        }
        if let Ok(mut app) = app_cell.try_borrow_mut()
            && message == app.taskbar_created
        {
            let _ = app.add_tray();
            app.update_tray();
            return Default::default();
        }
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

unsafe extern "system" fn overlay_proc(
    hwnd: HWND,
    message: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    if matches!(
        message,
        windows::Win32::UI::WindowsAndMessaging::WM_DESTROY
            | windows::Win32::UI::WindowsAndMessaging::WM_NCDESTROY
    ) {
        pill_buttons::destroyed(hwnd);
    }
    if message == windows::Win32::UI::WindowsAndMessaging::WM_GETOBJECT
        && let Some(result) = pill_buttons::object(hwnd, wparam, lparam)
    {
        return result;
    }
    if message == windows::Win32::UI::WindowsAndMessaging::WM_COMMAND {
        let cell = APP.load(Ordering::Acquire);
        if !cell.is_null()
            && let Ok(mut app) = unsafe { &*cell }.try_borrow_mut()
        {
            let x = match wparam.0 & 0xffff {
                101 => 50,
                102 => 120,
                103 => 200,
                104 => 260,
                105 => 210,
                106 => 20,
                _ => -1,
            };
            if x >= 0
                && app
                    .pill_buttons
                    .as_ref()
                    .is_some_and(|buttons| buttons.allows(wparam.0 & 0xffff))
            {
                app.pill_click(x, true);
            }
        }
        return Default::default();
    }
    if message == WM_MOUSEACTIVATE {
        return windows::Win32::Foundation::LRESULT(MA_NOACTIVATE as isize);
    }
    if message == windows::Win32::UI::WindowsAndMessaging::WM_LBUTTONDOWN {
        let cell = APP.load(Ordering::Acquire);
        if !cell.is_null()
            && let Ok(mut app) = unsafe { &*cell }.try_borrow_mut()
        {
            app.pill_pressed = true;
            unsafe {
                windows::Win32::UI::Input::KeyboardAndMouse::SetCapture(hwnd);
            }
        }
        return Default::default();
    }
    if message == WM_LBUTTONUP {
        let cell = APP.load(Ordering::Acquire);
        if !cell.is_null()
            && let Ok(mut app) = unsafe { &*cell }.try_borrow_mut()
        {
            let pressed = std::mem::take(&mut app.pill_pressed);
            let _ = unsafe { windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture() };
            let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) }.max(96);
            let x = ((lparam.0 as i16) as i32) * 96 / dpi as i32;
            let y = (((lparam.0 >> 16) as i16) as i32) * 96 / dpi as i32;
            if pressed && (0..300).contains(&x) && (0..48).contains(&y) {
                app.pill_click(x, false);
            }
        }
        return Default::default();
    }
    if message == WM_PAINT {
        let app_cell = APP.load(Ordering::Acquire);
        if !app_cell.is_null()
            && let Ok(app) = unsafe { &*app_cell }.try_borrow()
        {
            unsafe { paint_overlay(hwnd, &app) };
            return Default::default();
        }
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

unsafe extern "system" fn shortcut_proc(
    hwnd: HWND,
    message: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    let app_cell = APP.load(Ordering::Acquire);
    match message {
        WM_KEYDOWN if wparam.0 == 0x1b => {
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
            return Default::default();
        }
        WM_KEYDOWN | WM_SYSKEYDOWN if !app_cell.is_null() => {
            if let Some(shortcut) = shortcut_from_virtual_key(wparam.0 as u32)
                && let Ok(mut app) = unsafe { &*app_cell }.try_borrow_mut()
            {
                if app.changing_media_shortcut {
                    app.change_media_shortcut(shortcut);
                } else {
                    app.change_shortcut(shortcut);
                }
            }
            return Default::default();
        }
        WM_PAINT => {
            unsafe { paint_shortcut_window(hwnd) };
            return Default::default();
        }
        WM_CLOSE => {
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
            return Default::default();
        }
        _ => {}
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

impl WindowsApp {
    fn unlock_drawing(&mut self) {
        self.draw_locked = false;
        self.draw_lock_pending = false;
        let _ = unsafe { UnregisterHotKey(Some(self.hwnd), 4) };
        let audio = self.levels.as_ref().map_or(0, AudioLevels::recorded_ms);
        if let Some(board) = self.drawing.as_mut() {
            board.end_hold(audio);
        }
    }
    fn recording_hotkeys(&mut self) {
        if unsafe { RegisterHotKey(Some(self.hwnd), 3, MOD_NOREPEAT, 0x1b) }.is_err() {
            self.notify_drawing("Escape shortcut unavailable; use Cancel in the pill or tray");
        }
        if self.drawing.is_some() && !self.settings.draw_lock_shortcut.is_empty() {
            match self.settings.draw_lock_shortcut.parse::<Shortcut>() {
                Ok(shortcut) if self.registered_shortcut != Some(shortcut) => {
                    let (modifiers, key) = windows_shortcut(shortcut);
                    if unsafe { RegisterHotKey(Some(self.hwnd), 2, modifiers | MOD_NOREPEAT, key) }
                        .is_err()
                    {
                        self.notify_drawing(
                            "Draw lock shortcut unavailable; change it in Settings",
                        );
                    }
                }
                _ => self.notify_drawing(
                    "Choose a draw lock shortcut different from dictation in Settings",
                ),
            }
        }
    }
    fn notify_drawing(&mut self, message: &str) {
        set_wide_array(&mut self.tray.szInfoTitle, "Koett");
        set_wide_array(&mut self.tray.szInfo, message);
        self.tray.uFlags |= windows::Win32::UI::Shell::NIF_INFO;
        let _ = unsafe { Shell_NotifyIconW(NIM_MODIFY, &self.tray) };
        self.tray.uFlags &= !windows::Win32::UI::Shell::NIF_INFO;
    }
    fn poll_drawing(&mut self) {
        let now = Instant::now();
        let paused = self.levels.as_ref().is_some_and(AudioLevels::paused);
        if self.draw_lock_pending
            && ![VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN]
                .iter()
                .any(|key| unsafe { GetAsyncKeyState(i32::from(key.0)) } < 0)
        {
            self.draw_lock_pending = false;
            self.draw_locked = true;
            if unsafe {
                RegisterHotKey(Some(self.hwnd), 4, MOD_CONTROL | MOD_NOREPEAT, b'Z' as u32)
            }
            .is_err()
            {
                self.notify_drawing("Undo shortcut unavailable; use right-click while locked or Undo drawing in the tray");
            }
        }
        let state = if paused {
            if let Some(input) = self.draw_input.as_mut() {
                input.suspend();
            }
            draw_input::State::default()
        } else if self.draw_locked {
            draw_input::State {
                protect_clicks: true,
                ink: true,
                locked: true,
                discard: false,
            }
        } else {
            self.draw_input
                .as_mut()
                .map_or_else(Default::default, |input| input.state(now))
        };
        let audio_ms = self.levels.as_ref().map_or(0, AudioLevels::recorded_ms);
        if let Some(board) = self.drawing.as_mut() {
            board.update(state, audio_ms, now);
            if let Some(message) = board.notice.take() {
                set_wide_array(&mut self.tray.szInfoTitle, "Drawing unavailable");
                set_wide_array(&mut self.tray.szInfo, &message);
                self.tray.uFlags |= windows::Win32::UI::Shell::NIF_INFO;
                let _ = unsafe { Shell_NotifyIconW(NIM_MODIFY, &self.tray) };
                self.tray.uFlags &= !windows::Win32::UI::Shell::NIF_INFO;
            }
        }
        self.pill_state.drawing = state.ink;
    }

    fn lab_phase(&mut self, command: usize) {
        use pill_state::Phase;
        let now = self.clock.elapsed().as_millis() as u64;
        let phase = match command {
            1 => {
                self.status = AppStatus::Recording;
                self.paste_target = foreground_paste_target();
                self.recording_started = Some(Instant::now());
                self.meter = meter::Meter::default();
                Phase::Recording
            }
            2 => {
                self.status = AppStatus::Transcribing;
                Phase::Transcribing
            }
            3 => {
                self.status = AppStatus::Ready;
                self.last_target = self.paste_target;
                self.last_transcript = "Desktop lab result".into();
                Phase::Done(self.last_transcript.clone())
            }
            4 => {
                self.status = AppStatus::Ready;
                self.last_target = self.paste_target;
                self.last_transcript = "Desktop lab recovery".into();
                Phase::NotPasted { saved: true }
            }
            5 => Phase::Error {
                message: "Recording saved · retry from History".into(),
                rescue: true,
            },
            6 => Phase::Error {
                message: "Microphone unavailable".into(),
                rescue: false,
            },
            7 => Phase::Hidden,
            8 => {
                desktop::open(true, self.hwnd);
                return;
            }
            9 => {
                let _ = open_history();
                return;
            }
            11 => {
                self.pill_state.drawing = !self.pill_state.drawing;
                self.update_overlay();
                return;
            }
            12 => {
                self.last_target = self.paste_target;
                self.last_transcript = "Desktop lab media result".into();
                self.pill_state
                    .set(Phase::Done(self.last_transcript.clone()), now);
                self.pill_state.media_result = true;
                self.update_overlay();
                return;
            }
            10 => {
                self.pill_state.paused = !self.pill_state.paused;
                self.update_overlay();
                return;
            }
            _ => return,
        };
        self.pill_state.set(phase, now);
        self.update_tray();
        if self.status != AppStatus::Recording {
            self.draw_input = None;
        }
        self.update_overlay();
    }

    fn processing_request(&mut self, message: u32, value: usize) {
        use desktop::processing;
        if message == WM_CANCEL_SETUP {
            if processing::busy() {
                crate::model::SWITCH_CANCELLED.store(true, Ordering::Release);
                crate::formatting::CANCELLED.store(true, Ordering::Release);
            }
            return;
        }
        if self.status != AppStatus::Ready || self.capture_pending || processing::busy() {
            if message == WM_CHANGE_MODEL {
                processing::model_message(
                    "Stop recording or finish setup before changing models".into(),
                );
            } else {
                processing::formatting_status(
                    "Stop recording or finish setup before changing formatting".into(),
                );
            }
            return;
        }
        let Some(engine) = &self.engine else {
            return;
        };
        if message == WM_CHANGE_MODEL {
            if let Some(model) = crate::model::LocalModel::ALL.get(value).copied() {
                processing::model_status("Switching model…".into(), true);
                if engine
                    .commands
                    .send(EngineCommand::SwitchModel(model))
                    .is_err()
                {
                    processing::model_status("Dictation engine is unavailable".into(), false);
                }
            }
        } else if let Some(formatting) = processing::decode_formatting(value) {
            processing::formatting_status(if formatting.enabled {
                "Preparing S1-mini…".into()
            } else {
                "Raw dictation".into()
            });
            processing::formatting_busy(true);
            if engine
                .commands
                .send(EngineCommand::Formatting(formatting))
                .is_err()
            {
                processing::formatting_status("Dictation engine is unavailable".into());
                processing::formatting_busy(false);
            }
        }
    }
    fn toggle(&mut self) {
        self.recording_intent(RecordingIntent::Toggle);
    }

    fn recording_intent(&mut self, intent: RecordingIntent) {
        if self.desktop_lab {
            match intent {
                RecordingIntent::Start if self.status == AppStatus::Ready => self.lab_phase(1),
                RecordingIntent::Stop if self.status == AppStatus::Recording => self.lab_phase(2),
                RecordingIntent::Toggle => self.lab_phase(if self.status == AppStatus::Recording {
                    2
                } else {
                    1
                }),
                _ => {}
            }
            return;
        }
        if media_view::active() {
            return;
        }
        if self.pending_result.is_some() || self.staged.is_some() {
            return;
        }
        if self.capture_pending && intent != RecordingIntent::Stop {
            return;
        }
        if intent == RecordingIntent::Stop {
            if let Some(engine) = &self.engine {
                let _ = engine.commands.send(EngineCommand::Stop);
            }
            return;
        }
        if !matches!(
            self.status,
            AppStatus::Ready | AppStatus::Recording | AppStatus::Error(_)
        ) {
            return;
        }
        if intent == RecordingIntent::Start && self.status == AppStatus::Recording {
            return;
        }
        if matches!(self.status, AppStatus::Ready | AppStatus::Error(_)) && !self.capture_pending {
            self.paste_target = foreground_paste_target();
            self.capture_pending = true;
        }
        if let Some(engine) = &self.engine
            && engine.commands.send(intent.into()).is_err()
        {
            self.capture_pending = false;
            self.pill_state.set(
                pill_state::Phase::Error {
                    message: "Dictation engine is unavailable; restart Koett".into(),
                    rescue: false,
                },
                self.clock.elapsed().as_millis() as u64,
            );
            self.update_overlay();
        }
    }

    fn poll_updates(&mut self) {
        loop {
            let update = if !self.drawing.as_ref().is_some_and(drawing::Board::busy)
                && self.pending_result.is_some()
            {
                self.pending_result.take()
            } else {
                self.engine
                    .as_ref()
                    .and_then(|engine| engine.updates.try_recv().ok())
            };
            let Some(update) = update else { break };
            if matches!(update, EngineUpdate::TranscriptReady { .. })
                && self.drawing.as_ref().is_some_and(drawing::Board::busy)
            {
                self.pending_result = Some(update);
                continue;
            }
            match update {
                EngineUpdate::ActiveModel(model) => {
                    desktop::processing::model_status(format!("Ready: {model}"), false);
                    if let Ok(settings) = Settings::load() {
                        self.settings = settings;
                    }
                }
                EngineUpdate::Notice(message) => {
                    media_view::notice(&message);
                    desktop::processing::notice(&message);
                    if message == crate::runtime::LIMIT_WARNING {
                        let now = self.clock.elapsed().as_millis() as u64;
                        self.pill_state.show_notice(&message, now);
                    }
                    let message = message
                        .strip_prefix("@model-notice:")
                        .unwrap_or(&message)
                        .to_string();
                    eprintln!("Koett: {message}");
                }
                EngineUpdate::Status(status) => {
                    let media_outcome = media_view::finish(&status);
                    if let Some(cue) = sounds::for_change(&self.status, &status)
                        && let Ok(settings) = Settings::load()
                        && settings.recording_sounds
                    {
                        sounds::play(cue, settings.recording_sounds_volume);
                    }

                    let status = if status == AppStatus::Ready
                        && (self.staged.is_some() || self.pending_result.is_some())
                    {
                        AppStatus::Transcribing
                    } else {
                        status
                    };
                    if status != AppStatus::Recording {
                        self.draw_locked = false;
                        self.draw_lock_pending = false;
                        let _ = unsafe { UnregisterHotKey(Some(self.hwnd), 2) };
                        let _ = unsafe { UnregisterHotKey(Some(self.hwnd), 4) };
                        if status != AppStatus::Transcribing {
                            let _ = unsafe { UnregisterHotKey(Some(self.hwnd), 3) };
                        }
                        let audio_ms = self.levels.as_ref().map_or(0, AudioLevels::recorded_ms);
                        if let Some(board) = self.drawing.as_mut() {
                            board.stop(
                                audio_ms,
                                status != AppStatus::Transcribing && status != AppStatus::Ready,
                            );
                        }
                        self.draw_input = None;
                    }
                    self.capture_pending = false;
                    let now = self.clock.elapsed().as_millis() as u64;
                    match &status {
                        AppStatus::Recording => {
                            self.pill_media = false;
                            self.pill_state.set(pill_state::Phase::Recording, now)
                        }
                        AppStatus::Transcribing => {
                            if media_view::active() {
                                self.pill_media = true;
                                unsafe {
                                    let _ = UnregisterHotKey(Some(self.hwnd), 3);
                                    if RegisterHotKey(Some(self.hwnd), 3, MOD_NOREPEAT, 0x1b)
                                        .is_err()
                                    {
                                        media_view::notice(
                                            "Escape unavailable; use Cancel in the pill or media window",
                                        );
                                    }
                                }
                            }
                            self.pill_state.set(pill_state::Phase::Transcribing, now);
                            self.pill_state.media_result = self.pill_media;
                        }
                        AppStatus::Error(message) => self.pill_state.set(
                            pill_state::Phase::Error {
                                message: message.clone(),
                                rescue: true,
                            },
                            now,
                        ),
                        _ => {}
                    }
                    if let Some(outcome) = media_outcome {
                        self.pill_media = true;
                        let (phase, sticky) = match outcome {
                            Ok(completed) => (
                                pill_state::Phase::Done(
                                    completed.warning.unwrap_or(completed.text),
                                ),
                                !completed.saved,
                            ),
                            Err(message) => (
                                pill_state::Phase::Error {
                                    message,
                                    rescue: false,
                                },
                                false,
                            ),
                        };
                        self.pill_state.set(phase, now);
                        self.pill_state.sticky_result = sticky;
                        self.pill_state.media_result = true;
                    }
                    if status == AppStatus::Ready {
                        desktop::processing::model_finished();
                    }
                    self.status = status;
                    self.update_tray();
                    self.update_overlay();
                }
                EngineUpdate::RecordingStarted(levels) => {
                    self.meter = meter::Meter::default();
                    self.levels = Some(levels);
                    self.recording_started = Some(Instant::now());
                    self.drawing = None;
                    if let Ok(settings) = Settings::load() {
                        self.settings = settings;
                    }
                    if self.settings.draw_while_dictating {
                        match drawing::Board::new(
                            self.settings.pin_selected_text,
                            self.recording_started.unwrap_or_else(Instant::now),
                        )
                        .and_then(|board| {
                            draw_input::Monitor::start(self.hwnd, self.settings.draw_key)
                                .map(|input| (board, input))
                        }) {
                            Ok((board, input)) => {
                                self.drawing = Some(board);
                                self.draw_input = Some(input);
                            }
                            Err(error) => {
                                self.pill_state.set(
                                    pill_state::Phase::Error {
                                        message: error,
                                        rescue: false,
                                    },
                                    self.clock.elapsed().as_millis() as u64,
                                );
                            }
                        }
                    }
                    self.recording_hotkeys();
                    self.update_overlay();
                }
                EngineUpdate::ModelProgress(progress) => {
                    use crate::model::ModelProgress;
                    let text = match progress {
                        ModelProgress::Repairing => "Repairing model cache".to_string(),
                        ModelProgress::Downloading { received, total } => {
                            format!("Downloading model — {}%", received * 100 / total)
                        }
                        ModelProgress::Installing => "Installing model".to_string(),
                        ModelProgress::Ready => "Loading model".to_string(),
                    };
                    desktop::processing::model_status(text.clone(), true);
                    set_wide_array(&mut self.tray.szTip, &format!("Koett — {text}"));
                    let _ = unsafe { Shell_NotifyIconW(NIM_MODIFY, &self.tray) };
                }
                EngineUpdate::FormattingBusy(busy) => desktop::processing::formatting_busy(busy),
                EngineUpdate::TranscriptReady {
                    model,
                    transcript,
                    session,
                } => {
                    if self
                        .engine
                        .as_ref()
                        .is_some_and(|engine| engine.dictation_cancelled(session))
                    {
                        continue;
                    }
                    match Settings::load() {
                        Ok(saved) => self.settings.replacements = saved.replacements,
                        Err(error) => eprintln!(
                            "Koett: Settings unavailable; using last valid replacements: {error}"
                        ),
                    }
                    let (text, raw) = completed_words(
                        &transcript.text,
                        transcript.raw.as_deref(),
                        &self.settings.replacements,
                    );
                    self.deliver(&model, &text, raw.as_deref(), &transcript.words);
                }
            }
        }
    }

    fn tick_pill(&mut self) {
        let now = self.clock.elapsed().as_millis() as u64;
        if self.pill_state.opacity(now, pill::reduced_motion()) <= 0.0
            && !matches!(self.pill_state.phase, pill_state::Phase::Hidden)
        {
            self.pill_state.set(pill_state::Phase::Hidden, now);
        }
        if matches!(self.pill_state.phase, pill_state::Phase::Hidden)
            && self.status == AppStatus::Ready
            && !self.capture_pending
            && media_view::needs_copy()
        {
            self.pill_media = true;
            self.pill_state.set(
                pill_state::Phase::Done("Media not saved · Copy to keep".into()),
                now,
            );
            self.pill_state.media_result = true;
            self.pill_state.sticky_result = true;
        }
        let mut point = POINT::default();
        let mut bounds = RECT::default();
        unsafe {
            let _ = GetCursorPos(&mut point);
            let _ =
                windows::Win32::UI::WindowsAndMessaging::GetWindowRect(self.overlay, &mut bounds);
        }
        self.pill_state.hover(
            point.x >= bounds.left
                && point.x < bounds.right
                && point.y >= bounds.top
                && point.y < bounds.bottom,
            now,
        );
        self.update_overlay();
    }

    fn update_overlay(&self) {
        if let Some(buttons) = &self.pill_buttons {
            buttons.sync(self.overlay, &self.pill_state);
        }
        let opacity = self.pill_state.opacity(
            self.clock.elapsed().as_millis() as u64,
            pill::reduced_motion(),
        );
        unsafe {
            if opacity > 0.0 {
                if matches!(self.pill_state.phase, pill_state::Phase::Recording)
                    || self.recording_started.is_none()
                {
                    position_overlay(
                        self.overlay,
                        self.paste_target
                            .map(|target| target.hwnd)
                            .unwrap_or_else(|| GetForegroundWindow()),
                    );
                }
                let _ = windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow(
                    self.overlay,
                    self.pill_state.interactive(),
                );
                let _ = ShowWindow(self.overlay, SW_SHOWNOACTIVATE);
                let key = self.pill_state.paint_key();
                if self.last_pill_key.get() != Some(key)
                    || !pill::reduced_motion()
                    || self
                        .last_pill_redraw
                        .get()
                        .is_none_or(|t| t.elapsed() >= Duration::from_secs(1))
                {
                    self.last_pill_key.set(Some(key));
                    self.last_pill_redraw.set(Some(Instant::now()));
                    let _ = InvalidateRect(Some(self.overlay), None, false);
                }
            } else {
                let _ = ShowWindow(self.overlay, SW_HIDE);
            }
        }
    }

    fn pill_click(&mut self, x: i32, accessible: bool) {
        let now = self.clock.elapsed().as_millis() as u64;
        if (0..40).contains(&x) && self.status == AppStatus::Recording {
            self.recording_intent(RecordingIntent::Stop);
            return;
        }
        match &self.pill_state.phase {
            pill_state::Phase::Recording | pill_state::Phase::Transcribing
                if self.pill_state.hovered || accessible =>
            {
                if (40..100).contains(&x) {
                    self.cancel_current();
                } else if (100..175).contains(&x) && self.status == AppStatus::Recording {
                    if let Some(levels) = &self.levels {
                        self.pill_state.paused = !levels.paused();
                        levels.set_paused(self.pill_state.paused);
                    } else if self.desktop_lab {
                        self.pill_state.paused = !self.pill_state.paused;
                    }
                } else if (175..238).contains(&x) && !self.pill_media {
                    let _ = open_history();
                }
            }
            pill_state::Phase::Done(_) | pill_state::Phase::NotPasted { .. } => {
                if x >= 245 {
                    if (if self.pill_media {
                        media_view::copy(self.hwnd)
                    } else {
                        self.copy_last()
                    })
                    .is_ok()
                    {
                        if !self.pill_media {
                            self.pill_state
                                .set(pill_state::Phase::Done(self.last_transcript.clone()), now);
                        }
                        self.pill_state.copy(now);
                    }
                } else if x >= 190
                    && (self.pill_state.repeat_paste && !self.pill_media
                        || matches!(self.pill_state.phase, pill_state::Phase::NotPasted { .. }))
                {
                    self.repeat_paste();
                } else if matches!(self.pill_state.phase, pill_state::Phase::NotPasted { .. }) {
                    self.pill_state.set(pill_state::Phase::Hidden, now);
                }
            }
            pill_state::Phase::Error { .. } => self.pill_state.set(pill_state::Phase::Hidden, now),
            _ => {}
        }
        self.update_overlay();
    }

    fn show_shortcut_window(&mut self) {
        self.changing_media_shortcut = false;
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowTextW(
                self.shortcut_window,
                w!("Change dictation shortcut"),
            );
            let _ = ShowWindow(self.shortcut_window, SW_SHOWNORMAL);
            let _ = SetForegroundWindow(self.shortcut_window);
        }
    }

    fn change_media_shortcut(&mut self, shortcut: Shortcut) {
        if self.registered_shortcut == Some(shortcut)
            || Shortcut::from_str(&self.settings.draw_lock_shortcut).ok() == Some(shortcut)
        {
            show_fatal_error(
                "That shortcut is used for dictation or drawing. Choose another media shortcut.",
            );
            return;
        }

        if self.registered_media_shortcut == Some(shortcut) {
            unsafe {
                let _ = ShowWindow(self.shortcut_window, SW_HIDE);
            }
            return;
        }
        let old = self.registered_media_shortcut;
        unsafe {
            let _ = UnregisterHotKey(Some(self.hwnd), MEDIA_HOTKEY_ID);
        }
        let (modifiers, key) = windows_shortcut(shortcut);
        let registered = unsafe {
            RegisterHotKey(
                Some(self.hwnd),
                MEDIA_HOTKEY_ID,
                modifiers | MOD_NOREPEAT,
                key,
            )
        }
        .is_ok();
        let result = if registered {
            Settings::update(|settings| settings.media_shortcut = shortcut.to_string())
        } else {
            Err("That shortcut is already in use".into())
        };
        match result {
            Ok(()) => {
                self.registered_media_shortcut = Some(shortcut);
                self.settings.media_shortcut = shortcut.to_string();
                unsafe {
                    let _ = ShowWindow(self.shortcut_window, SW_HIDE);
                }
            }
            Err(error) => {
                unsafe {
                    let _ = UnregisterHotKey(Some(self.hwnd), MEDIA_HOTKEY_ID);
                }
                self.registered_media_shortcut = old.filter(|old| {
                    let (modifiers, key) = windows_shortcut(*old);
                    unsafe {
                        RegisterHotKey(
                            Some(self.hwnd),
                            MEDIA_HOTKEY_ID,
                            modifiers | MOD_NOREPEAT,
                            key,
                        )
                    }
                    .is_ok()
                });
                show_fatal_error(&format!(
                    "{error}. Choose another media shortcut; the previous shortcut is retained when available."
                ));
            }
        }
    }
    fn change_shortcut(&mut self, shortcut: Shortcut) {
        if self.registered_media_shortcut == Some(shortcut) {
            show_fatal_error("That shortcut is used for media. Choose another dictation shortcut.");
            return;
        }

        if self.registered_shortcut == Some(shortcut) {
            unsafe {
                let _ = ShowWindow(self.shortcut_window, SW_HIDE);
            }
            return;
        }

        let old = self.registered_shortcut;
        if old.is_some() {
            unsafe {
                let _ = UnregisterHotKey(Some(self.hwnd), HOTKEY_ID);
            }
        }
        let (modifiers, key) = windows_shortcut(shortcut);
        if unsafe { RegisterHotKey(Some(self.hwnd), HOTKEY_ID, modifiers | MOD_NOREPEAT, key) }
            .is_err()
        {
            self.registered_shortcut = old.filter(|old| {
                let (old_modifiers, old_key) = windows_shortcut(*old);
                unsafe {
                    RegisterHotKey(
                        Some(self.hwnd),
                        HOTKEY_ID,
                        old_modifiers | MOD_NOREPEAT,
                        old_key,
                    )
                }
                .is_ok()
            });
            if self.registered_shortcut.is_some() {
                show_fatal_error("That shortcut is already in use. The old shortcut still works.");
            } else {
                show_fatal_error("That shortcut is already in use. Choose another shortcut.");
            }
            self.update_tray();
            return;
        }

        let old_text = self.settings.shortcut.clone();
        self.settings.shortcut = shortcut.to_string();
        if let Err(error) = Settings::update(|current| {
            current.shortcut = self.settings.shortcut.clone();
        }) {
            unsafe {
                let _ = UnregisterHotKey(Some(self.hwnd), HOTKEY_ID);
            }
            self.registered_shortcut = old.filter(|old| {
                let (old_modifiers, old_key) = windows_shortcut(*old);
                unsafe {
                    RegisterHotKey(
                        Some(self.hwnd),
                        HOTKEY_ID,
                        old_modifiers | MOD_NOREPEAT,
                        old_key,
                    )
                }
                .is_ok()
            });
            self.settings.shortcut = old_text;
            show_fatal_error(&error);
            self.update_tray();
            return;
        }
        self.registered_shortcut = Some(shortcut);
        self.update_tray();
        unsafe {
            let _ = ShowWindow(self.shortcut_window, SW_HIDE);
        }
    }

    fn refresh_paste_preferences(&mut self) {
        if let Ok(settings) = Settings::load() {
            self.settings.paste_shortcut = settings.paste_shortcut;
            self.settings.image_paste_shortcuts = settings.image_paste_shortcuts;
        }
    }
    fn can_repeat_paste(&self) -> bool {
        !self.last_transcript.is_empty()
            && self.staged.is_none()
            && self.pending_result.is_none()
            && !self.capture_pending
            && matches!(self.status, AppStatus::Ready | AppStatus::Error(_))
    }
    fn post_image(
        &mut self,
        bundle: &Rc<clipboard::Bundle>,
        target: PasteTarget,
    ) -> Result<clipboard::Offer, String> {
        if foreground_paste_target() != Some(target) {
            return Err("Focus changed; drawing was not pasted".into());
        }
        let offer = bundle.image_first(self.hwnd)?;
        if let Err(error) = paste_offered(
            target,
            offer,
            paste_chord::image(&self.settings, target.pid),
        ) {
            self.clipboard_restore = Some(ClipboardRestore {
                bundle: Rc::clone(bundle),
                offer,
                target_pid: target.pid,
                started: Instant::now(),
                posted: false,
            });
            return Err(error);
        }
        Ok(offer)
    }
    fn repeat_paste(&mut self) {
        self.pill_media = false;
        if !self.can_repeat_paste() {
            return;
        }
        self.refresh_paste_preferences();
        let saved = match self.pill_state.phase {
            pill_state::Phase::NotPasted { saved } => saved,
            _ => true,
        };
        let result = self
            .last_target
            .ok_or_else(|| "No external paste target".to_string())
            .and_then(|target| {
                if foreground_paste_target() != Some(target) {
                    return Err("Focus changed; not pasted".into());
                }
                if let Some(bundle) = self.last_bundle.clone() {
                    let offer = self.post_image(&bundle, target)?;
                    self.staged = Some(StagedPaste {
                        offer,
                        target,
                        started: Instant::now(),
                        model: String::new(),
                        text: self.last_transcript.clone(),
                        raw: None,
                        saved: Some(saved),
                        text_shortcut: self.settings.paste_shortcut,
                    });
                } else {
                    let offer = clipboard::text_offer(self.hwnd, &self.last_transcript)?;
                    paste_offered(target, offer, self.settings.paste_shortcut)?;
                    self.pill_state.set(
                        pill_state::Phase::Done(self.last_transcript.clone()),
                        self.clock.elapsed().as_millis() as u64,
                    );
                }
                Ok(())
            });
        if let Err(error) = result {
            eprintln!("Koett: {error}");
            self.pill_state.set(
                pill_state::Phase::NotPasted { saved },
                self.clock.elapsed().as_millis() as u64,
            );
        }
    }
    fn cancel_current(&mut self) {
        if media_view::active() {
            if let Some(engine) = &self.engine {
                let _ = engine.commands.send(EngineCommand::CancelMedia);
            }
        } else {
            let parked = self.pending_result.is_some() || self.staged.is_some();
            self.discard_drawing();
            if parked {
                self.status = AppStatus::Ready;
                self.capture_pending = false;
                self.update_tray();
            }
            if let Some(engine) = &self.engine {
                engine.cancel_dictation();
                let _ = engine.commands.send(EngineCommand::Cancel);
            }
            self.pill_state.set(
                pill_state::Phase::Hidden,
                self.clock.elapsed().as_millis() as u64,
            );
            if self.desktop_lab {
                self.status = AppStatus::Ready;
            }
        }
    }
    fn discard_drawing(&mut self) {
        self.draw_locked = false;
        self.draw_lock_pending = false;
        let _ = unsafe { UnregisterHotKey(Some(self.hwnd), 2) };
        let _ = unsafe { UnregisterHotKey(Some(self.hwnd), 3) };
        let _ = unsafe { UnregisterHotKey(Some(self.hwnd), 4) };
        self.draw_input = None;
        self.pending_result = None;
        if let Some(mut board) = self.drawing.take() {
            board.stop(0, true);
        }
        if let Some(staged) = self.staged.take()
            && staged.saved.is_none()
        {
            self.record_delivery(
                &staged.model,
                &staged.text,
                staged.raw.as_deref(),
                Some("Drawing paste cancelled before text delivery".into()),
            );
        }
    }
    fn copy_last(&self) -> Result<(), String> {
        if let Some(bundle) = &self.last_bundle {
            bundle.copy(self.hwnd).map(|_| ())
        } else {
            copy_text(self.hwnd, &self.last_transcript)
        }
    }
    fn poll_clipboard_restore(&mut self) {
        let Some(restore) = self.clipboard_restore.as_ref() else {
            return;
        };
        let (offer, read, _) = clipboard::rendered(restore.offer, restore.target_pid);
        if !offer.unchanged() {
            self.clipboard_restore = None;
            return;
        }
        if restore.posted && !paste_timing::text_ready(restore.started, read, Instant::now()) {
            return;
        }
        let restore = self
            .clipboard_restore
            .take()
            .expect("clipboard restoration exists");
        if let Err(error) = restore.bundle.restore(offer)
            && offer.unchanged()
        {
            eprintln!("Koett: Clipboard recovery failed: {error}");
            self.notify_drawing(
                "Image and text could not be kept on the clipboard. Use tray Copy.",
            );
            if matches!(self.pill_state.phase, pill_state::Phase::Done(_))
                && self
                    .last_bundle
                    .as_ref()
                    .is_some_and(|bundle| Rc::ptr_eq(bundle, &restore.bundle))
            {
                self.pill_state.set(
                    pill_state::Phase::Error {
                        message: "Clipboard unavailable · use tray Copy".into(),
                        rescue: true,
                    },
                    self.clock.elapsed().as_millis() as u64,
                );
                self.update_overlay();
            }
        }
    }
    fn poll_staged(&mut self) {
        self.poll_clipboard_restore();
        let Some(staged) = self.staged.as_ref() else {
            return;
        };
        let (offer, read_at, image_requested) =
            clipboard::rendered(staged.offer, staged.target.pid);
        let ready = paste_timing::text_ready(staged.started, read_at, Instant::now());
        if !ready {
            return;
        }
        let staged = self.staged.take().expect("staged paste exists");
        self.clipboard_restore = self.last_bundle.as_ref().map(|bundle| ClipboardRestore {
            bundle: Rc::clone(bundle),
            offer,
            target_pid: staged.target.pid,
            started: Instant::now(),
            posted: false,
        });
        let result = if foreground_paste_target() != Some(staged.target) {
            Err("Focus changed; text was not pasted".into())
        } else {
            offer
                .replace_with_delayed_text(&staged.text)
                .and_then(|offer| {
                    let result = paste_offered(staged.target, offer, staged.text_shortcut);
                    if let Some(restore) = self.clipboard_restore.as_mut() {
                        restore.offer = offer;
                        restore.started = Instant::now();
                        restore.posted = result.is_ok();
                    }
                    result
                })
        };
        let image_omitted = result.is_ok() && !image_requested;
        if image_omitted {
            eprintln!(
                "Koett: Text pasted; the target did not request an image. Use Copy in an image-capable editor."
            );
        }
        if let Some(saved) = staged.saved {
            self.pill_state.set(
                pill_state::delivery_result(staged.text, result.is_ok(), saved, false),
                self.clock.elapsed().as_millis() as u64,
            );
            self.pill_state.repeat_paste = self.last_bundle.is_some();
            if let Err(error) = result {
                eprintln!("Koett: {error}");
            }
            self.update_overlay();
        } else {
            self.record_delivery(
                &staged.model,
                &staged.text,
                staged.raw.as_deref(),
                result.err(),
            );
        }
        if image_omitted && matches!(self.pill_state.phase, pill_state::Phase::Done(_)) {
            self.pill_state.set(
                pill_state::Phase::Done("Text pasted · image unavailable · Copy to keep".into()),
                self.clock.elapsed().as_millis() as u64,
            );
            self.pill_state.repeat_paste = false;
            self.update_overlay();
        }
    }

    fn deliver(
        &mut self,
        model: &str,
        text: &str,
        raw: Option<&str>,
        words: &[crate::annotation::TimedWord],
    ) {
        self.refresh_paste_preferences();
        let frames = self
            .drawing
            .as_mut()
            .map(drawing::Board::take_frames)
            .unwrap_or_default();
        if !frames.is_empty() {
            let raw = raw.or(Some(text));
            let anchors = frames
                .iter()
                .map(|frame| frame.start_ms.min(frame.end_ms) as f32 / 1000.0)
                .collect::<Vec<_>>();
            let summary = crate::annotation::keyframe_summary(text, words, &anchors);
            let summary = summary
                .lines()
                .zip(&frames)
                .map(|(bullet, frame)| {
                    let mut lines = vec![bullet.to_owned()];
                    if let Some(context) = &frame.context {
                        lines.extend(context.lines().into_iter().map(|line| format!("  {line}")));
                    }
                    if frame.pin
                        && (frame.context.is_none()
                            || frame
                                .context_status
                                .contains("Private mode: context omitted"))
                        && !frame.context_status.is_empty()
                    {
                        lines.push(format!("  {}", frame.context_status));
                    }
                    lines.join("\n")
                })
                .collect::<Vec<_>>()
                .join("\n");
            let result = contact_sheet::compose(&frames)
                .and_then(|image| clipboard::Bundle::new(&image, summary.clone()));
            match result {
                Ok(bundle) => {
                    let bundle = Rc::new(bundle);
                    self.last_bundle = Some(Rc::clone(&bundle));
                    self.last_transcript = summary.clone();
                    let result = self
                        .paste_target
                        .ok_or_else(|| "No external paste target".to_string())
                        .and_then(|target| {
                            self.post_image(&bundle, target)
                                .map(|offer| (offer, target))
                        });
                    match result {
                        Ok((offer, target)) => {
                            self.staged = Some(StagedPaste {
                                offer,
                                target,
                                started: Instant::now(),
                                model: model.into(),
                                text: summary,
                                raw: raw.map(str::to_owned),
                                saved: None,
                                text_shortcut: self.settings.paste_shortcut,
                            });
                        }
                        Err(error) => self.record_delivery(model, &summary, raw, Some(error)),
                    }
                }
                Err(error) => {
                    self.last_bundle = None;
                    self.record_delivery(model, text, raw, Some(error));
                }
            }
            return;
        }
        if text.trim().is_empty() {
            self.discard_drawing();
            self.status = AppStatus::Ready;
            self.capture_pending = false;
            self.update_tray();
            self.pill_state.set(
                pill_state::Phase::Hidden,
                self.clock.elapsed().as_millis() as u64,
            );
            self.update_overlay();
            return;
        }
        self.last_bundle = None;
        let delivery_error = clipboard::text_offer(self.hwnd, text)
            .and_then(|offer| {
                let target = self
                    .paste_target
                    .ok_or_else(|| "No external paste target. Transcript saved.".to_string())?;
                paste_offered(target, offer, self.settings.paste_shortcut)
            })
            .err();
        self.record_delivery(model, text, raw, delivery_error);
    }
    fn record_delivery(
        &mut self,
        model: &str,
        text: &str,
        raw: Option<&str>,
        delivery_error: Option<String>,
    ) {
        self.status = AppStatus::Ready;
        let _ = unsafe { UnregisterHotKey(Some(self.hwnd), 3) };
        self.last_transcript = text.to_string();
        self.last_target = self.paste_target;
        let failed = delivery_error.is_some();
        let storage = history::append_transcript_with_raw_resilient(model, text, raw);
        let saved = storage.is_ok();
        let history_error = match storage {
            Ok(Some(path)) => Some(format!(
                "The main transcript history failed. Koett saved this transcript to {}",
                path.display()
            )),
            Ok(None) => None,
            Err(error) => Some(error),
        };
        let history_failed = history_error.is_some();
        if history_error.is_some() || delivery_error.is_some() {
            let error = match (history_error, delivery_error) {
                (Some(history), Some(delivery)) => {
                    format!("{delivery}\nTranscript history also failed: {history}")
                }
                (Some(history), None) => format!("Transcript history failed: {history}"),
                (None, Some(delivery)) => delivery,
                (None, None) => unreachable!(),
            };
            eprintln!("Koett: {error}");
        }
        let phase = pill_state::delivery_result(text.to_string(), !failed, saved, history_failed);
        self.pill_state
            .set(phase, self.clock.elapsed().as_millis() as u64);
        self.pill_state.repeat_paste = self.last_bundle.is_some();
        self.update_overlay();
    }

    fn add_tray(&mut self) -> Result<(), String> {
        let mut tray = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: 1,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_GUID | NIF_SHOWTIP,
            uCallbackMessage: TRAY_MESSAGE,
            hIcon: unsafe { LoadIconW(None, IDI_APPLICATION) }
                .map_err(|error| format!("could not load the Koett tray icon: {error}"))?,
            guidItem: TRAY_GUID,
            ..Default::default()
        };
        set_wide_array(&mut tray.szTip, "Koett");
        if !unsafe { Shell_NotifyIconW(NIM_ADD, &tray) }.as_bool() {
            return Err("could not add the Koett tray icon".to_string());
        }
        tray.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        let _ = unsafe { Shell_NotifyIconW(NIM_SETVERSION, &tray) };
        self.tray = tray;
        Ok(())
    }

    fn remove_tray(&self) {
        if !self.tray.hWnd.is_invalid() {
            let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &self.tray) };
        }
    }

    fn update_tray(&mut self) {
        let status = if self.registered_shortcut.is_some() {
            status_text(&self.status).to_string()
        } else {
            "Shortcut unavailable — click Koett to change it".to_string()
        };
        set_wide_array(&mut self.tray.szTip, &format!("Koett — {status}"));
        let _ = unsafe { Shell_NotifyIconW(NIM_MODIFY, &self.tray) };
    }

    fn show_menu(&mut self, anchor: Option<POINT>) {
        let menu_target = foreground::menu_target();
        unsafe {
            let Ok(menu) = CreatePopupMenu() else { return };
            let status = wide(&format!("Status: {}", status_text(&self.status)));
            let _ = AppendMenuW(
                menu,
                MF_STRING | MF_DISABLED,
                0,
                PCWSTR::from_raw(status.as_ptr()),
            );
            let shortcut = wide(&format!("Shortcut: {}", self.settings.shortcut));
            let _ = AppendMenuW(
                menu,
                MF_STRING | MF_DISABLED,
                0,
                PCWSTR::from_raw(shortcut.as_ptr()),
            );
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                MENU_CHANGE_SHORTCUT,
                w!("Change shortcut…"),
            );
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
            let _ = AppendMenuW(menu, MF_STRING, MENU_OPEN_HISTORY, w!("History"));
            let _ = AppendMenuW(menu, MF_STRING, MENU_SETTINGS, w!("Settings"));
            let _ = AppendMenuW(
                menu,
                MF_STRING
                    | if self.settings.draw_while_dictating {
                        MF_CHECKED
                    } else {
                        Default::default()
                    }
                    | if self.status == AppStatus::Recording {
                        MF_DISABLED
                    } else {
                        Default::default()
                    },
                MENU_DRAW,
                w!("Draw while dictating"),
            );
            let _ = AppendMenuW(
                menu,
                MF_STRING
                    | if self.status != AppStatus::Recording
                        || !self.drawing.as_ref().is_some_and(drawing::Board::can_undo)
                    {
                        MF_DISABLED
                    } else {
                        Default::default()
                    },
                MENU_UNDO_DRAW,
                w!("Undo drawing"),
            );
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                MENU_WORDS,
                w!("Dictionary and replacements…"),
            );
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                MENU_RECORD,
                if self.status == AppStatus::Recording {
                    w!("Stop")
                } else {
                    w!("Record")
                },
            );
            let _ = AppendMenuW(
                menu,
                if self.last_transcript.is_empty() {
                    MF_STRING | MF_DISABLED
                } else {
                    MF_STRING
                },
                MENU_COPY_LAST,
                w!("Copy last transcript"),
            );
            let _ = AppendMenuW(
                menu,
                if !self.can_repeat_paste() {
                    MF_STRING | MF_DISABLED
                } else {
                    MF_STRING
                },
                MENU_PASTE_LAST,
                w!("Paste last transcript"),
            );
            let _ = AppendMenuW(
                menu,
                if self.status == AppStatus::Recording {
                    MF_STRING
                } else {
                    MF_STRING | MF_DISABLED
                },
                MENU_PAUSE,
                if self.pill_state.paused {
                    w!("Resume")
                } else {
                    w!("Pause")
                },
            );
            let _ = AppendMenuW(
                menu,
                if matches!(self.status, AppStatus::Recording | AppStatus::Transcribing) {
                    MF_STRING
                } else {
                    MF_STRING | MF_DISABLED
                },
                MENU_CANCEL,
                w!("Cancel"),
            );
            let startup_flags = if start_at_login_registered().unwrap_or(false) {
                MF_STRING | MF_CHECKED
            } else {
                MF_STRING
            };
            let _ = AppendMenuW(
                menu,
                startup_flags,
                MENU_START_AT_LOGIN,
                w!("Start at login"),
            );
            let _ = AppendMenuW(menu, MF_STRING, MENU_MEDIA, w!("Transcribe media…"));
            let _ = AppendMenuW(menu, MF_STRING, MENU_RECOVERY, w!("Transcript recovery…"));
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
            let _ = AppendMenuW(menu, MF_STRING, MENU_QUIT, w!("Quit Koett"));

            let mut point = POINT::default();
            let has_point = if let Some(anchor) = anchor {
                point = anchor;
                true
            } else {
                GetCursorPos(&mut point).is_ok()
            };
            if has_point {
                let _ = SetForegroundWindow(self.hwnd);
                let selected = TrackPopupMenu(
                    menu,
                    TPM_RETURNCMD | TPM_RIGHTBUTTON,
                    point.x,
                    point.y,
                    None,
                    self.hwnd,
                    None,
                )
                .0 as usize;
                if selected == 0 {
                    let current = GetForegroundWindow();
                    if current == self.hwnd || foreground::is_tray(current) {
                        let _ = Shell_NotifyIconW(NIM_SETFOCUS, &self.tray);
                    }
                } else if !matches!(
                    selected,
                    MENU_CHANGE_SHORTCUT | MENU_SETTINGS | MENU_WORDS | MENU_OPEN_HISTORY
                ) && let Some(target) = menu_target
                    && external_paste_target(target.hwnd) == Some(target)
                {
                    let current = GetForegroundWindow();
                    if current == self.hwnd || foreground::is_tray(current) {
                        let _ = SetForegroundWindow(target.hwnd);
                    }
                }
                self.handle_menu(selected);
            }
            let _ = DestroyMenu(menu);
        }
    }

    fn dictation_in_flight(&self) -> bool {
        self.capture_pending
            || self.pending_result.is_some()
            || self
                .staged
                .as_ref()
                .is_some_and(|staged| staged.saved.is_none())
            || matches!(self.status, AppStatus::Recording | AppStatus::Transcribing)
    }
    fn request_quit(&self) {
        if self.dictation_in_flight() {
            unsafe {
                let _ = MessageBoxW(
                    Some(self.hwnd),
                    w!("Finish or cancel dictation before quitting."),
                    w!("Koett"),
                    windows::Win32::UI::WindowsAndMessaging::MB_OK
                        | windows::Win32::UI::WindowsAndMessaging::MB_ICONWARNING,
                );
            }
            return;
        }
        let foreground = unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetAncestor(
                GetForegroundWindow(),
                windows::Win32::UI::WindowsAndMessaging::GA_ROOT,
            )
        };
        let mut pid = 0;
        unsafe {
            GetWindowThreadProcessId(foreground, Some(&mut pid));
        }
        let owner = if pid == std::process::id() && unsafe { IsWindowVisible(foreground) }.as_bool()
        {
            foreground
        } else {
            self.hwnd
        };
        if (media_view::needs_copy() || history::recovery::pending_count() != 0)
            && unsafe {
                MessageBoxW(
                    Some(owner),
                    w!("Some transcripts are not saved. Quit without saving or copying them?"),
                    w!("Koett"),
                    windows::Win32::UI::WindowsAndMessaging::MB_YESNO
                        | windows::Win32::UI::WindowsAndMessaging::MB_ICONWARNING
                        | windows::Win32::UI::WindowsAndMessaging::MB_DEFBUTTON2
                        | windows::Win32::UI::WindowsAndMessaging::MB_SETFOREGROUND,
                )
            } != windows::Win32::UI::WindowsAndMessaging::IDYES
        {
            return;
        }
        unsafe {
            PostQuitMessage(0);
        }
    }
    fn handle_menu(&mut self, command: usize) {
        match command {
            MENU_CHANGE_SHORTCUT => self.show_shortcut_window(),
            MENU_CHANGE_MEDIA_SHORTCUT => {
                self.changing_media_shortcut = true;
                unsafe {
                    let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowTextW(
                        self.shortcut_window,
                        w!("Change media shortcut"),
                    );
                    let _ = ShowWindow(self.shortcut_window, SW_SHOWNORMAL);
                    let _ = SetForegroundWindow(self.shortcut_window);
                }
            }
            MENU_MEDIA => {
                if let Some(engine) = &self.engine {
                    media_view::open(engine.commands.clone());
                }
            }
            MENU_RECOVERY => recovery::open(),
            MENU_SETTINGS => desktop::open(true, self.hwnd),
            MENU_WORDS => desktop::open_words(self.hwnd),
            MENU_DRAW => {
                if self.status != AppStatus::Recording {
                    if let Err(error) = Settings::update(|settings| {
                        settings.draw_while_dictating = !settings.draw_while_dictating
                    }) {
                        show_fatal_error(&error);
                    }
                    if let Ok(settings) = Settings::load() {
                        self.settings = settings;
                    }
                }
            }
            MENU_UNDO_DRAW => {
                if self.status == AppStatus::Recording
                    && let Some(board) = self.drawing.as_mut()
                {
                    board.undo();
                }
            }
            MENU_COPY_LAST => {
                if let Err(e) = self.copy_last() {
                    show_fatal_error(&e);
                }
            }
            MENU_PASTE_LAST => self.repeat_paste(),
            MENU_RECORD => self.toggle(),
            MENU_PAUSE => {
                if let Some(levels) = &self.levels {
                    self.pill_state.paused = !levels.paused();
                    levels.set_paused(self.pill_state.paused);
                }
            }
            MENU_CANCEL => {
                self.cancel_current();
                self.update_overlay();
            }
            MENU_OPEN_HISTORY => {
                if let Err(error) = open_history() {
                    show_fatal_error(&error);
                }
            }
            MENU_START_AT_LOGIN => {
                let result = (|| {
                    let mut edit = Settings::edit()?;
                    let previous = start_at_login_registered()?;
                    let enabled = !previous;
                    set_start_at_login(enabled)?;
                    edit.settings.start_at_login = enabled;
                    if let Err(error) = edit.commit() {
                        let _ = set_start_at_login(previous);
                        return Err(error);
                    }
                    self.settings.start_at_login = enabled;
                    Ok(())
                })();
                if let Err(error) = result {
                    show_fatal_error(&error);
                }
            }
            MENU_QUIT => self.request_quit(),
            _ => {}
        }
    }
}

fn tray_anchor(value: usize) -> POINT {
    POINT {
        x: (value as u16 as i16) as i32,
        y: ((value >> 16) as u16 as i16) as i32,
    }
}

fn shortcut_from_virtual_key(virtual_key: u32) -> Option<Shortcut> {
    if [
        u32::from(VK_CONTROL.0),
        u32::from(VK_MENU.0),
        u32::from(VK_SHIFT.0),
        u32::from(VK_LWIN.0),
        u32::from(VK_RWIN.0),
    ]
    .contains(&virtual_key)
    {
        return None;
    }
    let key = match virtual_key {
        0x20 => Key::Space,
        0x41..=0x5a => Key::Letter(char::from_u32(virtual_key)?),
        0x30..=0x39 => Key::Number((virtual_key - 0x30) as u8),
        0x70..=0x87 => Key::Function((virtual_key - 0x70 + 1) as u8),
        _ => return None,
    };
    let pressed = |key: VIRTUAL_KEY| unsafe { GetKeyState(i32::from(key.0)) } < 0;
    Shortcut::new(
        Modifiers {
            control: pressed(VK_CONTROL),
            alt: pressed(VK_MENU),
            shift: pressed(VK_SHIFT),
            super_key: pressed(VK_LWIN) || pressed(VK_RWIN),
        },
        key,
    )
    .ok()
}

unsafe fn paint_shortcut_window(hwnd: HWND) {
    let mut paint = PAINTSTRUCT::default();
    let dc = unsafe { BeginPaint(hwnd, &mut paint) };
    let white = unsafe { CreateSolidBrush(COLORREF(0x00ffffff)) };
    let mut bounds = RECT::default();
    if unsafe { GetClientRect(hwnd, &mut bounds) }.is_ok() {
        unsafe { FillRect(dc, &bounds, white) };
        let mut text = "Press a new shortcut.\nUse at least one modifier."
            .encode_utf16()
            .collect::<Vec<_>>();
        unsafe {
            SetBkMode(dc, TRANSPARENT);
            SetTextColor(dc, COLORREF(0x00211e1d));
            DrawTextW(dc, &mut text, &mut bounds, DT_CENTER | DT_VCENTER);
        }
    }
    unsafe {
        let _ = DeleteObject(HGDIOBJ(white.0));
        let _ = EndPaint(hwnd, &paint);
    }
}

unsafe fn position_overlay(overlay: HWND, target: HWND) {
    let monitor = unsafe { MonitorFromWindow(target, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        let mut dpi = 96;
        let mut dpi_y = 96;
        let _ = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut dpi_y) };
        let width = (300 * dpi / 96) as i32;
        let height = (48 * dpi / 96) as i32;
        let radius = (46 * dpi / 96) as i32;
        let region = unsafe { CreateRoundRectRgn(0, 0, width + 1, height + 1, radius, radius) };
        if unsafe { SetWindowRgn(overlay, Some(region), true) } == 0 {
            let _ = unsafe { DeleteObject(HGDIOBJ(region.0)) };
        }
        let x = info.rcWork.left + (info.rcWork.right - info.rcWork.left - width) / 2;
        let y = info.rcWork.bottom - height - 28;
        let _ = unsafe {
            SetWindowPos(
                overlay,
                Some(HWND_TOPMOST),
                x,
                y,
                width,
                height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
        };
    }
}

unsafe fn paint_overlay(hwnd: HWND, app: &WindowsApp) {
    let mut paint = PAINTSTRUCT::default();
    let _dc = unsafe { BeginPaint(hwnd, &mut paint) };
    let mut bounds = RECT::default();
    if unsafe { GetClientRect(hwnd, &mut bounds) }.is_ok() {
        let mut renderer = app.pill_renderer.borrow_mut();
        if renderer.is_none() {
            match pill::Renderer::new(hwnd, (bounds.right as u32, bounds.bottom as u32)) {
                Ok(created) => *renderer = Some(created),
                Err(error) => report_render_error(app, "initialize", &error),
            }
        }
        if let Some(current) = renderer.as_mut() {
            let elapsed = app
                .recording_started
                .map(|s| s.elapsed().as_secs())
                .unwrap_or(0);
            let events = app
                .drawing
                .as_ref()
                .map(drawing::Board::events)
                .unwrap_or_default();
            let bands = events
                .iter()
                .filter(|_| !app.pill_media)
                .filter_map(|event| {
                    app.meter
                        .band(event.start_ms, event.end_ms)
                        .map(|(left, right)| (event.index, left, right))
                })
                .collect::<Vec<_>>();
            if let Err(error) = current.paint(
                hwnd,
                bounds,
                pill::TimelinePaint {
                    bars: &if app.pill_media {
                        Vec::new()
                    } else {
                        app.meter.positions()
                    },
                    bands: &bands,
                },
                elapsed,
                &app.pill_state,
                app.clock.elapsed().as_millis() as u64,
            ) {
                report_render_error(app, "paint", &error);
                *renderer = None;
            }
        }
    }
    unsafe {
        let _ = EndPaint(hwnd, &paint);
    }
}

fn report_render_error(app: &WindowsApp, stage: &str, error: &windows::core::Error) {
    if app
        .last_render_error
        .get()
        .is_some_and(|t| t.elapsed() < Duration::from_secs(5))
    {
        return;
    }
    app.last_render_error.set(Some(Instant::now()));
    let message = format!("Pill {stage}: {error} ({:?})", error.code());
    eprintln!("{message}");
    if let Ok(path) =
        crate::paths::settings_file().map(|p| p.with_file_name("Windows Renderer Error.txt"))
    {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, message);
    }
}

fn status_text(status: &AppStatus) -> &str {
    match status {
        AppStatus::Starting => "Starting",
        AppStatus::Ready => "Ready",
        AppStatus::Recording => "Recording",
        AppStatus::Transcribing => "Transcribing",
        AppStatus::Error(_) => "Needs attention",
    }
}

fn set_wide_array<const N: usize>(output: &mut [u16; N], value: &str) {
    output.fill(0);
    for (destination, source) in output.iter_mut().zip(value.encode_utf16()) {
        *destination = source;
    }
}

fn open_history() -> Result<(), String> {
    desktop::open(false, HWND::default());
    Ok(())
}

fn start_at_login_registered() -> Result<bool, String> {
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            w!("Koett"),
            RRF_RT_REG_SZ,
            None,
            None,
            None,
        )
    };
    match result {
        ERROR_SUCCESS => Ok(true),
        ERROR_FILE_NOT_FOUND => Ok(false),
        _ => Err(format!(
            "could not read the Windows startup setting: {result:?}"
        )),
    }
}

fn set_start_at_login(enabled: bool) -> Result<(), String> {
    let mut key = HKEY::default();
    let result = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            None,
        )
    };
    if result != ERROR_SUCCESS {
        return Err(format!(
            "could not open the Windows startup setting: {result:?}"
        ));
    }

    let operation = if enabled {
        let executable = std::env::current_exe()
            .map_err(|error| format!("could not find the Koett executable: {error}"))?;
        let value = wide(&format!("\"{}\"", executable.display()));
        let bytes = unsafe {
            std::slice::from_raw_parts(value.as_ptr().cast::<u8>(), std::mem::size_of_val(&*value))
        };
        unsafe { RegSetValueExW(key, w!("Koett"), None, REG_SZ, Some(bytes)) }
    } else {
        let result = unsafe { RegDeleteValueW(key, w!("Koett")) };
        if result == ERROR_FILE_NOT_FOUND {
            ERROR_SUCCESS
        } else {
            result
        }
    };
    unsafe {
        let _ = RegCloseKey(key);
    }
    if operation == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(format!(
            "could not update the Windows startup setting: {operation:?}"
        ))
    }
}

fn windows_shortcut(shortcut: Shortcut) -> (HOT_KEY_MODIFIERS, u32) {
    let mut modifiers = HOT_KEY_MODIFIERS::default();
    if shortcut.modifiers.control {
        modifiers |= MOD_CONTROL;
    }
    if shortcut.modifiers.alt {
        modifiers |= MOD_ALT;
    }
    if shortcut.modifiers.shift {
        modifiers |= MOD_SHIFT;
    }
    if shortcut.modifiers.super_key {
        modifiers |= MOD_WIN;
    }
    let key = match shortcut.key {
        Key::Space => 0x20,
        Key::Letter(letter) => letter as u32,
        Key::Number(number) => u32::from(b'0' + number),
        Key::Function(number) => 0x70 + u32::from(number - 1),
    };
    (modifiers, key)
}

fn copy_text(owner: HWND, text: &str) -> Result<(), String> {
    clipboard::copy_text(owner, text)
}

fn shortcut_is_down(shortcut: Shortcut) -> bool {
    let (_, key) = windows_shortcut(shortcut);
    let mut keys = vec![VIRTUAL_KEY(key as u16)];
    let modifiers = shortcut.modifiers;
    for (required, key) in [
        (modifiers.control, VK_CONTROL),
        (modifiers.alt, VK_MENU),
        (modifiers.shift, VK_SHIFT),
    ] {
        if required {
            keys.push(key);
        }
    }
    keys.into_iter()
        .all(|key| unsafe { GetAsyncKeyState(i32::from(key.0)) } < 0)
        && (!modifiers.super_key
            || [VK_LWIN, VK_RWIN]
                .into_iter()
                .any(|key| unsafe { GetAsyncKeyState(i32::from(key.0)) } < 0))
}

fn external_paste_target(hwnd: HWND) -> Option<PasteTarget> {
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    (!hwnd.is_invalid()
        && pid != 0
        && pid != std::process::id()
        && unsafe { IsWindowVisible(hwnd) }.as_bool()
        && unsafe { windows::Win32::UI::WindowsAndMessaging::GetShellWindow() } != hwnd
        && !foreground::is_tray(hwnd))
    .then_some(PasteTarget { hwnd, pid })
}

fn foreground_paste_target() -> Option<PasteTarget> {
    external_paste_target(unsafe { GetForegroundWindow() })
}

fn paste_offered(
    expected: PasteTarget,
    offer: clipboard::Offer,
    shortcut: crate::settings::PasteShortcut,
) -> Result<(), String> {
    paste_security::check(expected.pid)?;
    let modifiers_are_down = || {
        [VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN]
            .into_iter()
            .any(|key| unsafe { GetAsyncKeyState(i32::from(key.0)) } < 0)
    };
    let keyboard = |key: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                dwFlags: flags,
                ..Default::default()
            },
        },
    };
    let mut inputs = vec![keyboard(VK_CONTROL, Default::default())];
    if shortcut == crate::settings::PasteShortcut::CtrlShiftV {
        inputs.push(keyboard(VK_SHIFT, Default::default()));
    }
    inputs.push(keyboard(VIRTUAL_KEY(b'V' as u16), Default::default()));
    inputs.push(keyboard(VIRTUAL_KEY(b'V' as u16), KEYEVENTF_KEYUP));
    if shortcut == crate::settings::PasteShortcut::CtrlShiftV {
        inputs.push(keyboard(VK_SHIFT, KEYEVENTF_KEYUP));
    }
    inputs.push(keyboard(VK_CONTROL, KEYEVENTF_KEYUP));
    paste_guard::paste_when_ready(
        expected,
        modifiers_are_down,
        || offer.verify(),
        foreground_paste_target,
        || {
            let inserted = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
            if inserted == inputs.len() as u32 {
                Ok(())
            } else {
                Err(
                    "Windows blocked the paste; the transcript is still on the clipboard"
                        .to_string(),
                )
            }
        },
    )
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}

// Runtime restores dictionary spelling around optional cleanup. Literal rules run last.
fn completed_words(
    text: &str,
    raw: Option<&str>,
    rules: &[crate::replacements::Replacement],
) -> (String, Option<String>) {
    let final_text = crate::replacements::apply(text, rules);
    let raw = raw.or_else(|| (final_text != text).then_some(text));
    (final_text, raw.map(str::to_owned))
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use windows::Win32::UI::Input::KeyboardAndMouse::{MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT};

    use super::{Shortcut, tray_anchor, windows_shortcut};

    fn review_app() -> super::WindowsApp {
        use super::*;
        let engine = None;
        let desktop_lab = false;
        let settings = Settings::default();
        let restored_last = "Previous transcript".into();
        WindowsApp {
            hwnd: HWND::default(),
            engine,
            desktop_lab,
            settings,
            status: if desktop_lab {
                AppStatus::Ready
            } else {
                AppStatus::Starting
            },
            paste_target: None,
            last_target: None,
            overlay: HWND::default(),
            pill_renderer: RefCell::new(None),
            last_render_error: Cell::new(None),
            pill_buttons: None,
            meter: meter::Meter::default(),
            pill_state: pill_state::State::default(),
            clock: Instant::now(),
            last_transcript: restored_last,
            pill_media: false,
            last_pill_redraw: Cell::new(None),
            last_pill_key: Cell::new(None),
            shortcut_window: HWND::default(),
            registered_shortcut: None,
            registered_media_shortcut: None,
            changing_media_shortcut: false,
            shortcut_latch: ShortcutLatch::default(),
            levels: None,
            recording_started: None,
            draw_input: None,
            drawing: None,
            draw_locked: false,
            draw_lock_pending: false,
            pill_pressed: false,
            pending_result: None,
            staged: None,
            last_bundle: None,
            clipboard_restore: None,
            draw_pointer: POINT::default(),
            capture_lab: None,
            capture_pending: false,
            tray: NOTIFYICONDATAW::default(),
            taskbar_created: 0,
        }
    }
    #[test]
    fn cancel_parked_drawing_releases_busy_and_quit_guards() {
        use super::*;
        let mut app = review_app();
        app.status = AppStatus::Transcribing;
        app.pending_result = Some(EngineUpdate::TranscriptReady {
            model: "fixture".into(),
            session: 1,
            transcript: crate::transcription::Transcript {
                raw: None,
                text: "spoken words".into(),
                words: vec![],
                audio_duration: Duration::from_secs(1),
                transcription: Duration::ZERO,
            },
        });
        assert!(app.dictation_in_flight());
        app.cancel_current();
        assert_eq!(app.status, AppStatus::Ready);
        assert!(app.pending_result.is_none());
        assert!(!app.dictation_in_flight());
        assert!(app.can_repeat_paste());
    }
    #[test]
    fn empty_parked_result_returns_ready_without_replacing_previous_text() {
        use super::*;
        let mut app = review_app();
        app.status = AppStatus::Transcribing;
        app.draw_locked = true;
        app.draw_lock_pending = true;
        // Use a private, otherwise unused chord rather than capturing Escape
        // from another application while this regression runs.
        unsafe { RegisterHotKey(None, 3, MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT, 0x7a) }.unwrap();
        app.deliver("fixture", "", None, &[]);
        assert!(
            unsafe { UnregisterHotKey(None, 3) }.is_err(),
            "empty completion must release its cancel hotkey"
        );
        assert!(!app.draw_locked);
        assert!(!app.draw_lock_pending);
        assert_eq!(app.status, AppStatus::Ready);
        assert!(!app.dictation_in_flight());
        assert_eq!(app.last_transcript, "Previous transcript");
    }
    #[test]
    fn quit_guard_rejects_recording_transcribing_and_capture_open() {
        use super::*;
        let mut app = review_app();
        for status in [AppStatus::Recording, AppStatus::Transcribing] {
            app.status = status;
            assert!(app.dictation_in_flight());
        }
        app.status = AppStatus::Ready;
        app.capture_pending = true;
        assert!(app.dictation_in_flight());
        app.capture_pending = false;
        assert!(!app.dictation_in_flight());
    }
    #[test]
    fn replacements_keep_original_asr_and_run_once_after_cleanup() {
        use crate::replacements::Replacement;
        let rules = [
            Replacement {
                phrase: "old portrait".into(),
                replacement: "new portrait".into(),
            },
            Replacement {
                phrase: "new portrait".into(),
                replacement: "wrong cascade".into(),
            },
        ];
        assert_eq!(
            super::completed_words("old portrait", None, &rules),
            ("new portrait".into(), Some("old portrait".into()))
        );
        assert_eq!(
            super::completed_words("old portrait", Some("Original ASR"), &rules),
            ("new portrait".into(), Some("Original ASR".into()))
        );
        assert_eq!(
            super::completed_words("unchanged", None, &rules),
            ("unchanged".into(), None)
        );
        assert_eq!(
            super::completed_words(
                "remove",
                None,
                &[Replacement {
                    phrase: "remove".into(),
                    replacement: String::new()
                }]
            ),
            (String::new(), Some("remove".into()))
        );
    }

    #[test]
    fn every_window_in_this_process_is_ineligible_for_paste() {
        use super::*;
        let hwnd = unsafe {
            CreateWindowExW(
                Default::default(),
                w!("STATIC"),
                w!("History search target"),
                Default::default(),
                0,
                0,
                100,
                100,
                None,
                None,
                None,
                None,
            )
        }
        .unwrap();
        assert!(external_paste_target(hwnd).is_none());
        assert!(external_paste_target(HWND::default()).is_none());
        unsafe { windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd) }.unwrap();
    }

    #[test]
    fn default_shortcut_maps_to_win32() {
        let (modifiers, key) = windows_shortcut(Shortcut::from_str("Ctrl+Shift+Space").unwrap());
        assert_eq!(
            (modifiers | MOD_NOREPEAT).0,
            0x4000 | MOD_CONTROL.0 | MOD_SHIFT.0
        );
        assert_eq!(key, 0x20);
    }

    #[test]
    fn tray_anchor_reads_signed_screen_coordinates() {
        let packed = u32::from(u16::from_ne_bytes((-20_i16).to_ne_bytes())) as usize
            | ((u32::from(u16::from_ne_bytes(300_i16.to_ne_bytes())) as usize) << 16);
        let point = tray_anchor(packed);
        assert_eq!((point.x, point.y), (-20, 300));
    }
}
