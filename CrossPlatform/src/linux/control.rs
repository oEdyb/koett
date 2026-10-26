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

pub fn send(command: &str) -> Result<(), String> {
    let mut stream = UnixStream::connect(path()?).map_err(|e| e.to_string())?;
    stream
        .write_all(command.as_bytes())
        .map_err(|e| e.to_string())
}

pub fn listen(sender: Sender<TrayAction>) -> Result<(), String> {
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
            let mut bytes = [0; 32];
            let Ok(count) = stream.read(&mut bytes) else {
                continue;
            };
            let action = match std::str::from_utf8(&bytes[..count]).unwrap_or("").trim() {
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
