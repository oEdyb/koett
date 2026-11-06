use super::*;
use std::cell::{Cell, RefCell};
use windows::Win32::{Graphics::Gdi::*, UI::HiDpi::GetDpiForWindow};
thread_local! {
    static THEME: RefCell<Option<native_theme::Theme>> = const { RefCell::new(None) };
    static FONT: Cell<HFONT> = const { Cell::new(HFONT(std::ptr::null_mut())) };
}
fn dpi(hwnd: HWND) -> i32 {
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let (mut x, mut y) = (0, 0);
    if unsafe {
        windows::Win32::UI::HiDpi::GetDpiForMonitor(
            monitor,
            windows::Win32::UI::HiDpi::MDT_EFFECTIVE_DPI,
            &mut x,
            &mut y,
        )
    }
    .is_ok()
        && x != 0
    {
        x as i32
    } else {
        unsafe { GetDpiForWindow(hwnd) }.max(96) as i32
    }
}
pub(super) fn reconcile(hwnd: HWND) {
    unsafe {
        let dpi = dpi(hwnd);
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_ok()
            && (rect.right - rect.left != 600 * dpi / 96
                || rect.bottom - rect.top != 420 * dpi / 96)
        {
            let _ = SetWindowPos(
                hwnd,
                None,
                0,
                0,
                600 * dpi / 96,
                420 * dpi / 96,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
            update(hwnd, false);
        }
    }
}
pub(super) fn update(hwnd: HWND, theme: bool) {
    unsafe {
        if theme && let Ok(value) = native_theme::Theme::new() {
            value.apply(hwnd);
            THEME.with(|current| current.replace(Some(value)));
        }
        let dpi = dpi(hwnd);
        let font = CreateFontW(
            -16 * dpi / 96,
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
            DEFAULT_QUALITY,
            0,
            w!("Segoe UI"),
        );
        let mut rect = RECT::default();
        let _ = GetClientRect(hwnd, &mut rect);
        let margin = 20 * dpi / 96;
        let button = 32 * dpi / 96;
        let gap = 12 * dpi / 96;
        let width = (rect.right - 2 * margin).max(1);
        let row = rect.bottom - margin - button;
        let columns = (width - 2 * gap) / 3;
        for (id, x, y, w, h) in [
            (101, margin, margin, width, 40 * dpi / 96),
            (
                102,
                margin,
                68 * dpi / 96,
                width,
                (row - 80 * dpi / 96).max(1),
            ),
            (1, margin, row, columns, button),
            (3, margin + columns + gap, row, columns, button),
            (2, margin + 2 * (columns + gap), row, columns, button),
        ] {
            if let Ok(control) = GetDlgItem(Some(hwnd), id) {
                let _ = SetWindowPos(control, None, x, y, w, h, SWP_NOZORDER | SWP_NOACTIVATE);
                SendMessageW(
                    control,
                    WM_SETFONT,
                    Some(WPARAM(font.0 as usize)),
                    Some(LPARAM(1)),
                );
            }
        }
        FONT.with(|current| {
            let old = current.replace(font);
            if !old.is_invalid() {
                let _ = DeleteObject(HGDIOBJ(old.0));
            }
        });
        let _ = InvalidateRect(Some(hwnd), None, true);
    }
}
pub(super) fn color(hwnd: HWND, message: u32, wparam: WPARAM) -> Option<LRESULT> {
    THEME.with(|theme| {
        let theme = theme.try_borrow().ok()?;
        let theme = theme.as_ref()?;
        if matches!(
            message,
            WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORBTN | WM_CTLCOLORDLG
        ) {
            unsafe {
                let dc = HDC(wparam.0 as *mut _);
                SetTextColor(dc, theme.foreground);
                SetBkColor(dc, theme.background);
            }
            Some(LRESULT(theme.brush.0 as isize))
        } else if message == WM_ERASEBKGND {
            unsafe {
                let mut rect = RECT::default();
                let _ = GetClientRect(hwnd, &mut rect);
                FillRect(HDC(wparam.0 as *mut _), &rect, theme.brush);
            }
            Some(LRESULT(1))
        } else {
            None
        }
    })
}
pub(super) fn clear() {
    FONT.with(|current| {
        let old = current.replace(HFONT(std::ptr::null_mut()));
        if !old.is_invalid() {
            unsafe {
                let _ = DeleteObject(HGDIOBJ(old.0));
            }
        }
    });
    THEME.with(|current| current.replace(None));
}
