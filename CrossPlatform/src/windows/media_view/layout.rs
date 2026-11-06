use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        UI::{HiDpi::GetDpiForWindow, WindowsAndMessaging::*},
    },
    core::w,
};
#[derive(Clone, Copy)]
struct Paint {
    background: COLORREF,
    foreground: COLORREF,
    brush: HBRUSH,
}
thread_local! { static PAINT: std::cell::Cell<Option<Paint>> = const { std::cell::Cell::new(None) }; }
thread_local! { static DPI: std::cell::Cell<u32> = const { std::cell::Cell::new(0) }; }
pub(super) fn dpi(hwnd: HWND) -> u32 {
    let dpi = DPI.with(std::cell::Cell::get);
    if dpi == 0 {
        unsafe { GetDpiForWindow(hwnd) }.max(96)
    } else {
        dpi
    }
}
pub(super) struct Layout {
    pub theme: crate::windows::native_theme::Theme,
    font: HFONT,
    dpi: u32,
    outer: [i32; 2],
}
impl Layout {
    pub fn new(hwnd: HWND) -> Result<Self, String> {
        let mut rect = RECT::default();
        unsafe { GetWindowRect(hwnd, &mut rect) }.map_err(|e| e.to_string())?;
        let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
        DPI.with(|value| value.set(dpi));
        Ok(Self {
            theme: crate::windows::native_theme::Theme::new()?,
            font: HFONT::default(),
            dpi,
            outer: [rect.right - rect.left, rect.bottom - rect.top],
        })
    }
    pub fn reconcile_dpi(&mut self, hwnd: HWND) {
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
        {
            self.change_dpi(hwnd, x, None);
        }
    }
    pub fn change_dpi(&mut self, hwnd: HWND, dpi: u32, suggested: Option<RECT>) {
        if dpi < 96 || self.dpi == dpi {
            return;
        }
        let mut rect = RECT::default();
        if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
            return;
        }
        let target = suggested.unwrap_or(rect);
        let width = scaled(self.outer[0], self.dpi, dpi);
        let height = scaled(self.outer[1], self.dpi, dpi);
        let previous = self.dpi;
        self.dpi = dpi;
        DPI.with(|value| value.set(dpi));
        if unsafe {
            SetWindowPos(
                hwnd,
                None,
                target.left,
                target.top,
                width,
                height,
                SWP_NOACTIVATE | SWP_NOZORDER,
            )
        }
        .is_err()
        {
            self.dpi = previous;
            DPI.with(|value| value.set(previous));
            return;
        }
        self.resize(hwnd);
    }
    pub fn resize(&mut self, hwnd: HWND) {
        PAINT.with(|paint| {
            paint.set(Some(Paint {
                background: self.theme.background,
                foreground: self.theme.foreground,
                brush: self.theme.brush,
            }))
        });
        unsafe {
            let scale = self.dpi as f32 / 96.0;
            if GetDpiForWindow(hwnd) == self.dpi {
                let mut rect = RECT::default();
                if GetWindowRect(hwnd, &mut rect).is_ok() {
                    self.outer = [rect.right - rect.left, rect.bottom - rect.top];
                }
            }
            let s = |v: i32| (v as f32 * scale).round() as i32;
            let mut bounds = RECT::default();
            let _ = GetClientRect(hwnd, &mut bounds);
            let width = ((bounds.right as f32 / scale) as i32 - 40).max(160);
            let height = ((bounds.bottom as f32 / scale) as i32 - 220).max(60);
            let font = CreateFontW(
                -s(14),
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
            let old = self.font;
            if !font.is_invalid() {
                self.font = font;
            }
            for (id, r) in [
                (106, [20, 16, width, 22]),
                (101, [20, 40, (width - 120).max(80), 32]),
                (109, [20 + width - 100, 40, 100, 32]),
                (1, [20, 82, 120, 32]),
                (2, [150, 82, 100, 32]),
                (104, [260, 82, 100, 32]),
                (110, [380, 82, 120, 32]),
                (108, [20, 122, width, 40]),
                (107, [20, 170, width, 22]),
                (105, [20, 196, width, height]),
            ] {
                if let Ok(child) = GetDlgItem(Some(hwnd), id) {
                    let _ = SetWindowPos(
                        child,
                        None,
                        s(r[0]),
                        s(r[1]),
                        s(r[2]),
                        s(r[3]),
                        SWP_NOACTIVATE | SWP_NOZORDER,
                    );
                    SendMessageW(
                        child,
                        WM_SETFONT,
                        Some(WPARAM(self.font.0 as usize)),
                        Some(LPARAM(1)),
                    );
                }
            }
            if old != self.font && !old.is_invalid() {
                let _ = DeleteObject(HGDIOBJ(old.0));
            }
            self.theme.apply(hwnd);
            let _ = InvalidateRect(Some(hwnd), None, true);
        }
    }
    pub fn paint(hwnd: HWND, message: u32, wparam: WPARAM) -> Option<LRESULT> {
        let theme = PAINT.with(std::cell::Cell::get)?;
        if !matches!(
            message,
            WM_ERASEBKGND | WM_CTLCOLOREDIT | WM_CTLCOLORSTATIC | WM_CTLCOLORBTN
        ) {
            return None;
        }
        unsafe {
            let dc = HDC(wparam.0 as *mut _);
            if message == WM_ERASEBKGND {
                let mut bounds = RECT::default();
                let _ = GetClientRect(hwnd, &mut bounds);
                FillRect(dc, &bounds, theme.brush);
                Some(LRESULT(1))
            } else {
                SetTextColor(dc, theme.foreground);
                SetBkColor(dc, theme.background);
                Some(LRESULT(theme.brush.0 as isize))
            }
        }
    }
}
impl Drop for Layout {
    fn drop(&mut self) {
        PAINT.with(|paint| paint.set(None));
        unsafe {
            if !self.font.is_invalid() {
                let _ = DeleteObject(HGDIOBJ(self.font.0));
            }
        }
    }
}

fn scaled(value: i32, previous: u32, next: u32) -> i32 {
    ((i64::from(value) * i64::from(next) + i64::from(previous) / 2) / i64::from(previous)) as i32
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn logical_window_size_survives_scale_changes_and_user_resize() {
        let mut size = [680, 500];
        let mut dpi = 96;
        for next in [144, 192, 168, 96] {
            size = size.map(|value| scaled(value, dpi, next));
            dpi = next;
            assert_eq!(size, [680 * dpi as i32 / 96, 500 * dpi as i32 / 96]);
        }
        assert_eq!(scaled(900, 144, 192), 1200);
        assert_eq!(scaled(1200, 192, 144), 900);
    }
}
