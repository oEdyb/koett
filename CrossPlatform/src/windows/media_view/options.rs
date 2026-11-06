use windows::{
    Win32::{Foundation::*, UI::WindowsAndMessaging::*},
    core::{PCWSTR, w},
};
pub(super) fn show(hwnd: HWND) {
    unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            return;
        };
        let shortcut = crate::settings::Settings::load()
            .map(|s| s.media_shortcut)
            .unwrap_or_default();
        let label = crate::windows::wide(&format!("Change media shortcut…\t{shortcut}"));
        let _ = AppendMenuW(menu, MF_STRING, 1, PCWSTR(label.as_ptr()));
        let _ = AppendMenuW(menu, MF_STRING, 2, w!("Media transcripts…"));
        let _ = AppendMenuW(menu, MF_STRING, 3, w!("Transcript recovery…"));
        let _ = AppendMenuW(
            menu,
            if super::needs_copy() {
                MF_STRING
            } else {
                MF_STRING | MF_DISABLED
            },
            4,
            w!("Discard unsaved transcript…"),
        );
        let mut point = POINT::default();
        let _ = GetCursorPos(&mut point);
        let selected = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            None,
            hwnd,
            None,
        )
        .0;
        let _ = DestroyMenu(menu);
        match selected {
            1 => {
                if let Ok(owner) = FindWindowW(w!("KoettOwnerWindow"), None) {
                    let _ = PostMessageW(
                        Some(owner),
                        WM_COMMAND,
                        WPARAM(crate::windows::MENU_CHANGE_MEDIA_SHORTCUT),
                        LPARAM(0),
                    );
                }
            }
            2 | 3 => {
                let directory = if selected == 3 {
                    crate::paths::failed_transcripts_directory()
                } else {
                    crate::paths::media_transcripts_directory()
                };
                if let Ok(path) = directory {
                    let _ = std::fs::create_dir_all(&path);
                    let path = crate::windows::wide(&path.to_string_lossy());
                    let result = windows::Win32::UI::Shell::ShellExecuteW(
                        Some(hwnd),
                        w!("open"),
                        PCWSTR(path.as_ptr()),
                        None,
                        None,
                        SW_SHOWNORMAL,
                    );
                    if result.0 as usize <= 32 {
                        super::local_notice("Could not open media transcripts");
                    }
                }
            }
            4 if MessageBoxW(
                Some(hwnd),
                w!("Discard the unsaved media transcript?"),
                w!("Koett"),
                MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
            ) == IDYES =>
            {
                super::discard();
            }
            _ => {}
        }
    }
}
