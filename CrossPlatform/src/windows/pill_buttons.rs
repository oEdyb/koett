use super::pill_state::{Phase, State};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        UI::{Controls::DRAWITEMSTRUCT, HiDpi::GetDpiForWindow, WindowsAndMessaging::*},
    },
    core::{PCWSTR, w},
};

pub(super) struct Buttons(Vec<(usize, HWND, i32, i32)>);
impl Buttons {
    pub fn new(parent: HWND) -> Result<Self, String> {
        let mut controls = Vec::new();
        for (id, label, x, width) in [
            (101, "Cancel", 40, 60),
            (102, "Pause", 102, 72),
            (103, "History", 176, 60),
            (104, "Copy", 244, 51),
            (105, "Paste", 190, 50),
        ] {
            let label = super::wide(label);
            let child = unsafe {
                CreateWindowExW(
                    WS_EX_NOACTIVATE,
                    w!("BUTTON"),
                    PCWSTR(label.as_ptr()),
                    WS_CHILD | WINDOW_STYLE(BS_OWNERDRAW as u32),
                    0,
                    0,
                    1,
                    1,
                    Some(parent),
                    Some(HMENU(id as *mut _)),
                    None,
                    None,
                )
            }
            .map_err(|e| e.to_string())?;
            controls.push((id, child, x, width));
        }
        Ok(Self(controls))
    }
    pub fn sync(&self, parent: HWND, state: &State) {
        let dpi = unsafe { GetDpiForWindow(parent) }.max(96) as i32;
        let active = matches!(state.phase, Phase::Recording | Phase::Transcribing) && state.hovered;
        for (id, child, x, width) in &self.0 {
            let show = match id {
                101 | 103 => active,
                102 => active && matches!(state.phase, Phase::Recording),
                104 => matches!(state.phase, Phase::Done(_) | Phase::NotPasted),
                105 => matches!(state.phase, Phase::NotPasted),
                _ => false,
            };
            unsafe {
                if *id == 102 || *id == 104 {
                    let label = if *id == 102 {
                        if state.paused { "Resume" } else { "Pause" }
                    } else if state.copied {
                        "Copied"
                    } else {
                        "Copy"
                    };
                    let mut current = [0u16; 32];
                    let count = GetWindowTextW(*child, &mut current);
                    if String::from_utf16_lossy(&current[..count.max(0) as usize]) != label {
                        let label = super::wide(label);
                        let _ = SetWindowTextW(*child, PCWSTR(label.as_ptr()));
                    }
                }
                let _ = MoveWindow(
                    *child,
                    x * dpi / 96,
                    8 * dpi / 96,
                    width * dpi / 96,
                    32 * dpi / 96,
                    false,
                );
                if IsWindowVisible(*child).as_bool() != show {
                    let _ = ShowWindow(*child, if show { SW_SHOWNOACTIVATE } else { SW_HIDE });
                }
            }
        }
    }
}
pub(super) unsafe fn draw(item: &DRAWITEMSTRUCT) {
    let light = super::pill::light_theme();
    let background = if light {
        COLORREF(0x00ffffff)
    } else {
        COLORREF(0x00141313)
    };
    let foreground = if light {
        COLORREF(0x0017191c)
    } else {
        COLORREF(0x00ffffff)
    };
    unsafe {
        let brush = CreateSolidBrush(background);
        FillRect(item.hDC, &item.rcItem, brush);
        let _ = DeleteObject(HGDIOBJ(brush.0));
        let font = CreateFontW(
            -(12 * GetDpiForWindow(item.hwndItem).max(96) as i32 / 96),
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
        let previous = SelectObject(item.hDC, HGDIOBJ(font.0));
        SetBkMode(item.hDC, TRANSPARENT);
        SetTextColor(item.hDC, foreground);
        let mut label = [0u16; 32];
        let count = GetWindowTextW(item.hwndItem, &mut label);
        let mut bounds = item.rcItem;
        DrawTextW(
            item.hDC,
            &mut label[..count.max(0) as usize],
            &mut bounds,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
        SelectObject(item.hDC, previous);
        let _ = DeleteObject(HGDIOBJ(font.0));
    }
}
