use windows::{
    Win32::UI::WindowsAndMessaging::{
        IDYES, MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MB_TOPMOST, MB_YESNO, MessageBoxW,
    },
    core::PCWSTR,
};

pub(super) fn present(message: &str) {
    if message.starts_with("@media-result:")
        || (!message.contains("\nModel log: ") && !message.contains("\nModel log unavailable: "))
    {
        return;
    }
    let message = message.to_owned();
    std::thread::spawn(move || show(&message));
}

fn show(message: &str) {
    let Some(log) = crate::model::failure_log(message) else {
        if message.contains("\nModel log unavailable: ") {
            let text = super::wide(&format!("Model setup could not finish.\n{message}"));
            unsafe {
                let _ = MessageBoxW(
                    None,
                    PCWSTR(text.as_ptr()),
                    windows::core::w!("Koett model setup"),
                    MB_ICONERROR | MB_OK | MB_SETFOREGROUND | MB_TOPMOST,
                );
            }
        }
        return;
    };
    let text = super::wide(&format!(
        "{}\nChoose the model again, or reopen Koett to retry.\n\nLog: {log}\n\nOpen the log?",
        message
            .lines()
            .next()
            .unwrap_or("Model setup could not finish.")
    ));
    let title = super::wide("Koett model setup");
    let answer = unsafe {
        MessageBoxW(
            None,
            PCWSTR(text.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_ICONERROR | MB_YESNO | MB_SETFOREGROUND | MB_TOPMOST,
        )
    };
    if answer == IDYES {
        let editor = std::env::var_os("WINDIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| "C:\\Windows".into())
            .join("System32\\notepad.exe");
        if let Err(error) = std::process::Command::new(editor).arg(log).spawn() {
            super::show_fatal_error(&format!(
                "Open {log} to read the model error; Windows could not open the log: {error}"
            ));
        }
    }
}
