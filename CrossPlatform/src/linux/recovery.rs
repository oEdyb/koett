use crate::{history::recovery, paths};
use std::io::Write;
use std::process::{Command, Stdio};

/// All Paste actions use controller delivery; `operation` only copies.
pub fn presses_paste(command: &str) -> bool {
    command == "paste-last"
}

pub fn last_text() -> Result<String, String> {
    recovery::last().ok_or_else(|| "Last Transcript is unavailable".to_string())
}

#[cfg(test)]
#[test]
fn paste_last_presses_paste_and_copy_last_does_not() {
    assert!(presses_paste("paste-last"));
    assert!(!presses_paste("copy-last"));
}

pub fn operation(command: &str) -> Result<serde_json::Value, String> {
    match command {
        "recovery-status" => {
            return Ok(
                serde_json::json!({"ok":true,"unsaved_count":recovery::pending_count(),"last_available":recovery::last().is_some(),"last_source":recovery::last_source()}),
            );
        }
        "retry-unsaved" => recovery::retry()?,
        "copy-last" | "copy-unsaved" => {
            let text = if command == "copy-unsaved" {
                recovery::unsaved()?
            } else {
                recovery::last().ok_or("Last Transcript is unavailable")?
            };
            {
                let (program, args): (&str, &[&str]) =
                    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
                        ("wl-copy", &["--type", "text/plain;charset=utf-8"])
                    } else {
                        ("xclip", &["-selection", "clipboard"])
                    };
                let mut child = Command::new(program)
                    .args(args)
                    .stdin(Stdio::piped())
                    .spawn()
                    .map_err(|e| e.to_string())?;
                child
                    .stdin
                    .take()
                    .ok_or("Clipboard input unavailable")?
                    .write_all(text.as_bytes())
                    .map_err(|e| e.to_string())?;
                if !child.wait().map_err(|e| e.to_string())?.success() {
                    return Err("Clipboard write failed".into());
                }
            }
        }
        "media-transcripts" | "failed-transcripts" | "failed-recordings" => {
            let path = match command {
                "media-transcripts" => paths::media_transcripts_directory()?,
                "failed-transcripts" => paths::failed_transcripts_directory()?,
                _ => paths::failed_recordings_directory()?,
            };
            std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
            use gtk4::gio::prelude::FileExt as _;
            let uri = gtk4::gio::File::for_path(path).uri();
            gtk4::gio::AppInfo::launch_default_for_uri(&uri, None::<&gtk4::gio::AppLaunchContext>)
                .map_err(|error| format!("Cannot open transcript folder: {error}"))?;
        }
        _ => return Err("Unknown recovery operation".into()),
    }
    Ok(serde_json::json!({"ok":true,"unsaved_count":recovery::pending_count()}))
}

/// CLI requests retain the target observed before entering the controller queue.
/// They must never use the direct pill click's PASTE_NOW bypass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PasteRequest {
    pub target: Option<u32>,
    expires: std::time::Instant,
    reply: super::menu_preferences::Reply,
}
impl PasteRequest {
    pub fn expired(&self) -> bool {
        std::time::Instant::now() >= self.expires
    }
    pub fn finish(self, result: Result<(), String>) {
        let value = match result {
            Ok(()) => {
                serde_json::json!({"ok":true,"status":"paste_posted","receipt":"key_posted_only"})
            }
            Err(message) => paste_error(&message),
        };
        let _ = self.reply.0.send(value);
    }
}
fn paste_error(message: &str) -> serde_json::Value {
    serde_json::json!({"ok":false,"error":{"code":"recovery_operation_failed","message":message,"retryable":true}})
}
pub(super) fn request_paste(
    sender: &std::sync::mpsc::Sender<super::TrayAction>,
) -> serde_json::Value {
    let target = capture_target();
    let (tx, rx) = std::sync::mpsc::channel();
    let request = PasteRequest {
        target,
        expires: std::time::Instant::now() + std::time::Duration::from_secs(3),
        reply: super::menu_preferences::Reply(std::sync::Arc::new(tx)),
    };
    if sender
        .send(super::TrayAction::RecoveryPaste(request))
        .is_err()
    {
        return paste_error("Koett is stopping");
    }
    rx.recv_timeout(std::time::Duration::from_secs(9))
        .unwrap_or_else(|_| paste_error("No paste response; inspect the target before retrying"))
}
fn capture_target() -> Option<u32> {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return super::output::capture_target();
    }
    if super::output::WINDOW_ACTIVE.load(std::sync::atomic::Ordering::Acquire) {
        return None;
    }
    use x11rb::protocol::xproto::ConnectionExt;
    let (connection, _) = x11rb::connect(None).ok()?;
    let focus = connection.get_input_focus().ok()?.reply().ok()?.focus;
    (focus > 1).then_some(focus)
}
#[cfg(test)]
mod paste_tests {
    use super::*;
    #[test]
    fn queued_paste_retains_target_and_claims_only_key_posting() {
        let (tx, rx) = std::sync::mpsc::channel();
        let request = PasteRequest {
            target: Some(42),
            expires: std::time::Instant::now() + std::time::Duration::from_secs(3),
            reply: super::super::menu_preferences::Reply(std::sync::Arc::new(tx)),
        };
        assert!(!request.expired());
        assert_eq!(request.target, Some(42));
        request.finish(Ok(()));
        let value = rx.recv().unwrap();
        assert_eq!(value["status"], "paste_posted");
        assert_eq!(value["receipt"], "key_posted_only");
    }
    #[test]
    fn expired_and_failed_pastes_return_actionable_errors() {
        let (tx, rx) = std::sync::mpsc::channel();
        let request = PasteRequest {
            target: None,
            expires: std::time::Instant::now(),
            reply: super::super::menu_preferences::Reply(std::sync::Arc::new(tx)),
        };
        assert!(request.expired());
        request.finish(Err("Copied · paste into your target application".into()));
        let value = rx.recv().unwrap();
        assert_eq!(value["ok"], false);
        assert_eq!(
            value["error"]["message"],
            "Copied · paste into your target application"
        );
    }
}
