use windows::{
    Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{IDYES, MB_ICONERROR, MB_YESNO, MessageBoxW},
    },
    core::PCWSTR,
};

pub(super) fn present(owner: HWND, message: &str) {
    let Some(log) = crate::model::failure_log(message) else {
        if message.contains("\nModel log unavailable: ") {
            super::show_fatal_error(&format!("Model setup could not finish.\n{message}"));
        }
        return;
    };
    let text = super::wide(&format!(
        "Model setup could not finish.\nChoose the model again, or reopen Koett to retry.\n\nLog: {log}\n\nOpen the log?"
    ));
    let title = super::wide("Koett model setup");
    let answer = unsafe {
        MessageBoxW(
            Some(owner),
            PCWSTR(text.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_ICONERROR | MB_YESNO,
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
