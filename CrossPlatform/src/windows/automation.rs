use super::*;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, SMTO_ABORTIFHUNG, SMTO_BLOCK, SendMessageTimeoutW,
};
static MESSAGE: std::sync::LazyLock<u32> =
    std::sync::LazyLock::new(|| unsafe { RegisterWindowMessageW(w!("Koett.Automation.v1")) });
const ACTIONS: [&str; 14] = [
    "status",
    "start",
    "stop",
    "toggle",
    "pause",
    "resume",
    "cancel",
    "copy-last",
    "paste-last",
    "settings",
    "history",
    "dictionary",
    "media",
    "quit",
];
pub(super) fn failure(code: &str, fix: &str, message: String) -> String {
    println!(
        "{}",
        serde_json::json!({"ok":false,"code":code,"error":message,"fix":fix,"outputs":[],"warnings":[]})
    );
    message
}
pub fn run(arguments: &[String]) -> Result<(), String> {
    if arguments == ["help"] || arguments == ["--help"] || arguments == ["help", "--json"] {
        println!(
            "{}",
            serde_json::json!({"ok":true,"commands":ACTIONS,"usage":"koett-engine app <action>","outputs":[],"warnings":[]})
        );
        return Ok(());
    }
    let [action] = arguments else {
        return Err(failure(
            "invalid_arguments",
            "Run koett-engine app help --json",
            "app requires one action".into(),
        ));
    };
    let index = ACTIONS
        .iter()
        .position(|value| *value == action)
        .ok_or_else(|| {
            failure(
                "unknown_action",
                "Run koett-engine app help --json",
                format!("Unknown app action: {action}"),
            )
        })?;
    let owner = unsafe { FindWindowW(w!("KoettOwnerWindow"), None) }.map_err(|_| {
        failure(
            "not_running",
            "Start Koett.exe in this desktop session",
            "Koett is not running in this desktop session".into(),
        )
    })?;
    if *MESSAGE == 0 {
        return Err(failure(
            "command_unavailable",
            "Restart Koett",
            "Could not register app command".into(),
        ));
    }
    let expected = unsafe { GetForegroundWindow() };
    let mut response = 0;
    if unsafe {
        SendMessageTimeoutW(
            owner,
            *MESSAGE,
            windows::Win32::Foundation::WPARAM(index),
            windows::Win32::Foundation::LPARAM(expected.0 as isize),
            SMTO_ABORTIFHUNG | SMTO_BLOCK,
            1500,
            Some(&mut response),
        )
    }
    .0 == 0
    {
        return Err(failure(
            "acknowledgement_timeout",
            "Run koett-engine app status before retrying",
            "Koett did not acknowledge the command; it may have been applied".into(),
        ));
    }
    if response == 0 {
        return Err(failure(
            "action_unavailable",
            "Run koett-engine app status and check the foreground target",
            "Action unavailable in the current state".into(),
        ));
    }
    if index == 0 {
        let status = match response & 0xff {
            1 => "starting",
            2 => "ready",
            3 => "recording",
            4 => "transcribing",
            _ => "error",
        };
        println!(
            "{}",
            serde_json::json!({"ok":true,"status":status,"paused":response&0x100!=0,"media_active":response&0x200!=0,"unsaved_media":response&0x400!=0,"outputs":[],"warnings":[]})
        );
    } else {
        println!(
            "{}",
            serde_json::json!({"ok":true,"action":action,"acknowledged":true,"outputs":[],"warnings":[]})
        );
    }
    Ok(())
}
pub(super) fn dispatch(
    _hwnd: HWND,
    message: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> Option<windows::Win32::Foundation::LRESULT> {
    if *MESSAGE == 0 || message != *MESSAGE {
        return None;
    }
    let result = (|| {
        let cell = unsafe { APP.load(Ordering::Acquire).as_ref() }?;
        let mut app = cell.try_borrow_mut().ok()?;
        match wparam.0 {
            0 => {
                let state = match app.status {
                    AppStatus::Starting => 1,
                    AppStatus::Ready => 2,
                    AppStatus::Recording => 3,
                    AppStatus::Transcribing => 4,
                    AppStatus::Error(_) => 5,
                };
                return Some(
                    state
                        | if app.levels.as_ref().is_some_and(AudioLevels::paused) {
                            0x100
                        } else {
                            0
                        }
                        | if media_view::active() { 0x200 } else { 0 }
                        | if media_view::needs_copy() { 0x400 } else { 0 },
                );
            }
            1 if !app.capture_pending
                && !media_view::active()
                && matches!(app.status, AppStatus::Ready | AppStatus::Error(_)) =>
            {
                app.recording_intent(RecordingIntent::Start)
            }
            2 if app.status == AppStatus::Recording => app.recording_intent(RecordingIntent::Stop),
            3 if !media_view::active()
                && matches!(
                    app.status,
                    AppStatus::Ready | AppStatus::Recording | AppStatus::Error(_)
                ) =>
            {
                app.toggle()
            }
            4 | 5 if app.status == AppStatus::Recording => {
                app.levels.as_ref()?.set_paused(wparam.0 == 4);
                app.pill_state.paused = wparam.0 == 4;
            }
            6 => {
                app.cancel_current();
                app.update_overlay();
            }
            7 if !app.last_transcript.is_empty() => {
                app.copy_last().ok()?;
            }
            8 if app.can_repeat_paste()
                && app.last_target == foreground_paste_target()
                && !app.last_transcript.is_empty()
                && unsafe { GetForegroundWindow() }.0 as isize == lparam.0
                && foreground_paste_target().is_some() =>
            {
                app.repeat_paste()
            }
            9 => app.handle_menu(MENU_SETTINGS),
            10 => app.handle_menu(MENU_OPEN_HISTORY),
            11 => app.handle_menu(MENU_WORDS),
            12 => app.handle_menu(MENU_MEDIA),
            13 if !media_view::needs_copy() => app.request_quit(),
            _ => return None,
        }
        Some(1)
    })()
    .unwrap_or(0);
    Some(windows::Win32::Foundation::LRESULT(result))
}
