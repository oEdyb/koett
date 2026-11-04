use crate::settings::PasteShortcut;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

pub static WINDOW_ACTIVE: AtomicBool = AtomicBool::new(false);
/// Longest wait for the target to read a pasted contact sheet.
pub const IMAGE_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(1500);
/// Settle time after the image read, before the clipboard changes to text.
pub const AFTER_IMAGE_READ: std::time::Duration = std::time::Duration::from_millis(160);
/// wl-copy cannot report reads, so wait this long after an image paste.
const WL_COPY_IMAGE_WAIT: std::time::Duration = std::time::Duration::from_millis(700);

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
    let mut trace = DeliveryStep::new("text", "fallback", text.len());
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
    trace.post();
    let status = Command::new("wtype")
        .args(wtype_arguments(shortcut))
        .status()
        .map_err(|e| format!("Copied · automatic paste unavailable: {e}"))?;
    trace.finish(status.success());
    if status.success() {
        Ok(())
    } else {
        Err("Copied · paste manually (virtual keyboard unavailable)".into())
    }
}

/// Pastes the contact sheet with Ctrl+V; image paste never uses the terminal
/// text chord (terminal agents read the image themselves on Ctrl+V).
pub fn deliver_image(png: &[u8], target: Option<u32>) -> Result<(), String> {
    let mut trace = DeliveryStep::new("image", "fallback", png.len());
    super::clipboard_owner::start(super::clipboard_owner::Payload::image(png))?;
    if !target_unchanged(target) {
        return Err("target window changed".into());
    }
    trace.post();
    let status = Command::new("wtype")
        .args(wtype_arguments(PasteShortcut::CtrlV))
        .status()
        .map_err(|e| format!("automatic paste unavailable: {e}"))?;
    trace.finish(status.success());
    if !status.success() {
        return Err("virtual keyboard unavailable".into());
    }
    std::thread::sleep(WL_COPY_IMAGE_WAIT);
    Ok(())
}

pub fn deliver_sketch_text(
    text: &str,
    target: Option<u32>,
    shortcut: PasteShortcut,
) -> Result<(), String> {
    let mut trace = DeliveryStep::new("text", "fallback", text.len());
    super::clipboard_owner::replace(super::clipboard_owner::Payload::text(text))?;
    if !target_unchanged(target) {
        return Err("Copied · paste into your target application".into());
    }
    trace.post();
    let status = Command::new("wtype")
        .args(wtype_arguments(shortcut))
        .status()
        .map_err(|e| e.to_string())?;
    trace.finish(status.success());
    if status.success() {
        Ok(())
    } else {
        Err("virtual keyboard unavailable".into())
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

/// The person pressed Paste just now, so focus cannot have moved since.
pub const PASTE_NOW: u32 = u32::MAX;

/// Only Niri can report the focused window here. Elsewhere the target can't be
/// checked, and a wrong-app paste is worse than a clipboard-only result.
pub fn capture_target() -> Option<u32> {
    if WINDOW_ACTIVE.load(Ordering::Acquire) {
        return None;
    }
    target_on_wayland(std::env::var_os("NIRI_SOCKET").is_some(), focused_window)
}

fn target_on_wayland(can_check_focus: bool, focused: impl FnOnce() -> Option<u32>) -> Option<u32> {
    if can_check_focus { focused() } else { None }
}

pub fn target_unchanged(target: Option<u32>) -> bool {
    if WINDOW_ACTIVE.load(Ordering::Acquire) {
        return false;
    }
    match target {
        Some(PASTE_NOW) => true,
        Some(id) => focused_window() == Some(id),
        None => false,
    }
}

#[cfg(test)]
mod target_tests {
    use super::*;

    #[test]
    fn a_desktop_without_a_focus_check_has_no_paste_target() {
        assert_eq!(target_on_wayland(false, || Some(7)), None);
    }

    #[test]
    fn niri_targets_the_focused_window() {
        assert_eq!(target_on_wayland(true, || Some(7)), Some(7));
    }

    #[test]
    fn no_target_means_clipboard_only() {
        assert!(!target_unchanged(None));
    }

    #[test]
    fn pressing_paste_skips_the_target_check() {
        assert!(target_unchanged(Some(PASTE_NOW)));
    }
}

/// Payload-free diagnostics: a post is logged once immediately before a paste key.
pub(super) struct DeliveryStep {
    id: u64,
    kind: &'static str,
    backend: &'static str,
    bytes: usize,
    outcome: &'static str,
}
impl DeliveryStep {
    pub fn new(kind: &'static str, backend: &'static str, bytes: usize) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let value = Self {
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            kind,
            backend,
            bytes,
            outcome: "aborted",
        };
        value.log("begin");
        value
    }
    fn log(&self, event: &str) {
        eprintln!(
            "delivery_step pid={} id={} kind={} backend={} attempt=1 bytes={} event={} outcome={}",
            std::process::id(),
            self.id,
            self.kind,
            self.backend,
            self.bytes,
            event,
            self.outcome
        );
    }
    pub fn post(&self) {
        self.log("paste_post");
    }
    pub fn finish(&mut self, ok: bool) {
        self.outcome = if ok { "ok" } else { "failed" };
    }
}
impl Drop for DeliveryStep {
    fn drop(&mut self) {
        self.log("end");
    }
}
