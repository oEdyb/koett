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
    BeginPaint, CreateSolidBrush, DT_CENTER, DT_SINGLELINE, DT_VCENTER, DeleteObject, DrawTextW,
    EndPaint, FillRect, GetMonitorInfoW, GetStockObject, HGDIOBJ, InvalidateRect,
    MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow, NULL_PEN, PAINTSTRUCT, RoundRect,
    SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
    RegCreateKeyExW, RegDeleteValueW, RegSetValueExW,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, HOT_KEY_MODIFIERS, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_KEYUP, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey,
    SendInput, UnregisterHotKey, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::Shell::{
    NIF_GUID, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NIM_SETFOCUS,
    NIM_SETVERSION, NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW, ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CW_USEDEFAULT, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
    DestroyWindow, DispatchMessageW, GetClientRect, GetCursorPos, GetForegroundWindow, GetMessageW,
    HWND_TOPMOST, IDI_APPLICATION, KillTimer, LWA_ALPHA, LoadIconW, MF_CHECKED, MF_DISABLED,
    MF_SEPARATOR, MF_STRING, MSG, MessageBoxW, PostQuitMessage, RegisterClassExW,
    RegisterWindowMessageW, SW_HIDE, SW_SHOWNOACTIVATE, SW_SHOWNORMAL, SWP_NOACTIVATE,
    SWP_SHOWWINDOW, SetForegroundWindow, SetLayeredWindowAttributes, SetTimer, SetWindowPos,
    ShowWindow, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_CLOSE, WM_DESTROY, WM_HOTKEY, WM_KEYDOWN, WM_LBUTTONUP, WM_PAINT,
    WM_RBUTTONUP, WM_SYSKEYDOWN, WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_OVERLAPPEDWINDOW, WS_POPUP,
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
const MENU_CHANGE_SHORTCUT: usize = 1;
const MENU_OPEN_HISTORY: usize = 2;
const MENU_START_AT_LOGIN: usize = 3;
const MENU_QUIT: usize = 4;
const TRAY_GUID: GUID = GUID::from_u128(0xc65ddae4_119c_486e_8bed_e30a24f2fb1f);

static APP: AtomicPtr<WindowsApp> = AtomicPtr::new(ptr::null_mut());

struct WindowsApp {
    hwnd: HWND,
    engine: Option<EngineHandle>,
    settings: Settings,
    status: AppStatus,
    paste_target: Option<HWND>,
    overlay: HWND,
    shortcut_window: HWND,
    registered_shortcut: Shortcut,
    levels: Option<AudioLevels>,
    recording_started: Option<Instant>,
    tray: NOTIFYICONDATAW,
    taskbar_created: u32,
}

pub fn run() -> Result<(), String> {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    let mutex = single_instance()?;
    let settings = Settings::load()?;
    let shortcut = Shortcut::from_str(&settings.shortcut)?;
    if settings.start_at_login {
        set_start_at_login(true)?;
    }
    let engine = runtime::start(settings.clone());
    let mut app = Box::new(WindowsApp {
        hwnd: HWND::default(),
        engine: Some(engine),
        settings,
        status: AppStatus::Starting,
        paste_target: None,
        overlay: HWND::default(),
        shortcut_window: HWND::default(),
        registered_shortcut: shortcut,
        levels: None,
        recording_started: None,
        tray: NOTIFYICONDATAW::default(),
        taskbar_created: 0,
    });

    let result = unsafe { run_message_loop(&mut app, shortcut) };
    APP.store(ptr::null_mut(), Ordering::Release);
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

fn single_instance() -> Result<HANDLE, String> {
    let handle = unsafe { CreateMutexW(None, true, w!("Local\\Koett")) }
        .map_err(|error| format!("could not create the Koett process lock: {error}"))?;
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe {
            let _ = CloseHandle(handle);
        }
        return Err("Koett is already running".to_string());
    }
    Ok(handle)
}

unsafe fn run_message_loop(app: &mut Box<WindowsApp>, shortcut: Shortcut) -> Result<(), String> {
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
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT,
            w!("KoettOverlayWindow"),
            w!("Koett Recording"),
            WS_POPUP,
            0,
            0,
            260,
            52,
            None,
            None,
            Some(instance.into()),
            None,
        )
    }
    .map_err(|error| format!("could not create the Koett recording pill: {error}"))?;
    unsafe {
        SetLayeredWindowAttributes(app.overlay, COLORREF::default(), 242, LWA_ALPHA)
            .map_err(|error| format!("could not style the Koett recording pill: {error}"))?;
    }
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
    APP.store(&mut **app, Ordering::Release);
    app.taskbar_created = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
    app.add_tray()?;

    let (modifiers, virtual_key) = windows_shortcut(shortcut);
    unsafe { RegisterHotKey(Some(hwnd), HOTKEY_ID, modifiers | MOD_NOREPEAT, virtual_key) }
        .map_err(|_| format!("the shortcut {shortcut} is already in use"))?;
    if unsafe { SetTimer(Some(hwnd), POLL_TIMER_ID, 33, None) } == 0 {
        return Err("could not start the Koett event timer".to_string());
    }

    let mut message = MSG::default();
    while unsafe { GetMessageW(&mut message, None, 0, 0) }.as_bool() {
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }

    unsafe {
        let _ = KillTimer(Some(hwnd), POLL_TIMER_ID);
        let _ = UnregisterHotKey(Some(hwnd), HOTKEY_ID);
    }
    Ok(())
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    let app = APP.load(Ordering::Acquire);
    if !app.is_null() {
        let app = unsafe { &mut *app };
        match message {
            WM_HOTKEY => {
                app.toggle();
                return Default::default();
            }
            WM_TIMER if wparam.0 == POLL_TIMER_ID => {
                app.poll_updates();
                if app.status == AppStatus::Recording {
                    unsafe {
                        let _ = InvalidateRect(Some(app.overlay), None, false);
                    }
                }
                return Default::default();
            }
            TRAY_MESSAGE => {
                let event = lparam.0 as u32 & 0xffff;
                if event == WM_RBUTTONUP || event == WM_LBUTTONUP {
                    app.show_menu();
                }
                return Default::default();
            }
            WM_DESTROY => {
                unsafe { PostQuitMessage(0) };
                return Default::default();
            }
            _ => {}
        }
        if message == app.taskbar_created {
            let _ = app.add_tray();
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
    if message == WM_PAINT {
        let app = APP.load(Ordering::Acquire);
        if !app.is_null() {
            unsafe { paint_overlay(hwnd, &*app) };
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
    let app = APP.load(Ordering::Acquire);
    match message {
        WM_KEYDOWN | WM_SYSKEYDOWN if !app.is_null() => {
            if let Some(shortcut) = shortcut_from_virtual_key(wparam.0 as u32) {
                unsafe { &mut *app }.change_shortcut(shortcut);
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
    fn toggle(&mut self) {
        if self.status == AppStatus::Recording {
            let focused = unsafe { GetForegroundWindow() };
            self.paste_target = (focused != self.hwnd).then_some(focused);
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
                EngineUpdate::Status(status) => {
                    if let AppStatus::Error(error) = &status {
                        show_fatal_error(error);
                    }
                    self.status = status;
                    self.update_tray();
                    self.update_overlay();
                }
                EngineUpdate::RecordingStarted(levels) => {
                    self.levels = Some(levels);
                    self.recording_started = Some(Instant::now());
                    self.update_overlay();
                }
                EngineUpdate::ModelProgress(progress) => {
                    use crate::model::ModelProgress;
                    let text = match progress {
                        ModelProgress::Downloading { received, total } => {
                            format!("Downloading model — {}%", received * 100 / total)
                        }
                        ModelProgress::Installing => "Installing model".to_string(),
                        ModelProgress::Ready => "Loading model".to_string(),
                    };
                    set_wide_array(&mut self.tray.szTip, &format!("Koett — {text}"));
                    let _ = unsafe { Shell_NotifyIconW(NIM_MODIFY, &self.tray) };
                }
                EngineUpdate::TranscriptReady { model, transcript } => {
                    self.deliver(&model, &transcript.text);
                }
            }
        }
    }

    fn update_overlay(&self) {
        unsafe {
            if self.status == AppStatus::Recording {
                position_overlay(self.overlay, GetForegroundWindow());
                let _ = ShowWindow(self.overlay, SW_SHOWNOACTIVATE);
            } else {
                let _ = ShowWindow(self.overlay, SW_HIDE);
            }
        }
    }

    fn show_shortcut_window(&self) {
        unsafe {
            let _ = ShowWindow(self.shortcut_window, SW_SHOWNORMAL);
            let _ = SetForegroundWindow(self.shortcut_window);
        }
    }

    fn change_shortcut(&mut self, shortcut: Shortcut) {
        if shortcut == self.registered_shortcut {
            unsafe {
                let _ = ShowWindow(self.shortcut_window, SW_HIDE);
            }
            return;
        }

        let old = self.registered_shortcut;
        unsafe {
            let _ = UnregisterHotKey(Some(self.hwnd), HOTKEY_ID);
        }
        let (modifiers, key) = windows_shortcut(shortcut);
        if unsafe { RegisterHotKey(Some(self.hwnd), HOTKEY_ID, modifiers | MOD_NOREPEAT, key) }
            .is_err()
        {
            let (old_modifiers, old_key) = windows_shortcut(old);
            let restored = unsafe {
                RegisterHotKey(
                    Some(self.hwnd),
                    HOTKEY_ID,
                    old_modifiers | MOD_NOREPEAT,
                    old_key,
                )
            };
            if restored.is_err() {
                show_fatal_error("Koett could not restore the old shortcut. Restart Koett.");
            } else {
                show_fatal_error("That shortcut is already in use. The old shortcut still works.");
            }
            return;
        }

        let old_text = self.settings.shortcut.clone();
        self.settings.shortcut = shortcut.to_string();
        if let Err(error) = self.settings.save() {
            unsafe {
                let _ = UnregisterHotKey(Some(self.hwnd), HOTKEY_ID);
            }
            let (old_modifiers, old_key) = windows_shortcut(old);
            let _ = unsafe {
                RegisterHotKey(
                    Some(self.hwnd),
                    HOTKEY_ID,
                    old_modifiers | MOD_NOREPEAT,
                    old_key,
                )
            };
            self.settings.shortcut = old_text;
            show_fatal_error(&error);
            return;
        }
        self.registered_shortcut = shortcut;
        unsafe {
            let _ = ShowWindow(self.shortcut_window, SW_HIDE);
        }
    }

    fn deliver(&mut self, model: &str, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        if let Err(error) = copy_text(self.hwnd, text) {
            show_fatal_error(&error);
        } else if self.paste_target == Some(unsafe { GetForegroundWindow() })
            && let Err(error) = paste()
        {
            show_fatal_error(&format!("Copied. Automatic paste failed: {error}"));
        }
        if let Err(error) = history::append_transcript(model, text) {
            show_fatal_error(&format!(
                "The text was delivered, but history failed: {error}"
            ));
        }
        self.paste_target = None;
    }

    fn add_tray(&mut self) -> Result<(), String> {
        let mut tray = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: 1,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_GUID,
            uCallbackMessage: TRAY_MESSAGE,
            hIcon: unsafe { LoadIconW(None, IDI_APPLICATION) }
                .map_err(|error| format!("could not load the Koett tray icon: {error}"))?,
            guidItem: TRAY_GUID,
            ..Default::default()
        };
        set_wide_array(&mut tray.szTip, "Koett — Starting");
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
        let status = status_text(&self.status);
        set_wide_array(&mut self.tray.szTip, &format!("Koett — {status}"));
        let _ = unsafe { Shell_NotifyIconW(NIM_MODIFY, &self.tray) };
    }

    fn show_menu(&mut self) {
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
            let _ = AppendMenuW(menu, MF_STRING, MENU_OPEN_HISTORY, w!("Open Transcripts"));
            let startup_flags = if self.settings.start_at_login {
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
            if GetCursorPos(&mut point).is_ok() {
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
            MENU_OPEN_HISTORY => {
                if let Err(error) = open_history() {
                    show_fatal_error(&error);
                }
            }
            MENU_START_AT_LOGIN => {
                let enabled = !self.settings.start_at_login;
                match set_start_at_login(enabled) {
                    Ok(()) => {
                        self.settings.start_at_login = enabled;
                        if let Err(error) = self.settings.save() {
                            show_fatal_error(&error);
                        }
                    }
                    Err(error) => show_fatal_error(&error),
                }
            }
            MENU_QUIT => unsafe { PostQuitMessage(0) },
            _ => {}
        }
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
        let width = 260;
        let height = 52;
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
    let dc = unsafe { BeginPaint(hwnd, &mut paint) };
    let background = unsafe { CreateSolidBrush(COLORREF(0x00211e1d)) };
    let coral = unsafe { CreateSolidBrush(COLORREF(0x006b7dff)) };
    let old_brush = unsafe { SelectObject(dc, HGDIOBJ(background.0)) };
    let old_pen = unsafe { SelectObject(dc, GetStockObject(NULL_PEN)) };
    let _ = unsafe { RoundRect(dc, 0, 0, 260, 52, 26, 26) };

    let level = app
        .levels
        .as_ref()
        .map(AudioLevels::current)
        .map(|(rms, _)| (rms * 18.0).clamp(0.08, 1.0))
        .unwrap_or(0.08);
    let _ = unsafe { SelectObject(dc, HGDIOBJ(coral.0)) };
    let phase = app
        .recording_started
        .map(|started| started.elapsed().as_secs_f32() * 8.0)
        .unwrap_or(0.0);
    for index in 0..14 {
        let wave = ((index as f32 * 0.7 + phase).sin().abs() * 0.65 + 0.35) * level;
        let height = (6.0 + wave * 24.0) as i32;
        let left = 20 + index * 7;
        let rect = RECT {
            left,
            top: 26 - height / 2,
            right: left + 3,
            bottom: 26 + height / 2,
        };
        unsafe { FillRect(dc, &rect, coral) };
    }

    let elapsed = app
        .recording_started
        .map(|started| started.elapsed().as_secs())
        .unwrap_or(0);
    let mut timer = format!("{:02}:{:02}", elapsed / 60, elapsed % 60)
        .encode_utf16()
        .collect::<Vec<_>>();
    let mut timer_rect = RECT {
        left: 130,
        top: 0,
        right: 238,
        bottom: 52,
    };
    unsafe {
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, COLORREF(0x00ffffff));
        DrawTextW(
            dc,
            &mut timer,
            &mut timer_rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
        let _ = SelectObject(dc, old_pen);
        let _ = SelectObject(dc, old_brush);
        let _ = DeleteObject(HGDIOBJ(coral.0));
        let _ = DeleteObject(HGDIOBJ(background.0));
        let _ = EndPaint(hwnd, &paint);
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
    let path = crate::paths::history_file()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    }
    if !path.exists() {
        std::fs::write(&path, "# Koett Transcripts\n\n")
            .map_err(|error| format!("could not create {}: {error}", path.display()))?;
    }
    let path = wide(&path.display().to_string());
    unsafe {
        let _ = ShellExecuteW(
            None,
            w!("open"),
            PCWSTR::from_raw(path.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        );
    }
    Ok(())
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

    use super::{Shortcut, windows_shortcut};

    #[test]
    fn default_shortcut_maps_to_win32() {
        let (modifiers, key) = windows_shortcut(Shortcut::from_str("Ctrl+Shift+Space").unwrap());
        assert_eq!(
            (modifiers | MOD_NOREPEAT).0,
            0x4000 | MOD_CONTROL.0 | MOD_SHIFT.0
        );
        assert_eq!(key, 0x20);
    }
}
