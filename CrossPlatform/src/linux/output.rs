use crate::settings::PasteShortcut;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

pub static WINDOW_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn focused_window() -> Option<u32> {
    let output = Command::new("niri")
        .args(["msg", "-j", "focused-window"])
        .output()
        .ok()?;
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    if value["pid"].as_u64() == Some(u64::from(std::process::id())) {
        return None;
    }
    if value["app_id"]
        .as_str()
        .is_some_and(|id| id == super::APP_ID)
    {
        return None;
    }
    u32::try_from(value["id"].as_u64()?).ok()
}

pub fn deliver(text: &str, target: Option<u32>, shortcut: PasteShortcut) -> Result<(), String> {
    let mut copy = Command::new("wl-copy")
        .args(["--type", "text/plain;charset=utf-8"])
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Copy failed: {e}"))?;
    copy.stdin
        .take()
        .ok_or("Clipboard input unavailable")?
        .write_all(text.as_bytes())
        .map_err(|e| e.to_string())?;
    if !copy.wait().map_err(|e| e.to_string())?.success() {
        return Err("Copy failed; recover from History".into());
    }
    if !target_unchanged(target) {
        return Err("Copied · paste into your target application".into());
    }
    let status = Command::new("wtype")
        .args(wtype_arguments(shortcut))
        .status()
        .map_err(|e| format!("Copied · automatic paste unavailable: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("Copied · paste manually (virtual keyboard unavailable)".into())
    }
}

pub(super) fn paste_keysyms(shortcut: PasteShortcut) -> &'static [u32] {
    match shortcut {
        PasteShortcut::CtrlV => &[xkeysym::key::Control_L, xkeysym::key::v],
        PasteShortcut::CtrlShiftV => &[
            xkeysym::key::Control_L,
            xkeysym::key::Shift_L,
            xkeysym::key::v,
        ],
    }
}

fn wtype_arguments(shortcut: PasteShortcut) -> &'static [&'static str] {
    match shortcut {
        PasteShortcut::CtrlV => &["-M", "ctrl", "-P", "v", "-p", "v", "-m", "ctrl"],
        PasteShortcut::CtrlShiftV => &[
            "-M", "ctrl", "-M", "shift", "-P", "v", "-p", "v", "-m", "shift", "-m", "ctrl",
        ],
    }
}

pub fn capture_target() -> Option<u32> {
    if WINDOW_ACTIVE.load(Ordering::Acquire) {
        return None;
    }
    if std::env::var_os("NIRI_SOCKET").is_some() {
        focused_window()
    } else {
        Some(u32::MAX)
    }
}
pub fn target_unchanged(target: Option<u32>) -> bool {
    if WINDOW_ACTIVE.load(Ordering::Acquire) {
        return false;
    }
    match target {
        Some(id) if id == u32::MAX => true,
        Some(id) => focused_window() == Some(id),
        None => false,
    }
}
