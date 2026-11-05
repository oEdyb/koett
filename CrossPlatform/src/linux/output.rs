use crate::settings::PasteShortcut;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

pub static WINDOW_ACTIVE: AtomicBool = AtomicBool::new(false);
/// Longest wait for the target to read a pasted contact sheet.
pub const IMAGE_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(1500);
/// Settle time after the image read, before the clipboard changes to text.
pub const AFTER_IMAGE_READ: std::time::Duration = std::time::Duration::from_millis(160);
/// wl-copy cannot report reads, so wait this long after an image paste.
const WL_COPY_IMAGE_WAIT: std::time::Duration = std::time::Duration::from_millis(700);

// Only Koett's own, active exclusive layers authorize an underlying-window capture.
static KEYBOARD_LAYERS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<usize, (bool, bool)>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));
pub(super) fn track_layer_keyboard(window: &gtk4::Window, requested: bool) {
    use gtk4::prelude::*;
    let token = window.as_ptr() as usize;
    let new = KEYBOARD_LAYERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(token, (requested, window.is_active()))
        .is_none();
    if new {
        window.connect_is_active_notify(move |window| {
            if let Some(value) = KEYBOARD_LAYERS
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get_mut(&token)
            {
                value.1 = window.is_active();
            }
        });
        window.connect_unrealize(move |_| {
            KEYBOARD_LAYERS
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&token);
        });
    }
}
fn own_layer_active() -> bool {
    KEYBOARD_LAYERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .values()
        .any(|(requested, active)| *requested && *active)
}
fn workspace_target(value: &serde_json::Value) -> Option<u32> {
    value
        .as_array()?
        .iter()
        .find(|w| w["is_focused"] == true)?
        .get("active_window_id")?
        .as_u64()
        .and_then(|id| u32::try_from(id).ok())
}
fn bounded_json(
    mut child: std::process::Child,
    wait: std::time::Duration,
) -> Option<serde_json::Value> {
    let deadline = std::time::Instant::now() + wait;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                let mut bytes = Vec::new();
                child
                    .stdout
                    .take()?
                    .take(64 * 1024)
                    .read_to_end(&mut bytes)
                    .ok()?;
                return serde_json::from_slice(&bytes).ok();
            }
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(3))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}
fn niri_json(request: &'static str) -> Option<serde_json::Value> {
    let child = Command::new("niri")
        .args(["msg", "-j", request])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let value = bounded_json(child, std::time::Duration::from_millis(200));
    if value.is_none() {
        let mut trace = DeliveryStep::new("focus_query", "niri", 0);
        trace.finish(false);
        trace.reason = "query_failed";
    }
    value
}
pub(super) fn keyboard_layout_group() -> Option<u32> {
    niri_json("keyboard-layouts")?["current_idx"]
        .as_u64()
        .and_then(|index| u32::try_from(index).ok())
}
fn underlying_window() -> Option<u32> {
    std::env::var_os("NIRI_SOCKET")?;
    workspace_target(&niri_json("workspaces")?)
}
pub fn focused_identity() -> Option<(u32, String)> {
    if std::env::var_os("NIRI_SOCKET").is_none() {
        return if super::gnome_focus::desktop() {
            super::gnome_focus::focused()
        } else {
            super::kwin_focus::focused()
        };
    }
    let value = niri_json("focused-window")?;
    if value["pid"].as_u64() == Some(u64::from(std::process::id()))
        || value["app_id"]
            .as_str()
            .is_some_and(|id| id == super::APP_ID)
    {
        return None;
    }
    Some((
        u32::try_from(value["id"].as_u64()?).ok()?,
        value["app_id"].as_str().unwrap_or_default().to_owned(),
    ))
}

pub fn focused_window() -> Option<u32> {
    focused_identity().map(|(id, _)| id)
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
    trace.prepare_paste(target)?;
    trace.send_fallback(shortcut)
}

/// Pastes the contact sheet with Ctrl+V; image paste never uses the terminal
/// text chord (terminal agents read the image themselves on Ctrl+V).
pub fn deliver_image(png: &[u8], target: Option<u32>) -> Result<(), String> {
    let mut trace = DeliveryStep::new("image", "fallback", png.len());
    super::clipboard_owner::start(super::clipboard_owner::Payload::image(png))?;
    trace.prepare_paste(target)?;
    let chord = trace.image_chord(target)?;
    trace.send_fallback(chord)?;
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
    trace.prepare_paste(target)?;
    trace.send_fallback(shortcut)
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

/// Explicit recovery Paste binds the current external window when the action executes.
pub const PASTE_NOW: u32 = u32::MAX;

fn captured_target(
    focused: Option<u32>,
    own_window: bool,
    own_layer: bool,
    underlying: Option<u32>,
) -> Option<u32> {
    if own_window {
        None
    } else {
        focused.or_else(|| own_layer.then_some(underlying).flatten())
    }
}
pub fn capture_target() -> Option<u32> {
    let own = WINDOW_ACTIVE.load(Ordering::Acquire);
    let focused = if own { None } else { focused_window() };
    let layer = own_layer_active();
    let underlying = if !own && focused.is_none() && layer {
        underlying_window()
    } else {
        None
    };
    let target = captured_target(focused, own, layer, underlying);
    let mut trace = DeliveryStep::new("capture", "focus", 0);
    trace.expected = target;
    trace.observed = focused;
    trace.finish(target.is_some());
    trace.reason = if own {
        "koett_active"
    } else if focused.is_none() && target.is_some() {
        "overlay_target"
    } else if target.is_some() {
        "target_captured"
    } else {
        "focus_unavailable"
    };
    // Only a known Koett keyboard layer can authorize the workspace fallback.
    target
}

const PASTE_WAIT: std::time::Duration = std::time::Duration::from_millis(1000);
const PASTE_SETTLE: std::time::Duration = std::time::Duration::from_millis(25);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Sample {
    focused: Option<u32>,
    own_window: bool,
    modifiers: Option<u8>,
}
fn post_refusal(expected: Option<u32>, sample: Sample) -> Option<&'static str> {
    if sample.own_window || expected.is_none() || sample.focused != expected {
        Some("focus_changed_before_post")
    } else if sample.modifiers.is_some_and(|mask| mask != 0) {
        Some("modifiers_held_before_post")
    } else {
        None
    }
}
#[derive(Debug, PartialEq, Eq)]
enum Decision {
    Wait(&'static str),
    Refuse(&'static str),
    Post,
}
struct PasteGate {
    expected: Option<u32>,
    explicit: bool,
    stable: Option<std::time::Duration>,
}
impl PasteGate {
    fn new(target: Option<u32>) -> Self {
        Self {
            expected: target.filter(|id| *id != PASTE_NOW),
            explicit: target == Some(PASTE_NOW),
            stable: None,
        }
    }
    fn observe(&mut self, sample: Sample, elapsed: std::time::Duration) -> Decision {
        if self.expected.is_none() && !self.explicit {
            return Decision::Refuse("target_missing");
        }
        let waiting = if sample.own_window {
            Some("koett_active")
        } else if sample.focused.is_none() {
            Some("focus_unavailable")
        } else {
            None
        };
        if let Some(reason) = waiting {
            self.stable = None;
            return Decision::Wait(reason);
        }
        if self.expected.is_none() {
            self.expected = sample.focused;
        }
        if sample.focused != self.expected {
            self.stable = None;
            return Decision::Refuse("focus_changed");
        }
        if sample.modifiers.is_some_and(|mask| mask != 0) {
            self.stable = None;
            return Decision::Wait("modifiers_held");
        }
        match self.stable {
            Some(since) if elapsed.saturating_sub(since) >= PASTE_SETTLE => Decision::Post,
            Some(_) => Decision::Wait("settling"),
            None => {
                self.stable = Some(elapsed);
                Decision::Wait("settling")
            }
        }
    }
}

fn modifier_mask(keys: &mut super::sketch::keys::Keyboards) -> Option<u8> {
    let value = keys.modifiers();
    let mask = u8::from(value.shift)
        | u8::from(value.control) << 1
        | u8::from(value.alt) << 2
        | u8::from(value.meta) << 3;
    if mask != 0 {
        Some(mask)
    } else if keys.is_empty() || keys.unreadable() != 0 {
        None
    } else {
        Some(0)
    }
}

/// Payload-free diagnostics: a post is logged once immediately before a paste key.
pub(super) struct DeliveryStep {
    id: u64,
    kind: &'static str,
    backend: &'static str,
    bytes: usize,
    outcome: &'static str,
    reason: &'static str,
    expected: Option<u32>,
    observed: Option<u32>,
    modifiers: Option<u8>,
    exit_code: Option<i32>,
    native_key: Option<u32>,
    native_mask: Option<u32>,
}
impl DeliveryStep {
    pub fn new(kind: &'static str, backend: &'static str, bytes: usize) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let value = Self {
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            kind,
            backend,
            bytes,
            outcome: "pending",
            reason: "clipboard_pending",
            expected: None,
            observed: None,
            modifiers: None,
            exit_code: None,
            native_key: None,
            native_mask: None,
        };
        value.log("begin");
        value
    }
    fn log(&self, event: &str) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let line = format!(
            "delivery_step pid={} id={} kind={} backend={} attempt=1 bytes={} event={} outcome={} reason={} expected={:?} observed={:?} modifiers={:?} exit_code={:?} native_key={:?} native_mask={:?} timestamp_ms={timestamp}",
            std::process::id(),
            self.id,
            self.kind,
            self.backend,
            self.bytes,
            event,
            self.outcome,
            self.reason,
            self.expected,
            self.observed,
            self.modifiers,
            self.exit_code,
            self.native_key,
            self.native_mask
        );
        eprintln!("{line}");
        persist_delivery_metadata(&line);
    }
    pub fn prepare_paste(&mut self, target: Option<u32>) -> Result<(), String> {
        let mut gate = PasteGate::new(target);
        // Read modifier state only, never key events; close devices after this bounded gate.
        let mut keyboards = super::sketch::keys::Keyboards::open().ok();
        let started = std::time::Instant::now();
        loop {
            let sample = Sample {
                focused: focused_window(),
                own_window: WINDOW_ACTIVE.load(Ordering::Acquire) || own_layer_active(),
                modifiers: keyboards.as_mut().and_then(modifier_mask),
            };
            self.expected = gate.expected;
            self.observed = sample.focused;
            self.modifiers = sample.modifiers;
            match gate.observe(sample, started.elapsed()) {
                Decision::Post => {
                    self.expected = gate.expected;
                    self.reason = "guard_passed";
                    self.log("guard_ready");
                    return Ok(());
                }
                Decision::Refuse(reason) => {
                    self.reason = reason;
                    self.outcome = "refused";
                    self.log("guard_refused");
                    return Err("Not pasted · saved. Use Paste or Copy".into());
                }
                Decision::Wait(reason) => {
                    self.reason = reason;
                    if started.elapsed() >= PASTE_WAIT {
                        self.outcome = "refused";
                        self.log("guard_timeout");
                        return Err("Not pasted · saved. Use Paste or Copy".into());
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
    }
    pub fn image_chord(&mut self, target: Option<u32>) -> Result<PasteShortcut, String> {
        super::image_paste::for_target(target).inspect_err(|_| {
            self.reason = "image_chord_focus";
            self.outcome = "refused";
        })
    }
    pub fn post(&mut self) -> Result<(), String> {
        self.observed = focused_window();
        self.modifiers = super::sketch::keys::Keyboards::open()
            .ok()
            .as_mut()
            .and_then(modifier_mask);
        let sample = Sample {
            focused: self.observed,
            own_window: WINDOW_ACTIVE.load(Ordering::Acquire) || own_layer_active(),
            modifiers: self.modifiers,
        };
        if let Some(reason) = post_refusal(self.expected, sample) {
            self.reason = reason;
            self.outcome = "refused";
            self.log("guard_refused");
            return Err("Not pasted · saved. Use Paste or Copy".into());
        }
        self.log("paste_post");
        Ok(())
    }
    fn send_fallback(&mut self, shortcut: PasteShortcut) -> Result<(), String> {
        if std::env::var_os("NIRI_SOCKET").is_some() {
            self.reason = "native_keyboard_setup";
            let result = super::native_keyboard::paste(shortcut, |key, mask| {
                self.native_key = Some(key);
                self.native_mask = Some(mask);
                self.post()
            });
            if self.outcome != "refused" {
                self.finish(result.is_ok());
                self.reason = if result.is_ok() {
                    "key_posted_native"
                } else {
                    "native_keyboard_error"
                };
            }
            return result;
        }
        self.post()?;
        match Command::new("wtype")
            .args(wtype_arguments(shortcut))
            .status()
        {
            Ok(status) => {
                self.exit_code = status.code();
                self.finish(status.success());
                if status.success() {
                    self.reason = "key_posted";
                    Ok(())
                } else {
                    self.reason = "wtype_exit";
                    Err("Not pasted · saved. Virtual keyboard failed".into())
                }
            }
            Err(error) => {
                self.finish(false);
                self.reason = "wtype_spawn";
                Err(format!(
                    "Not pasted · saved. Could not run virtual keyboard: {error}"
                ))
            }
        }
    }
    pub fn finish(&mut self, ok: bool) {
        self.outcome = if ok { "ok" } else { "failed" };
        if !ok {
            self.reason = "backend_error";
        }
    }
}
impl Drop for DeliveryStep {
    fn drop(&mut self) {
        if self.outcome == "pending" {
            self.outcome = "aborted";
        }
        self.log("end");
    }
}

fn persist_delivery_metadata(line: &str) {
    #[cfg(test)]
    if std::env::var_os("AGENT_LAB_RUNTIME").is_none() {
        return;
    }
    if let Some(base) = directories::BaseDirs::new() {
        let folder = base
            .state_dir()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| base.home_dir().join(".local/state"))
            .join("koett");
        let _ = write_delivery_metadata(&folder, line);
    }
}
fn write_delivery_metadata(folder: &std::path::Path, line: &str) -> std::io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = LOCK
        .lock()
        .map_err(|_| std::io::Error::other("delivery log lock"))?;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(folder)?;
    let path = folder.join("delivery.log");
    if std::fs::symlink_metadata(&path).is_ok_and(|meta| meta.len() > 256 * 1024) {
        std::fs::rename(&path, folder.join("delivery.previous.log"))?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    writeln!(file, "{line}")
}

#[cfg(test)]
mod target_tests {
    use super::*;
    use std::time::Duration;
    fn sample(id: Option<u32>, modifiers: Option<u8>) -> Sample {
        Sample {
            focused: id,
            own_window: false,
            modifiers,
        }
    }
    #[test]
    fn focus_and_modifiers_are_checked_again_after_keyboard_setup() {
        assert_eq!(post_refusal(Some(42), sample(Some(42), Some(0))), None);
        assert_eq!(post_refusal(Some(42), sample(Some(42), None)), None);
        assert_eq!(
            post_refusal(Some(42), sample(Some(43), Some(0))),
            Some("focus_changed_before_post")
        );
        for mask in [1, 2, 4, 8] {
            assert_eq!(
                post_refusal(Some(42), sample(Some(42), Some(mask))),
                Some("modifiers_held_before_post")
            );
        }
    }
    #[test]
    fn missing_focus_during_overlay_release_waits_then_posts_to_original_window() {
        let mut gate = PasteGate::new(Some(42));
        assert_eq!(
            gate.observe(sample(None, Some(0)), Duration::ZERO),
            Decision::Wait("focus_unavailable")
        );
        assert_eq!(
            gate.observe(sample(Some(42), Some(0)), Duration::from_millis(40)),
            Decision::Wait("settling")
        );
        assert_eq!(
            gate.observe(sample(Some(42), Some(0)), Duration::from_millis(80)),
            Decision::Post
        );
    }
    #[test]
    fn transient_koett_focus_never_becomes_the_paste_target() {
        let mut gate = PasteGate::new(Some(42));
        let mut own = sample(None, Some(0));
        own.own_window = true;
        assert_eq!(
            gate.observe(own, Duration::ZERO),
            Decision::Wait("koett_active")
        );
        assert_eq!(
            gate.observe(sample(Some(42), Some(0)), Duration::from_millis(30)),
            Decision::Wait("settling")
        );
        assert_eq!(
            gate.observe(sample(Some(42), Some(0)), Duration::from_millis(60)),
            Decision::Post
        );
    }
    #[test]
    fn different_window_or_remapped_id_is_never_guessed_safe() {
        let mut gate = PasteGate::new(Some(42));
        assert_eq!(
            gate.observe(sample(Some(43), Some(0)), Duration::ZERO),
            Decision::Refuse("focus_changed")
        );
        let mut missing = PasteGate::new(None);
        assert_eq!(
            missing.observe(sample(Some(42), Some(0)), Duration::ZERO),
            Decision::Refuse("target_missing")
        );
    }
    #[test]
    fn each_held_modifier_waits_for_a_clean_stable_release() {
        for held in [1, 2, 4, 8, 15] {
            let mut gate = PasteGate::new(Some(42));
            assert_eq!(
                gate.observe(sample(Some(42), Some(held)), Duration::ZERO),
                Decision::Wait("modifiers_held")
            );
            assert_eq!(
                gate.observe(sample(Some(42), Some(0)), Duration::from_millis(50)),
                Decision::Wait("settling")
            );
            assert_eq!(
                gate.observe(sample(Some(42), Some(0)), Duration::from_millis(80)),
                Decision::Post
            );
        }
    }
    #[test]
    fn explicit_recovery_binds_once_and_unavailable_keyboard_access_is_not_a_new_permission_gate() {
        let mut gate = PasteGate::new(Some(PASTE_NOW));
        assert_eq!(
            gate.observe(sample(Some(7), None), Duration::ZERO),
            Decision::Wait("settling")
        );
        assert_eq!(
            gate.observe(sample(Some(8), None), Duration::from_millis(50)),
            Decision::Refuse("focus_changed")
        );
        let mut gate = PasteGate::new(Some(7));
        gate.observe(sample(Some(7), None), Duration::ZERO);
        assert_eq!(
            gate.observe(sample(Some(7), None), Duration::from_millis(50)),
            Decision::Post
        );
    }
    #[test]
    fn capture_uses_underlying_workspace_only_for_our_active_keyboard_layer() {
        assert_eq!(captured_target(None, false, false, Some(42)), None);
        assert_eq!(captured_target(None, false, true, Some(42)), Some(42));
        assert_eq!(captured_target(Some(7), false, true, Some(42)), Some(7));
        assert_eq!(captured_target(None, true, true, Some(42)), None);
        assert_eq!(
            workspace_target(
                &serde_json::json!([{ "is_focused":false,"active_window_id":7},{"is_focused":true,"active_window_id":42}])
            ),
            Some(42)
        );
        assert_eq!(
            workspace_target(&serde_json::json!([{ "is_focused":true,"active_window_id":null}])),
            None
        );
    }
    #[test]
    fn a_stuck_niri_query_is_killed_and_reaped() {
        let child = Command::new("python3")
            .args(["-c", "import time; time.sleep(2)"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let pid = child.id();
        let start = std::time::Instant::now();
        assert!(bounded_json(child, Duration::from_millis(30)).is_none());
        assert!(start.elapsed() < Duration::from_millis(500));
        assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
    }
    #[test]
    fn metadata_log_is_private_bounded_and_rotates() {
        use std::os::unix::fs::PermissionsExt;
        let root = directories::BaseDirs::new()
            .unwrap()
            .cache_dir()
            .join(format!("koett-log-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let line = "delivery_step kind=text bytes=37 reason=focus_changed expected=Some(42) observed=Some(43)";
        write_delivery_metadata(&root, line).unwrap();
        assert_eq!(
            std::fs::metadata(root.join("delivery.log"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(&root).unwrap().permissions().mode() & 0o777,
            0o700
        );
        std::fs::write(root.join("delivery.log"), vec![b'x'; 256 * 1024 + 1]).unwrap();
        write_delivery_metadata(&root, line).unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("delivery.log")).unwrap(),
            format!("{line}\n")
        );
        assert!(root.join("delivery.previous.log").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
