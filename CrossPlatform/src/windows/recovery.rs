mod layout;
use super::*;
use std::sync::{
    LazyLock, Mutex,
    atomic::{AtomicBool, AtomicUsize},
};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::PCWSTR;
static OPEN: AtomicBool = AtomicBool::new(false);
static BUSY: AtomicBool = AtomicBool::new(false);
struct Operation;
impl Drop for Operation {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}
static WINDOW: AtomicUsize = AtomicUsize::new(0);
static SNAPSHOT: LazyLock<Mutex<Snapshot>> = LazyLock::new(|| Mutex::new(Snapshot::default()));
#[derive(Default, Clone)]
struct Snapshot {
    text: String,
    message: String,
    count: usize,
}
fn buttons(has_text: bool, busy: bool) -> [(i32, bool); 3] {
    [(1, has_text && !busy), (3, has_text && !busy), (2, true)]
}
pub(super) fn busy() -> bool {
    BUSY.load(Ordering::Acquire)
}
pub(super) fn request(retry: bool, copy: Option<HWND>) -> Result<(), String> {
    if BUSY.swap(true, Ordering::AcqRel) {
        return Err("Transcript recovery is busy".into());
    }
    let owner = copy.map(|handle| handle.0 as usize);
    let sequence = unsafe { windows::Win32::System::DataExchange::GetClipboardSequenceNumber() };
    let spawned = std::thread::Builder::new()
        .name("koett-transcript-recovery".into())
        .spawn(move || {
            let _operation = Operation;
            let result: Result<Snapshot, String> = (|| {
                if retry {
                    history::recovery::retry()?;
                }
                let count = history::recovery::pending_count();
                let text = if count == 0 {
                    String::new()
                } else {
                    history::recovery::unsaved()?
                };
                if let Some(owner) = owner {
                    if text.is_empty() {
                        return Err("No unsaved transcripts".into());
                    }
                    if unsafe { windows::Win32::System::DataExchange::GetClipboardSequenceNumber() }
                        != sequence
                    {
                        return Err("Clipboard changed · Copy again to replace it".into());
                    }
                    clipboard::copy_text_if_unchanged(HWND(owner as *mut _), &text, sequence)?;
                }
                Ok(Snapshot {
                    count,
                    text,
                    message: if owner.is_some() {
                        "Copied · save still pending".into()
                    } else if count == 0 {
                        "All transcripts saved".into()
                    } else {
                        format!("{count} unsaved · Copy or Retry save")
                    },
                })
            })();
            if let Ok(mut snapshot) = SNAPSHOT.lock() {
                match result {
                    Ok(value) => *snapshot = value,
                    Err(error) => {
                        snapshot.text = history::recovery::unsaved().unwrap_or_default();
                        eprintln!("Transcript recovery failed: {error}");
                        snapshot.message = if retry {
                            "Not saved · check disk space and permissions".into()
                        } else {
                            "Copy unavailable · try again or retry save".into()
                        };
                        snapshot.count = history::recovery::pending_count();
                    }
                }
            }
            BUSY.store(false, Ordering::Release);
        });
    if let Err(error) = spawned {
        BUSY.store(false, Ordering::Release);
        return Err(error.to_string());
    }
    Ok(())
}
pub(super) fn open() {
    if OPEN.swap(true, Ordering::AcqRel) {
        let handle = HWND(WINDOW.load(Ordering::Acquire) as *mut _);
        if !handle.is_invalid() {
            unsafe {
                let _ = ShowWindow(handle, SW_RESTORE);
                let _ = SetForegroundWindow(handle);
            }
        }
        return;
    }
    if std::thread::Builder::new()
        .name("koett-recovery-window".into())
        .spawn(|| {
            if let Err(error) = run() {
                show_fatal_error(&error);
            }
            WINDOW.store(0, Ordering::Release);
            OPEN.store(false, Ordering::Release);
        })
        .is_err()
    {
        OPEN.store(false, Ordering::Release);
    }
}
unsafe fn control(
    parent: HWND,
    class: PCWSTR,
    title: &str,
    id: usize,
    style: WINDOW_STYLE,
    rect: [i32; 4],
) -> Result<HWND, String> {
    unsafe {
        CreateWindowExW(
            Default::default(),
            class,
            PCWSTR(wide(title).as_ptr()),
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
    .map_err(|error| error.to_string())
}
fn run() -> Result<(), String> {
    unsafe {
        let instance = GetModuleHandleW(None).map_err(|error| error.to_string())?;
        let class = w!("KoettRecoveryWindow");
        let _ = RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: instance.into(),
            lpszClassName: class,
            ..Default::default()
        });
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class,
            w!("Transcript recovery"),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            600,
            420,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .map_err(|error| error.to_string())?;
        WINDOW.store(hwnd.0 as usize, Ordering::Release);
        control(
            hwnd,
            w!("STATIC"),
            "Loading…",
            101,
            Default::default(),
            [20, 16, 520, 44],
        )?;
        let transcript = control(
            hwnd,
            w!("EDIT"),
            "",
            102,
            WS_TABSTOP | WS_BORDER | WS_VSCROLL | WINDOW_STYLE(0x4 | 0x40 | 0x800),
            [20, 68, 520, 220],
        )?;
        if !windows::Win32::UI::Shell::SetWindowSubclass(transcript, Some(text_proc), 1, 0)
            .as_bool()
        {
            return Err("Could not enable transcript keyboard navigation".into());
        }
        control(
            hwnd,
            w!("BUTTON"),
            "Copy unsaved",
            1,
            WS_TABSTOP,
            [20, 304, 164, 32],
        )?;
        control(
            hwnd,
            w!("BUTTON"),
            "Retry save",
            3,
            WS_TABSTOP,
            [196, 304, 164, 32],
        )?;
        control(
            hwnd,
            w!("BUTTON"),
            "Close",
            IDCANCEL.0 as usize,
            WS_TABSTOP,
            [372, 304, 164, 32],
        )?;
        let _ = request(false, None);
        let dpi = windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd).max(96) as i32;
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            600 * dpi / 96,
            420 * dpi / 96,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
        layout::update(hwnd, true);
        let _ = SetTimer(Some(hwnd), 1, 200, None);
        let _ = SetForegroundWindow(hwnd);
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            if message.message == WM_DPICHANGED || !IsDialogMessageW(hwnd, &message).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
    Ok(())
}
unsafe extern "system" fn proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if let Some(result) = layout::color(hwnd, message, wparam) {
            return result;
        }
        match message {
            WM_SIZE => {
                layout::update(hwnd, false);
                LRESULT(0)
            }
            WM_SETTINGCHANGE | WM_THEMECHANGED => {
                layout::update(hwnd, true);
                LRESULT(0)
            }
            WM_DPICHANGED => {
                if lparam.0 != 0 {
                    let r = *(lparam.0 as *const RECT);
                    let _ = SetWindowPos(
                        hwnd,
                        None,
                        r.left,
                        r.top,
                        r.right - r.left,
                        r.bottom - r.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
                layout::update(hwnd, false);
                LRESULT(0)
            }
            WM_COMMAND => {
                match wparam.0 & 0xffff {
                    1 => {
                        let _ = request(false, Some(hwnd));
                    }
                    3 => {
                        let _ = request(true, None);
                    }
                    value if value == IDCANCEL.0 as usize => {
                        let _ = DestroyWindow(hwnd);
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            WM_TIMER => {
                layout::reconcile(hwnd);
                if let Ok(snapshot) = SNAPSHOT.try_lock() {
                    if !busy() && snapshot.count != history::recovery::pending_count() {
                        let _ = request(false, None);
                    }
                    for (id, text) in [
                        (
                            101,
                            if busy() {
                                "Saving…"
                            } else {
                                &snapshot.message
                            },
                        ),
                        (102, &snapshot.text),
                    ] {
                        if let Ok(control) = GetDlgItem(Some(hwnd), id) {
                            let mut current =
                                vec![0u16; GetWindowTextLengthW(control) as usize + 1];
                            let length = GetWindowTextW(control, &mut current);
                            let current = String::from_utf16_lossy(&current[..length as usize]);
                            if current != text {
                                let _ = SetWindowTextW(control, PCWSTR(wide(text).as_ptr()));
                            }
                        }
                    }
                    for (id, enabled) in buttons(!snapshot.text.is_empty(), busy()) {
                        if let Ok(control) = GetDlgItem(Some(hwnd), id) {
                            let _ = windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow(
                                control, enabled,
                            );
                        }
                    }
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                layout::clear();
                let _ = KillTimer(Some(hwnd), 1);
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn close_always_works_and_copy_retry_require_available_text() {
        for has_text in [false, true] {
            for busy in [false, true] {
                let controls = buttons(has_text, busy);
                assert_eq!(controls[2], (2, true));
                assert_eq!(controls[0], (1, has_text && !busy));
                assert_eq!(controls[1], (3, has_text && !busy));
            }
        }
    }
}

fn dialog_code(native: isize, key: usize) -> isize {
    if matches!(key, 9 | 13 | 27) {
        native & !(DLGC_WANTALLKEYS as isize | DLGC_WANTTAB as isize)
    } else {
        native
    }
}
unsafe extern "system" fn text_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    unsafe {
        use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass};
        if message == WM_NCDESTROY {
            let _ = RemoveWindowSubclass(hwnd, Some(text_proc), 1);
        }
        let native = DefSubclassProc(hwnd, message, wparam, lparam);
        if message == WM_GETDLGCODE {
            LRESULT(dialog_code(native.0, wparam.0))
        } else {
            native
        }
    }
}
#[cfg(test)]
mod keyboard_tests {
    use super::*;
    #[test]
    fn transcript_leaves_tab_enter_escape_to_dialog_but_keeps_copy_and_arrows() {
        let native = 141;
        for key in [9, 13, 27] {
            assert_eq!(
                dialog_code(native, key) & (DLGC_WANTALLKEYS as isize | DLGC_WANTTAB as isize),
                0
            );
        }
        for key in [0x43, 0x25, 0x27] {
            assert_eq!(dialog_code(native, key), native);
        }
    }
}
