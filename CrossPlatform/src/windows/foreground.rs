use super::{PasteTarget, external_paste_target};
use std::cell::Cell;
use windows::Win32::{
    Foundation::*,
    UI::{Accessibility::*, WindowsAndMessaging::*},
};

thread_local! {
    static RECENT: Cell<Option<PasteTarget>> = const { Cell::new(None) };
    static GENERATION: Cell<u64> = const { Cell::new(0) };
}

pub(super) struct Tracker(HWINEVENTHOOK);
impl Tracker {
    pub fn start() -> Result<Self, String> {
        let hook = unsafe {
            SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(changed),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            )
        };
        if hook.is_invalid() {
            RECENT.with(|recent| recent.set(None));
            Err("Could not track Windows tray focus".into())
        } else {
            observe(unsafe { GetForegroundWindow() });
            Ok(Self(hook))
        }
    }
}
impl Drop for Tracker {
    fn drop(&mut self) {
        let _ = unsafe { UnhookWinEvent(self.0) };
    }
}
unsafe extern "system" fn changed(
    _: HWINEVENTHOOK,
    _: u32,
    hwnd: HWND,
    _: i32,
    _: i32,
    _: u32,
    _: u32,
) {
    observe(hwnd);
}
fn observe(hwnd: HWND) {
    let previous = RECENT.with(|recent| recent.replace(None));
    if std::env::var_os("KOETT_CLIPBOARD_TRACE").is_some() {
        let mut name = [0; 128];
        let count = unsafe { GetClassNameW(hwnd, &mut name) };
        eprintln!(
            "tray_focus hwnd={} current={} class={} tray={} previous={}",
            hwnd.0 as usize,
            unsafe { GetForegroundWindow() }.0 as usize,
            String::from_utf16_lossy(&name[..count.max(0) as usize]),
            is_tray(hwnd),
            previous.map_or(0, |target| target.hwnd.0 as usize)
        );
    }
    let generation = GENERATION.with(|value| {
        let next = value.get().wrapping_add(1);
        value.set(next);
        next
    });
    let next = if is_tray(hwnd) {
        previous
    } else {
        external_paste_target(hwnd)
    };
    if GENERATION.with(Cell::get) == generation {
        RECENT.with(|recent| recent.set(next));
    }
}
pub(super) fn is_tray(hwnd: HWND) -> bool {
    let shell_window = unsafe { GetShellWindow() };
    if shell_window.is_invalid() {
        return false;
    }
    let (mut shell, mut process) = (0, 0);
    unsafe {
        GetWindowThreadProcessId(shell_window, Some(&mut shell));
        GetWindowThreadProcessId(hwnd, Some(&mut process));
    }
    if shell == 0 || process != shell {
        return false;
    }
    let mut name = [0; 128];
    let length = unsafe { GetClassNameW(hwnd, &mut name) };
    matches!(
        String::from_utf16_lossy(&name[..length.max(0) as usize]).as_str(),
        "Shell_TrayWnd"
            | "Shell_SecondaryTrayWnd"
            | "NotifyIconOverflowWindow"
            | "TopLevelWindowForOverflowXamlIsland"
    )
}
pub(super) fn menu_target() -> Option<PasteTarget> {
    let hwnd = unsafe { GetForegroundWindow() };
    if std::env::var_os("KOETT_CLIPBOARD_TRACE").is_some() {
        eprintln!(
            "tray_menu hwnd={} recent={}",
            hwnd.0 as usize,
            RECENT
                .with(Cell::get)
                .map_or(0, |target| target.hwnd.0 as usize)
        );
    }
    let target = if is_tray(hwnd) {
        RECENT.with(Cell::get)
    } else {
        external_paste_target(hwnd)
    }?;
    (external_paste_target(target.hwnd) == Some(target)).then_some(target)
}
