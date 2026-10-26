use super::TrayAction;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::mpsc::Sender;

fn path() -> Result<std::path::PathBuf, String> {
    let base = std::env::var_os("XDG_RUNTIME_DIR").ok_or("XDG_RUNTIME_DIR is unavailable")?;
    use std::os::unix::fs::MetadataExt;
    let directory = std::path::PathBuf::from(base);
    let metadata = std::fs::symlink_metadata(&directory).map_err(|e| e.to_string())?;
    let owner = std::fs::metadata("/proc/self")
        .map_err(|e| e.to_string())?
        .uid();
    if !metadata.is_dir() || metadata.uid() != owner || metadata.mode() & 0o077 != 0 {
        return Err("Control requires a private, owned XDG_RUNTIME_DIR".into());
    }
    let display = std::env::var("WAYLAND_DISPLAY")
        .or_else(|_| std::env::var("DISPLAY"))
        .map_err(|_| "Desktop session is unavailable")?;
    let session: String = display
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .take(32)
        .collect();
    if session.is_empty() {
        return Err("Desktop session identifier is invalid".into());
    }
    Ok(directory.join(format!("koett-{session}-control.sock")))
}

// Allow JSON escaping of the bounded 2 KiB token plus its small envelope.
const MAX_REQUEST: usize = 8192;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
enum ActivationRequest {
    Show { token: String },
    Settings { token: String },
}

fn valid_token(token: &str) -> bool {
    !token.is_empty() && token.len() <= 2048 && !token.chars().any(char::is_control)
}

fn encode(command: &str, token: Option<&str>) -> Result<Vec<u8>, String> {
    let request = match (command, token.filter(|token| valid_token(token))) {
        ("show", Some(token)) => Some(ActivationRequest::Show {
            token: token.into(),
        }),
        ("settings", Some(token)) => Some(ActivationRequest::Settings {
            token: token.into(),
        }),
        _ => None,
    };
    match request {
        Some(request) => serde_json::to_vec(&request).map_err(|e| e.to_string()),
        None => Ok(command.as_bytes().to_vec()),
    }
}

// Only explicit window requests carry activation authority. Bare requests
// remain best-effort: a Wayland compositor may refuse to focus an existing
// window without a fresh token from a launcher or user interaction.
pub fn send(command: &str) -> Result<(), String> {
    let mut stream = UnixStream::connect(path()?).map_err(|e| e.to_string())?;
    let token = if matches!(command, "show" | "settings") {
        startup_token()
    } else {
        None
    };
    let bytes = encode(command, token.as_deref())?;
    stream.write_all(&bytes).map_err(|e| e.to_string())
}

pub(super) fn startup_token() -> Option<String> {
    use gtk4::prelude::DisplayExt;
    if let Ok(token) = std::env::var("XDG_ACTIVATION_TOKEN") {
        return Some(token);
    }
    // GTK >=4.10 caches and unsets startup environment in a constructor,
    // before Rust main. Initialize GTK without creating a window, then read
    // the public display property. No window or activation is needed to retrieve it.
    // https://github.com/GNOME/gtk/blob/4.24.1/gdk/gdk.c
    gtk4::init().ok()?;
    gtk4::gdk::Display::default()?
        .startup_notification_id()
        .map(|id| id.to_string())
}

fn read_request(stream: &mut UnixStream) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    Read::by_ref(stream)
        .take(MAX_REQUEST as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() <= MAX_REQUEST).then_some(bytes)
}

pub fn listen(sender: Sender<TrayAction>, ui: Sender<super::window::Update>) -> Result<(), String> {
    let path = path()?;
    // The lifetime lock is already held. Only remove our stale socket, never arbitrary files.
    if let Ok(metadata) = std::fs::symlink_metadata(&path) {
        use std::os::unix::fs::FileTypeExt;
        if !metadata.file_type().is_socket() {
            return Err("Control path is not a socket".into());
        }
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    let listener = UnixListener::bind(&path).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        for connection in listener.incoming() {
            let Ok(mut stream) = connection else {
                break;
            };
            // Filesystem mode and the private, per-user runtime directory authenticate peers.
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(200)));
            let Some(bytes) = read_request(&mut stream) else {
                continue;
            };
            if let Ok(request) = serde_json::from_slice::<ActivationRequest>(&bytes) {
                let update = match request {
                    ActivationRequest::Show { token } if valid_token(&token) => {
                        super::window::Update::Show(token)
                    }
                    ActivationRequest::Settings { token } if valid_token(&token) => {
                        super::window::Update::Settings(token)
                    }
                    _ => continue,
                };
                if ui.send(update).is_err() {
                    break;
                }
                continue;
            }
            let action = match std::str::from_utf8(&bytes).unwrap_or("").trim() {
                "start" => TrayAction::Start,
                "stop" => TrayAction::Stop,
                "toggle" => TrayAction::Toggle,
                "show" => TrayAction::OpenTranscripts,
                "settings" => TrayAction::OpenSettings,
                "quit" => TrayAction::Quit,
                _ => continue,
            };
            if sender.send(action).is_err() {
                break;
            }
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_commands_keep_legacy_wire_bytes_with_activation_environment() {
        for command in ["start", "stop", "toggle", "quit"] {
            assert_eq!(
                encode(command, Some("fresh-token")).unwrap(),
                command.as_bytes()
            );
        }
        for command in ["show", "settings"] {
            assert_eq!(encode(command, None).unwrap(), command.as_bytes());
        }
    }

    #[test]
    fn activation_is_forwarded_only_for_window_requests_and_validated() {
        let token = "a-long-compositor-activation-token".repeat(8);
        for command in ["show", "settings"] {
            let bytes = encode(command, Some(&token)).unwrap();
            let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(value["command"], command);
            assert_eq!(value["token"], token);
            for invalid in ["", "bad\0token", "bad\ntoken", &"x".repeat(2049)] {
                assert_eq!(encode(command, Some(invalid)).unwrap(), command.as_bytes());
            }
        }
        assert!(
            serde_json::from_slice::<ActivationRequest>(br#"{"command":"start","token":"token"}"#)
                .is_err()
        );
    }

    #[test]
    fn socket_reads_complete_fragmented_tokens_and_bounds_input() {
        let payload = encode("settings", Some(&"token".repeat(50))).unwrap();
        let (mut reader, mut writer) = UnixStream::pair().unwrap();
        let expected = payload.clone();
        let write = std::thread::spawn(move || {
            for fragment in payload.chunks(7) {
                writer.write_all(fragment).unwrap();
            }
        });
        assert_eq!(read_request(&mut reader).unwrap(), expected);
        write.join().unwrap();
        let (mut reader, mut writer) = UnixStream::pair().unwrap();
        writer.write_all(&vec![b'x'; MAX_REQUEST + 1]).unwrap();
        drop(writer);
        assert!(read_request(&mut reader).is_none());
    }
}
