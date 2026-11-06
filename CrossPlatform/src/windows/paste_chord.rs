use crate::settings::{PasteShortcut, Settings};
use std::os::windows::io::{FromRawHandle, OwnedHandle};
use windows::Win32::{
    Foundation::HANDLE,
    System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    },
};

pub(super) fn image(settings: &Settings, pid: u32) -> PasteShortcut {
    process_key(pid)
        .and_then(|key| settings.image_paste_shortcuts.get(&key).copied())
        .or_else(|| settings.image_paste_shortcuts.get("windows:*").copied())
        .unwrap_or_default()
}
pub(super) fn process_key(pid: u32) -> Option<String> {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let _owned = unsafe { OwnedHandle::from_raw_handle(handle.0) };
    let mut name = [0_u16; 32768];
    let mut length = name.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            HANDLE(handle.0),
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(name.as_mut_ptr()),
            &mut length,
        )
    }
    .ok()?;
    let name = String::from_utf16(&name[..length as usize]).ok()?;
    let name = name.rsplit('\\').next()?;
    (!name.is_empty()).then(|| format!("windows:{}", name.to_ascii_lowercase()))
}
