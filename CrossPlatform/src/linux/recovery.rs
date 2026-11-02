use crate::{history::recovery, paths};
use std::io::Write;
use std::process::{Command, Stdio};

pub fn operation(command: &str) -> Result<serde_json::Value, String> {
    match command {
        "recovery-status" => {
            return Ok(
                serde_json::json!({"ok":true,"unsaved_count":recovery::pending_count(),"last_available":recovery::last().is_some(),"last_source":recovery::last_source()}),
            );
        }
        "retry-unsaved" => recovery::retry()?,
        "copy-last" | "copy-unsaved" | "paste-last" => {
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
            if command == "paste-last" {
                return Err("Copied · paste into your target application".into());
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
