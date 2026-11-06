use super::*;
const LABELS: [&str; 3] = [
    "Recording sounds",
    "Include page details in pins",
    "Read selected text at pins",
];
struct Menu(HMENU);
impl Drop for Menu {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyMenu(self.0);
        }
    }
}
fn values(settings: &Settings) -> [bool; 3] {
    [
        settings.recording_sounds,
        settings.pin_page_details,
        settings.pin_selected_text,
    ]
}
fn toggle(settings: &mut Settings, selected: usize) {
    match selected {
        1 => settings.recording_sounds = !settings.recording_sounds,
        2 => settings.pin_page_details = !settings.pin_page_details,
        3 => settings.pin_selected_text = !settings.pin_selected_text,
        _ => {}
    }
}
pub(super) fn show(owner: HWND, anchor: HWND) -> Result<(), String> {
    let current = Settings::load()?;
    let menu = Menu(unsafe { CreatePopupMenu() }.map_err(|error| error.to_string())?);
    for (index, (label, enabled)) in LABELS.iter().zip(values(&current)).enumerate() {
        let label = super::super::wide(label);
        unsafe {
            AppendMenuW(
                menu.0,
                MF_STRING
                    | if enabled {
                        MF_CHECKED
                    } else {
                        MENU_ITEM_FLAGS::default()
                    },
                index + 1,
                PCWSTR(label.as_ptr()),
            )
        }
        .map_err(|error| error.to_string())?;
    }
    let mut rect = RECT::default();
    unsafe { GetWindowRect(anchor, &mut rect) }.map_err(|error| error.to_string())?;
    let selected = unsafe {
        TrackPopupMenu(
            menu.0,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            rect.left,
            rect.bottom,
            None,
            owner,
            None,
        )
    }
    .0 as usize;
    if (1..=3).contains(&selected) {
        Settings::update(|settings| toggle(settings, selected))?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn options_toggle_independently_and_cancel_changes_nothing() {
        let mut settings = Settings::default();
        let initial = values(&settings);
        toggle(&mut settings, 0);
        assert_eq!(values(&settings), initial);
        for selected in 1..=3 {
            toggle(&mut settings, selected);
            let mut expected = initial;
            expected[selected - 1] = !expected[selected - 1];
            assert_eq!(values(&settings), expected);
            toggle(&mut settings, selected);
            assert_eq!(values(&settings), initial);
        }
    }
}
