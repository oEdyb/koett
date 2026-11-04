#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "windows")]
fn main() {
    if let Err(error) = koett_engine::windows::run() {
        koett_engine::windows::show_fatal_error(&error);
    }
}

#[cfg(target_os = "linux")]
fn main() {
    if let Err(error) = koett_engine::linux::run() {
        if std::env::args().nth(1).is_some_and(|s| {
            s.starts_with("menu-")
                || s.starts_with("ask")
                || matches!(
                    s.as_str(),
                    "copy-last"
                        | "paste-last"
                        | "copy-unsaved"
                        | "retry-unsaved"
                        | "recovery-status"
                        | "media-transcripts"
                        | "failed-transcripts"
                        | "failed-recordings"
                        | "quit"
                )
        }) {
            println!(
                "{}",
                serde_json::json!({"ok": false, "error": {"code": if std::env::args().nth(1).is_some_and(|s| s.starts_with("ask")) { "ask_operation_failed" } else if std::env::args().nth(1).is_some_and(|s| s.starts_with("menu-")) { "menu_operation_failed" } else { "recovery_operation_failed" }, "message": error, "retryable": true}})
            );
            std::process::exit(1);
        }
        koett_engine::linux::show_fatal_error(&error);
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn main() {
    eprintln!("The Koett desktop shell supports Windows and Linux.");
}
