mod desktop;
mod meter;
mod native_theme;
mod pill;
mod pill_buttons;
mod pill_state;

use std::cell::{Cell, RefCell};
use std::mem::size_of;
use std::ptr;
use std::str::FromStr;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{
    COLORREF, CloseHandle, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, GetLastError,
    GlobalFree, HANDLE, HWND, POINT, RECT,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateRoundRectRgn, CreateSolidBrush, DT_CENTER, DT_VCENTER, DeleteObject,
    DrawTextW, EndPaint, FillRect, GetMonitorInfoW, HGDIOBJ, InvalidateRect,
    MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow, PAINTSTRUCT, SetBkMode, SetTextColor,
    SetWindowRgn, TRANSPARENT,
};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_UNICODETEXT;
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
    HWND_TOPMOST, IDI_APPLICATION, KillTimer, LoadIconW, MA_NOACTIVATE, MF_CHECKED, MF_DISABLED,
    MF_SEPARATOR, MF_STRING, MSG, MessageBoxW, PostQuitMessage, RegisterClassExW,
    RegisterWindowMessageW, SW_HIDE, SW_SHOWNOACTIVATE, SW_SHOWNORMAL, SWP_NOACTIVATE,
    SWP_SHOWWINDOW, SetForegroundWindow, SetTimer, SetWindowPos, ShowWindow, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE,
    WM_CONTEXTMENU, WM_DESTROY, WM_HOTKEY, WM_KEYDOWN, WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_PAINT,
    WM_RBUTTONUP, WM_SYSKEYDOWN, WM_TIMER, WNDCLASSEXW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_OVERLAPPEDWINDOW, WS_POPUP,
};
use windows::core::{GUID, PCWSTR, w};

use crate::audio::AudioLevels;
use crate::history;
use crate::runtime::{self, EngineCommand, EngineHandle, EngineUpdate};
use crate::settings::Settings;
use crate::shortcut::{Key, Modifiers, Shortcut};
use crate::state::AppStatus;

const HOTKEY_ID: i32 = 1;
const POLL_TIMER_ID: usize = 1;
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
const TRAY_GUID: GUID = GUID::from_u128(0xc65ddae4_119c_486e_8bed_e30a24f2fb1f);

// All Koett windows run on the thread that owns this RefCell. Win32 can re-enter
// a window procedure while a menu or dialog is open, so callbacks use
// try_borrow instead of creating overlapping references from a raw pointer.
static APP: AtomicPtr<RefCell<WindowsApp>> = AtomicPtr::new(ptr::null_mut());

struct WindowsApp {
    hwnd: HWND,
    engine: Option<EngineHandle>,
    desktop_lab: bool,
    settings: Settings,
    status: AppStatus,
    paste_target: Option<HWND>,
    overlay: HWND,
    pill_renderer: RefCell<Option<pill::Renderer>>,
    last_render_error: Cell<Option<Instant>>,
    pill_buttons: Option<pill_buttons::Buttons>,
    meter: meter::Meter,
    pill_state: pill_state::State,
    clock: Instant,
    last_transcript: String,
    last_pill_redraw: Cell<Option<Instant>>,
    last_pill_key: Cell<Option<pill_state::PaintKey>>,
    shortcut_window: HWND,
    registered_shortcut: Option<Shortcut>,
    levels: Option<AudioLevels>,
    recording_started: Option<Instant>,
    tray: NOTIFYICONDATAW,
    taskbar_created: u32,
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
    let desktop_lab = std::env::args().any(|argument| argument == "--desktop-lab");
    let engine = (!desktop_lab).then(|| runtime::start(settings.clone()));
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
        overlay: HWND::default(),
        pill_renderer: RefCell::new(None),
        last_render_error: Cell::new(None),
        pill_buttons: None,
        meter: meter::Meter::default(),
        pill_state: pill_state::State::default(),
        clock: Instant::now(),
        last_transcript: restored_last,
        last_pill_redraw: Cell::new(None),
        last_pill_key: Cell::new(None),
        shortcut_window: HWND::default(),
        registered_shortcut: None,
        levels: None,
        recording_started: None,
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
    app.update_tray();
    if unsafe { SetTimer(Some(hwnd), POLL_TIMER_ID, 33, None) } == 0 {
        return Err("could not start the Koett event timer".to_string());
    }
    drop(app);
    if let Some(error) = startup_warning {
        show_fatal_error(&format!(
            "Koett started, but start at login could not be enabled: {error}"
        ));
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
    let app_cell = APP.load(Ordering::Acquire);
    if !app_cell.is_null() {
        let app_cell = unsafe { &*app_cell };
        match message {
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
            WM_HOTKEY => {
                if let Ok(mut app) = app_cell.try_borrow_mut() {
                    app.toggle();
                }
                return Default::default();
            }
            WM_TIMER if wparam.0 == POLL_TIMER_ID => {
                if let Ok(mut app) = app_cell.try_borrow_mut() {
                    app.poll_updates();
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
                _ => -1,
            };
            if x >= 0
                && app
                    .pill_buttons
                    .as_ref()
                    .is_some_and(|buttons| buttons.allows(wparam.0 & 0xffff))
            {
                app.pill_click(x);
            }
        }
        return Default::default();
    }
    if message == WM_MOUSEACTIVATE {
        return windows::Win32::Foundation::LRESULT(MA_NOACTIVATE as isize);
    }
    if message == WM_LBUTTONUP {
        let cell = APP.load(Ordering::Acquire);
        if !cell.is_null()
            && let Ok(mut app) = unsafe { &*cell }.try_borrow_mut()
        {
            let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) }.max(96);
            app.pill_click(((lparam.0 as u16 as i16) as i32) * 96 / dpi as i32);
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
        WM_KEYDOWN | WM_SYSKEYDOWN if !app_cell.is_null() => {
            if let Some(shortcut) = shortcut_from_virtual_key(wparam.0 as u32)
                && let Ok(mut app) = unsafe { &*app_cell }.try_borrow_mut()
            {
                app.change_shortcut(shortcut);
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
    fn lab_phase(&mut self, command: usize) {
        use pill_state::Phase;
        let now = self.clock.elapsed().as_millis() as u64;
        let phase = match command {
            1 => {
                self.status = AppStatus::Recording;
                self.paste_target = Some(unsafe { GetForegroundWindow() });
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
                self.last_transcript = "Desktop lab result".into();
                Phase::Done(self.last_transcript.clone())
            }
            4 => {
                self.status = AppStatus::Ready;
                self.last_transcript = "Desktop lab recovery".into();
                Phase::NotPasted
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
        self.update_overlay();
    }

    fn toggle(&mut self) {
        if self.desktop_lab {
            self.lab_phase(if self.status == AppStatus::Recording {
                2
            } else {
                1
            });
            return;
        }
        if self.status != AppStatus::Ready
            && self.status != AppStatus::Recording
            && !matches!(self.status, AppStatus::Error(_))
        {
            return;
        }
        let focused = unsafe { GetForegroundWindow() };
        let focused = (!focused.is_invalid()
            && focused != self.hwnd
            && focused != self.overlay
            && focused != self.shortcut_window)
            .then_some(focused);
        if self.status == AppStatus::Ready || matches!(self.status, AppStatus::Error(_)) {
            self.paste_target = focused;
        }
        if self.status == AppStatus::Recording {
            // Keep the app that was focused when recording started if the
            // non-activating overlay is briefly reported during the stop.
            if focused.is_some() {
                self.paste_target = focused;
            }
        }
        if let Some(engine) = &self.engine {
            let _ = engine.commands.send(EngineCommand::Toggle);
        }
    }

    fn poll_updates(&mut self) {
        loop {
            let update = self
                .engine
                .as_ref()
                .and_then(|engine| engine.updates.try_recv().ok());
            let Some(update) = update else { break };
            match update {
                EngineUpdate::ActiveModel(_) => {}
                EngineUpdate::Notice(message) => {
                    let message = message
                        .strip_prefix("@model-notice:")
                        .unwrap_or(&message)
                        .to_string();
                    eprintln!("Koett: {message}");
                }
                EngineUpdate::Status(status) => {
                    let now = self.clock.elapsed().as_millis() as u64;
                    match &status {
                        AppStatus::Recording => {
                            self.pill_state.set(pill_state::Phase::Recording, now)
                        }
                        AppStatus::Transcribing => {
                            self.pill_state.set(pill_state::Phase::Transcribing, now)
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
                    self.status = status;
                    self.update_tray();
                    self.update_overlay();
                }
                EngineUpdate::RecordingStarted(levels) => {
                    self.meter = meter::Meter::default();
                    self.levels = Some(levels);
                    self.recording_started = Some(Instant::now());
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
                    set_wide_array(&mut self.tray.szTip, &format!("Koett — {text}"));
                    let _ = unsafe { Shell_NotifyIconW(NIM_MODIFY, &self.tray) };
                }
                EngineUpdate::FormattingBusy(_) => {}
                EngineUpdate::TranscriptReady {
                    model, transcript, ..
                } => {
                    self.deliver(&model, &transcript.text, transcript.raw.as_deref());
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
                        self.paste_target.unwrap_or_else(|| GetForegroundWindow()),
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

    fn pill_click(&mut self, x: i32) {
        let now = self.clock.elapsed().as_millis() as u64;
        match &self.pill_state.phase {
            pill_state::Phase::Recording | pill_state::Phase::Transcribing
                if self.pill_state.hovered =>
            {
                if (40..100).contains(&x) {
                    if let Some(engine) = &self.engine {
                        engine.cancel_dictation();
                    }
                    self.pill_state.set(pill_state::Phase::Hidden, now);
                    if self.desktop_lab {
                        self.status = AppStatus::Ready;
                    }
                } else if (100..175).contains(&x) && self.status == AppStatus::Recording {
                    if let Some(levels) = &self.levels {
                        self.pill_state.paused = !levels.paused();
                        levels.set_paused(self.pill_state.paused);
                    } else if self.desktop_lab {
                        self.pill_state.paused = !self.pill_state.paused;
                    }
                } else if (175..238).contains(&x) {
                    let _ = open_history();
                }
            }
            pill_state::Phase::Done(_) | pill_state::Phase::NotPasted => {
                if x >= 245 {
                    if copy_text(self.hwnd, &self.last_transcript).is_ok() {
                        self.pill_state
                            .set(pill_state::Phase::Done(self.last_transcript.clone()), now);
                        self.pill_state.copy(now);
                    }
                } else if x >= 190 && matches!(self.pill_state.phase, pill_state::Phase::NotPasted)
                {
                    if self.paste_target == Some(unsafe { GetForegroundWindow() })
                        && copy_text(self.hwnd, &self.last_transcript).is_ok()
                        && paste().is_ok()
                    {
                        self.pill_state
                            .set(pill_state::Phase::Done(self.last_transcript.clone()), now);
                    }
                } else if matches!(self.pill_state.phase, pill_state::Phase::NotPasted) {
                    self.pill_state.set(pill_state::Phase::Hidden, now);
                }
            }
            pill_state::Phase::Error { .. } => self.pill_state.set(pill_state::Phase::Hidden, now),
            _ => {}
        }
        self.update_overlay();
    }

    fn show_shortcut_window(&self) {
        unsafe {
            let _ = ShowWindow(self.shortcut_window, SW_SHOWNORMAL);
            let _ = SetForegroundWindow(self.shortcut_window);
        }
    }

    fn change_shortcut(&mut self, shortcut: Shortcut) {
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

    fn deliver(&mut self, model: &str, text: &str, raw: Option<&str>) {
        if text.trim().is_empty() {
            self.pill_state.set(
                pill_state::Phase::Hidden,
                self.clock.elapsed().as_millis() as u64,
            );
            self.update_overlay();
            return;
        }
        let delivery_error = if let Err(error) = copy_text(self.hwnd, text) {
            Some(error)
        } else if self.paste_target == Some(unsafe { GetForegroundWindow() })
            && let Err(error) = paste()
        {
            Some(format!("Copied. Automatic paste failed: {error}"))
        } else if self.paste_target != Some(unsafe { GetForegroundWindow() }) {
            Some("Focus changed. Transcript saved.".to_string())
        } else {
            None
        };
        self.last_transcript = text.to_string();
        let failed = delivery_error.is_some();
        let history_error = match history::append_transcript_with_raw_resilient(model, text, raw) {
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
        let phase = if history_failed {
            pill_state::Phase::Error {
                message: "History failed · Copy from tray".into(),
                rescue: true,
            }
        } else if failed {
            pill_state::Phase::NotPasted
        } else {
            pill_state::Phase::Done(text.to_string())
        };
        self.pill_state
            .set(phase, self.clock.elapsed().as_millis() as u64);
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
                self.handle_menu(selected);
            }
            let _ = Shell_NotifyIconW(NIM_SETFOCUS, &self.tray);
            let _ = DestroyMenu(menu);
        }
    }

    fn handle_menu(&mut self, command: usize) {
        match command {
            MENU_CHANGE_SHORTCUT => self.show_shortcut_window(),
            MENU_SETTINGS => desktop::open(true, self.hwnd),
            MENU_COPY_LAST => {
                if let Err(e) = copy_text(self.hwnd, &self.last_transcript) {
                    show_fatal_error(&e);
                }
            }
            MENU_RECORD => self.toggle(),
            MENU_PAUSE => {
                if let Some(levels) = &self.levels {
                    self.pill_state.paused = !levels.paused();
                    levels.set_paused(self.pill_state.paused);
                }
            }
            MENU_CANCEL => {
                if let Some(engine) = &self.engine {
                    engine.cancel_dictation();
                }
                self.pill_state.set(
                    pill_state::Phase::Hidden,
                    self.clock.elapsed().as_millis() as u64,
                );
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
            MENU_QUIT => unsafe { PostQuitMessage(0) },
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
            if let Err(error) = current.paint(
                hwnd,
                bounds,
                &app.meter.positions(),
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
    let encoded = text.encode_utf16().chain([0]).collect::<Vec<_>>();
    for _ in 0..10 {
        if unsafe { OpenClipboard(Some(owner)) }.is_ok() {
            let result = unsafe { write_open_clipboard(&encoded) };
            unsafe {
                let _ = CloseClipboard();
            }
            return result;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Err("could not open the Windows clipboard".to_string())
}

unsafe fn write_open_clipboard(encoded: &[u16]) -> Result<(), String> {
    unsafe { EmptyClipboard() }
        .map_err(|error| format!("could not clear the Windows clipboard: {error}"))?;
    let memory = unsafe { GlobalAlloc(GMEM_MOVEABLE, std::mem::size_of_val(encoded)) }
        .map_err(|error| format!("could not allocate clipboard text: {error}"))?;
    let destination = unsafe { GlobalLock(memory) }.cast::<u16>();
    if destination.is_null() {
        unsafe {
            let _ = GlobalFree(Some(memory));
        }
        return Err("could not lock clipboard text".to_string());
    }
    unsafe {
        ptr::copy_nonoverlapping(encoded.as_ptr(), destination, encoded.len());
        let _ = GlobalUnlock(memory);
    }
    let result = unsafe { SetClipboardData(CF_UNICODETEXT.0.into(), Some(HANDLE(memory.0))) };
    if let Err(error) = result {
        unsafe {
            let _ = GlobalFree(Some(memory));
        }
        return Err(format!("could not set clipboard text: {error}"));
    }
    Ok(())
}

fn paste() -> Result<(), String> {
    let modifiers_are_down = || {
        [VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN]
            .into_iter()
            .any(|key| unsafe { GetAsyncKeyState(i32::from(key.0)) } < 0)
    };
    let deadline = Instant::now() + Duration::from_millis(500);
    while modifiers_are_down() {
        if Instant::now() >= deadline {
            return Err(
                "release the shortcut keys; the transcript is still on the clipboard".to_string(),
            );
        }
        std::thread::sleep(Duration::from_millis(5));
    }

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
    let inputs = [
        keyboard(VK_CONTROL, Default::default()),
        keyboard(VIRTUAL_KEY(b'V' as u16), Default::default()),
        keyboard(VIRTUAL_KEY(b'V' as u16), KEYEVENTF_KEYUP),
        keyboard(VK_CONTROL, KEYEVENTF_KEYUP),
    ];
    let inserted = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
    if inserted == inputs.len() as u32 {
        Ok(())
    } else {
        Err("Windows blocked the paste; the transcript is still on the clipboard".to_string())
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use windows::Win32::UI::Input::KeyboardAndMouse::{MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT};

    use super::{Shortcut, tray_anchor, windows_shortcut};

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
