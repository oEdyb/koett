use super::*;
use crate::settings::PasteShortcut;
use std::collections::BTreeSet;

struct Menu(HMENU);
impl Drop for Menu {
    fn drop(&mut self) {
        let _ = unsafe { DestroyMenu(self.0) };
    }
}
fn append(menu: HMENU, label: &str, id: usize, flags: MENU_ITEM_FLAGS) -> Result<(), String> {
    let label = label.encode_utf16().chain([0]).collect::<Vec<_>>();
    unsafe { AppendMenuW(menu, flags, id, PCWSTR(label.as_ptr())) }.map_err(|e| e.to_string())
}
fn select(owner: HWND, anchor: HWND, menu: HMENU) -> Result<usize, String> {
    let mut rect = RECT::default();
    unsafe { GetWindowRect(anchor, &mut rect) }.map_err(|e| e.to_string())?;
    Ok(unsafe {
        TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            rect.left,
            rect.bottom,
            None,
            owner,
            None,
        )
    }
    .0 as usize)
}
enum Choice {
    Default,
    Shortcut(PasteShortcut),
}
fn choose(
    owner: HWND,
    anchor: HWND,
    label: &str,
    current: Option<PasteShortcut>,
    allow_default: bool,
) -> Result<Option<Choice>, String> {
    let menu = Menu(unsafe { CreatePopupMenu() }.map_err(|e| e.to_string())?);
    append(menu.0, label, 0, MF_STRING | MF_DISABLED)?;
    append(menu.0, "", 0, MF_SEPARATOR)?;
    if allow_default {
        append(
            menu.0,
            "Use default",
            3,
            MF_STRING
                | if current.is_none() {
                    MF_CHECKED
                } else {
                    MENU_ITEM_FLAGS::default()
                },
        )?;
    }
    for (index, (name, chord)) in [
        ("Ctrl+V\t", PasteShortcut::CtrlV),
        ("Ctrl+Shift+V\t", PasteShortcut::CtrlShiftV),
    ]
    .into_iter()
    .enumerate()
    {
        append(
            menu.0,
            name,
            index + 1,
            MF_STRING
                | if current == Some(chord) {
                    MF_CHECKED
                } else {
                    MENU_ITEM_FLAGS::default()
                },
        )?;
    }
    match select(owner, anchor, menu.0)? {
        0 => Ok(None),
        1 => Ok(Some(Choice::Shortcut(PasteShortcut::CtrlV))),
        2 => Ok(Some(Choice::Shortcut(PasteShortcut::CtrlShiftV))),
        3 if allow_default => Ok(Some(Choice::Default)),
        _ => Err("Unknown paste shortcut".into()),
    }
}
fn apps(settings: &Settings) -> BTreeSet<String> {
    unsafe extern "system" fn visit(hwnd: HWND, data: LPARAM) -> windows::core::BOOL {
        let (count, apps) = unsafe { &mut *(data.0 as *mut (usize, BTreeSet<String>)) };
        *count += 1;
        if *count > 256 {
            return false.into();
        }
        let mut pid = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
        }
        if pid != std::process::id()
            && unsafe { IsWindowVisible(hwnd) }.as_bool()
            && let Some(key) = crate::windows::paste_chord::process_key(pid)
        {
            apps.insert(key);
        }
        true.into()
    }
    let mut data = (
        0_usize,
        settings
            .image_paste_shortcuts
            .keys()
            .filter(|key| key.starts_with("windows:") && key.as_str() != "windows:*")
            .cloned()
            .collect::<BTreeSet<_>>(),
    );
    let _ = unsafe {
        EnumWindows(
            Some(visit),
            LPARAM((&mut data as *mut (usize, BTreeSet<String>)) as isize),
        )
    };
    data.1
}
pub(super) fn show(owner: HWND, anchor: HWND) -> Result<(), String> {
    let settings = Settings::load()?;
    let menu = Menu(unsafe { CreatePopupMenu() }.map_err(|e| e.to_string())?);
    append(menu.0, "Text", 100, MF_STRING)?;
    let mut keys = vec!["windows:*".to_owned()];
    keys.extend(apps(&settings));
    let labels = keys
        .iter()
        .enumerate()
        .map(|(index, key)| {
            if index == 0 {
                "Images — default".to_owned()
            } else {
                format!(
                    "Images — {}",
                    key.trim_start_matches("windows:").trim_end_matches(".exe")
                )
            }
        })
        .collect::<Vec<_>>();
    for (index, label) in labels.iter().enumerate() {
        append(menu.0, label, 200 + index, MF_STRING)?;
    }
    let selected = select(owner, anchor, menu.0)?;
    if selected == 0 {
        return Ok(());
    }
    if selected == 100 {
        if let Some(Choice::Shortcut(chord)) =
            choose(owner, anchor, "Text", Some(settings.paste_shortcut), false)?
        {
            Settings::update(|current| current.paste_shortcut = chord)?;
        }
    } else {
        let position = selected.checked_sub(200).ok_or("Invalid paste shortcut")?;
        let key = keys.get(position).ok_or("Unknown image paste app")?.clone();
        let inherited = position != 0;
        let current = settings.image_paste_shortcuts.get(&key).copied();
        let current = if inherited {
            current
        } else {
            Some(current.unwrap_or_default())
        };
        if let Some(choice) = choose(owner, anchor, &labels[position], current, inherited)? {
            Settings::update(|current| match choice {
                Choice::Default => {
                    current.image_paste_shortcuts.remove(&key);
                }
                Choice::Shortcut(chord) => {
                    current.image_paste_shortcuts.insert(key, chord);
                }
            })?;
        }
    }
    Ok(())
}
