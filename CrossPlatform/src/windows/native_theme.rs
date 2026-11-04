use windows::Win32::{
    Foundation::*,
    Graphics::{Dwm::*, Gdi::*},
    UI::{Accessibility::*, WindowsAndMessaging::*},
};

pub(super) struct Theme {
    pub background: COLORREF,
    pub foreground: COLORREF,
    pub brush: HBRUSH,
    light: bool,
}
impl Theme {
    pub fn new() -> Result<Self, String> {
        let mut contrast = HIGHCONTRASTW {
            cbSize: std::mem::size_of::<HIGHCONTRASTW>() as u32,
            ..Default::default()
        };
        let high = unsafe {
            SystemParametersInfoW(
                SPI_GETHIGHCONTRAST,
                contrast.cbSize,
                Some((&mut contrast as *mut HIGHCONTRASTW).cast()),
                Default::default(),
            )
        }
        .is_ok()
            && (contrast.dwFlags & HCF_HIGHCONTRASTON).0 != 0;
        let light = super::pill::light_theme();
        let (background, foreground) = if high {
            unsafe {
                (
                    COLORREF(GetSysColor(COLOR_WINDOW)),
                    COLORREF(GetSysColor(COLOR_WINDOWTEXT)),
                )
            }
        } else if light {
            (COLORREF(0x00f9fafa), COLORREF(0x0017191c))
        } else {
            (COLORREF(0x00161718), COLORREF(0x00f9fafa))
        };
        let brush = unsafe { CreateSolidBrush(background) };
        if brush.is_invalid() {
            return Err("Could not create Windows window colors".into());
        }
        Ok(Self {
            background,
            foreground,
            brush,
            light,
        })
    }
    pub fn apply(&self, hwnd: HWND) {
        let dark = windows::core::BOOL::from(!self.light);
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                (&dark as *const windows::core::BOOL).cast(),
                std::mem::size_of_val(&dark) as u32,
            );
        }
    }
}
impl Drop for Theme {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(HGDIOBJ(self.brush.0));
        }
    }
}
