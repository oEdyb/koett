//! Native controls route requests to the existing engine; only it commits changes.
use super::*;
use crate::{
    formatting::{Context, Formatting, Structure, Style},
    model::LocalModel,
};
use std::sync::{LazyLock, Mutex};
const MODEL: usize = 60;
const SWITCH: usize = 61;
const MODEL_STATUS: usize = 62;
const ENABLE: usize = 63;
const STYLE: usize = 64;
const STRUCTURE: usize = 65;
const CONTEXT: usize = 66;
const APPLY: usize = 67;
const FORMAT_STATUS: usize = 68;
const CANCEL: usize = 69;
#[derive(Default, Clone)]
struct Progress {
    revision: u64,
    model_busy: bool,
    format_busy: bool,
    model: String,
    format: String,
}
static PROGRESS: LazyLock<Mutex<Progress>> = LazyLock::new(|| Mutex::new(Progress::default()));
fn change(action: impl FnOnce(&mut Progress)) {
    if let Ok(mut p) = PROGRESS.lock() {
        action(&mut p);
        p.revision = p.revision.wrapping_add(1);
    }
}
pub(in crate::windows) fn model_status(message: String, busy: bool) {
    change(|p| {
        p.model = message;
        p.model_busy = busy;
    });
}
pub(in crate::windows) fn model_message(message: String) {
    change(|p| p.model = message);
}
pub(in crate::windows) fn formatting_status(message: String) {
    change(|p| p.format = message);
}
pub(in crate::windows) fn formatting_busy(busy: bool) {
    change(|p| p.format_busy = busy);
}
pub(in crate::windows) fn model_finished() {
    change(|p| p.model_busy = false);
}
pub(in crate::windows) fn notice(message: &str) {
    if let Some(error) = message.strip_prefix("@model-notice:") {
        model_status(error.to_owned(), false);
    } else if PROGRESS.lock().map(|p| p.format_busy).unwrap_or(false)
        || message.starts_with("S1-mini")
        || message.starts_with("Raw active:")
    {
        formatting_status(message.to_owned());
    }
}
pub(in crate::windows) fn busy() -> bool {
    PROGRESS
        .lock()
        .map(|p| p.model_busy || p.format_busy)
        .unwrap_or(true)
}
pub(in crate::windows) fn encode_formatting(f: Formatting) -> usize {
    let style = match f.style {
        Style::Casual => 0,
        Style::SemiCasual => 1,
        Style::SemiFormal => 2,
        Style::Formal => 3,
    };
    usize::from(f.enabled)
        | (style << 1)
        | (usize::from(f.structure == Structure::Lists) << 3)
        | (usize::from(f.context == Context::Email) << 4)
}
pub(in crate::windows) fn decode_formatting(bits: usize) -> Option<Formatting> {
    if bits > 31 {
        return None;
    }
    Some(Formatting {
        enabled: bits & 1 != 0,
        style: [
            Style::Casual,
            Style::SemiCasual,
            Style::SemiFormal,
            Style::Formal,
        ][(bits >> 1) & 3],
        structure: if bits & 8 != 0 {
            Structure::Lists
        } else {
            Structure::Prose
        },
        context: if bits & 16 != 0 {
            Context::Email
        } else {
            Context::General
        },
    })
}
pub(super) struct Controls {
    owner: usize,
    model: HWND,
    enable: HWND,
    style: HWND,
    structure: HWND,
    context: HWND,
    revision: Option<u64>,
    window: HWND,
}
fn combo(
    window: &mut Window,
    id: usize,
    labels: &[&str],
    bounds: [i32; 4],
) -> Result<HWND, String> {
    let h = window.control(
        w!("COMBOBOX"),
        "",
        WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
        id,
        bounds,
    )?;
    for label in labels {
        let wide = super::super::wide(label);
        unsafe {
            SendMessageW(h, CB_ADDSTRING, None, Some(LPARAM(wide.as_ptr() as isize)));
        }
    }
    Ok(h)
}
fn select(h: HWND, index: usize) {
    unsafe {
        SendMessageW(h, CB_SETCURSEL, Some(WPARAM(index)), None);
    }
}
fn index(h: HWND) -> Result<usize, String> {
    usize::try_from(unsafe { SendMessageW(h, CB_GETCURSEL, None, None) }.0)
        .map_err(|_| "Choose a value".into())
}
impl Controls {
    pub(super) fn new(window: &mut Window, settings: &Settings) -> Result<Self, String> {
        window.control(
            w!("STATIC"),
            "Speech model",
            Default::default(),
            0,
            [24, 20, 488, 24],
        )?;
        let model = combo(
            window,
            MODEL,
            &LocalModel::ALL.map(LocalModel::label),
            [24, 48, 336, 180],
        )?;
        window.control(
            w!("BUTTON"),
            "Switch model",
            WS_TABSTOP,
            SWITCH,
            [376, 48, 136, 32],
        )?;
        window.control(
            w!("STATIC"),
            "",
            Default::default(),
            MODEL_STATUS,
            [24, 88, 488, 48],
        )?;
        let enable = window.control(
            w!("BUTTON"),
            "Format dictation (S1-mini)",
            WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
            ENABLE,
            [24, 144, 488, 32],
        )?;
        window.theme.apply_checkbox(enable);
        window.control(
            w!("STATIC"),
            "Style",
            Default::default(),
            0,
            [24, 184, 232, 24],
        )?;
        window.control(
            w!("STATIC"),
            "Structure",
            Default::default(),
            0,
            [280, 184, 232, 24],
        )?;
        let style = combo(
            window,
            STYLE,
            &["Casual", "Semi-casual", "Semi-formal", "Formal"],
            [24, 212, 232, 160],
        )?;
        let structure = combo(window, STRUCTURE, &["Prose", "Lists"], [280, 212, 232, 120])?;
        window.control(
            w!("STATIC"),
            "Context",
            Default::default(),
            0,
            [24, 252, 232, 24],
        )?;
        let context = combo(window, CONTEXT, &["General", "Email"], [24, 280, 232, 120])?;
        window.control(
            w!("BUTTON"),
            "Apply formatting",
            WS_TABSTOP,
            APPLY,
            [280, 280, 232, 32],
        )?;
        window.control(
            w!("STATIC"),
            "",
            Default::default(),
            FORMAT_STATUS,
            [24, 320, 488, 64],
        )?;
        window.control(
            w!("BUTTON"),
            "Cancel setup",
            WS_TABSTOP,
            CANCEL,
            [376, 400, 136, 32],
        )?;
        let mut result = Self {
            owner: window.owner,
            window: window.hwnd,
            model,
            enable,
            style,
            structure,
            context,
            revision: None,
        };
        result.load(settings);
        result.poll();
        Ok(result)
    }
    fn load(&mut self, s: &Settings) {
        select(
            self.model,
            LocalModel::ALL
                .iter()
                .position(|m| *m == s.selected_model)
                .unwrap_or(0),
        );
        let bits = encode_formatting(s.formatting);
        unsafe {
            SendMessageW(self.enable, BM_SETCHECK, Some(WPARAM(bits & 1)), None);
        }
        select(self.style, (bits >> 1) & 3);
        select(self.structure, (bits >> 3) & 1);
        select(self.context, (bits >> 4) & 1);
    }
    pub(super) fn command(&mut self, id: usize) -> bool {
        let request = match id {
            SWITCH => index(self.model).map(|i| (super::super::WM_CHANGE_MODEL, i)),
            APPLY => (|| {
                let style = index(self.style)?;
                let structure = index(self.structure)?;
                let context = index(self.context)?;
                let enabled = unsafe { SendMessageW(self.enable, BM_GETCHECK, None, None) }.0 != 0;
                Ok((
                    super::super::WM_CHANGE_FORMATTING,
                    usize::from(enabled) | (style << 1) | (structure << 3) | (context << 4),
                ))
            })(),
            CANCEL => Ok((super::super::WM_CANCEL_SETUP, 0)),
            _ => return false,
        };
        match request {
            Ok((message, value)) => unsafe {
                let _ = PostMessageW(
                    Some(HWND(self.owner as *mut _)),
                    message,
                    WPARAM(value),
                    LPARAM(0),
                );
            },
            Err(e) => formatting_status(e),
        }
        true
    }
    pub(super) fn poll(&mut self) {
        let Ok(p) = PROGRESS.lock().map(|p| p.clone()) else {
            return;
        };
        if self.revision == Some(p.revision) {
            return;
        }
        self.revision = Some(p.revision);
        for (id, text) in [
            (MODEL_STATUS, p.model.as_str()),
            (FORMAT_STATUS, p.format.as_str()),
        ] {
            if let Ok(h) = unsafe { GetDlgItem(Some(self.window), id as i32) } {
                set(h, text);
            }
        }
        let busy = p.model_busy || p.format_busy;
        for id in [MODEL, SWITCH, ENABLE, STYLE, STRUCTURE, CONTEXT, APPLY] {
            if let Ok(h) = unsafe { GetDlgItem(Some(self.window), id as i32) } {
                unsafe {
                    let _ = windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow(h, !busy);
                }
            }
        }
        if let Ok(h) = unsafe { GetDlgItem(Some(self.window), CANCEL as i32) } {
            unsafe {
                let _ = windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow(h, busy);
            }
        }
        // Reflect actual committed settings after a successful operation or rollback.
        if !busy && let Ok(s) = Settings::load() {
            self.load(&s);
        }
    }
    pub(super) fn theme(&self, theme: &super::super::native_theme::Theme) {
        theme.apply_checkbox(self.enable);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formatting_commands_round_trip_all_options_and_reject_unknown_bits() {
        for bits in 0..32 {
            assert_eq!(encode_formatting(decode_formatting(bits).unwrap()), bits);
        }
        assert!(decode_formatting(32).is_none());
        assert!(decode_formatting(usize::MAX).is_none());
    }
}
