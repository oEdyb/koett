//! Native dictionary and literal-replacement editing. No decoder is loaded here.
use super::*;
use crate::dictionary;
const KIND: usize = 40;
const TERM: usize = 41;
const FORMS: usize = 42;
const ACCEPT: usize = 43;
const REMOVE: usize = 44;
const IMPORT: usize = 45;
const EXPORT_WORDS: usize = 46;
const WORD_STATUS: usize = 47;
const TERM_LABEL: usize = 48;
const FORMS_LABEL: usize = 49;
impl Window {
    pub(super) fn create_words(&mut self) -> Result<(), String> {
        self.mode = self.control(
            w!("COMBOBOX"),
            "Words",
            WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
            KIND,
            [24, 24, 320, 120],
        )?;
        for label in ["Dictionary", "Replacements"] {
            let text = super::super::wide(label);
            unsafe {
                SendMessageW(
                    self.mode,
                    CB_ADDSTRING,
                    None,
                    Some(LPARAM(text.as_ptr() as isize)),
                );
            }
        }
        unsafe {
            SendMessageW(self.mode, CB_SETCURSEL, Some(WPARAM(0)), None);
        }
        self.list = self.control(
            w!("LISTBOX"),
            "Entries",
            WS_TABSTOP | WS_BORDER | WS_VSCROLL | WINDOW_STYLE(LBS_NOTIFY as u32),
            LIST,
            [24, 76, 320, 460],
        )?;
        self.control(
            w!("STATIC"),
            "Term",
            Default::default(),
            TERM_LABEL,
            [368, 76, 520, 24],
        )?;
        self.search = self.control(
            w!("EDIT"),
            "",
            WS_TABSTOP | WS_BORDER | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
            TERM,
            [368, 108, 520, 32],
        )?;
        self.control(
            w!("STATIC"),
            "Sounds like (comma-separated)",
            Default::default(),
            FORMS_LABEL,
            [368, 156, 520, 24],
        )?;
        self.detail = self.control(
            w!("EDIT"),
            "",
            WS_TABSTOP
                | WS_BORDER
                | WS_VSCROLL
                | WINDOW_STYLE(ES_MULTILINE as u32 | ES_AUTOVSCROLL as u32),
            FORMS,
            [368, 188, 520, 88],
        )?;
        self.control(
            w!("BUTTON"),
            "Save entry",
            WS_TABSTOP,
            ACCEPT,
            [368, 300, 160, 32],
        )?;
        self.control(
            w!("BUTTON"),
            "Remove entry",
            WS_TABSTOP,
            REMOVE,
            [544, 300, 160, 32],
        )?;
        self.control(
            w!("BUTTON"),
            "Import…",
            WS_TABSTOP,
            IMPORT,
            [24, 560, 140, 32],
        )?;
        self.control(
            w!("BUTTON"),
            "Export…",
            WS_TABSTOP,
            EXPORT_WORDS,
            [180, 560, 140, 32],
        )?;
        self.control(
            w!("STATIC"),
            "",
            Default::default(),
            WORD_STATUS,
            [368, 356, 520, 144],
        )?;
        self.reload_words()
    }
    fn replacements(&self) -> bool {
        unsafe { SendMessageW(self.mode, CB_GETCURSEL, None, None) }.0 == 1
    }
    fn reload_words(&mut self) -> Result<(), String> {
        let settings = Settings::load()?;
        self.word_entries = if self.replacements() {
            settings
                .replacements
                .into_iter()
                .map(|r| (r.phrase, r.replacement))
                .collect()
        } else {
            settings
                .dictionary
                .into_iter()
                .map(|e| (e.term, e.sounds_like.join(", ")))
                .collect()
        };
        unsafe {
            SendMessageW(self.list, LB_RESETCONTENT, None, None);
        }
        for (term, _) in &self.word_entries {
            let label = super::super::wide(term);
            unsafe {
                SendMessageW(
                    self.list,
                    LB_ADDSTRING,
                    None,
                    Some(LPARAM(label.as_ptr() as isize)),
                );
            }
        }
        set(self.search, "");
        set(self.detail, "");
        let replace = self.replacements();
        for (id, label) in [
            (TERM_LABEL, if replace { "Phrase" } else { "Term" }),
            (
                FORMS_LABEL,
                if replace {
                    "Replace with"
                } else {
                    "Sounds like (comma-separated)"
                },
            ),
        ] {
            if let Ok(h) = unsafe { GetDlgItem(Some(self.hwnd), id as i32) } {
                set(h, label);
            }
        }
        for id in [IMPORT, EXPORT_WORDS] {
            if let Ok(h) = unsafe { GetDlgItem(Some(self.hwnd), id as i32) } {
                unsafe {
                    let _ = windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow(h, !replace);
                }
            }
        }
        Ok(())
    }
    fn words_action(&mut self, id: usize, notification: u16) -> Result<(), String> {
        match id {
            KIND if notification == CBN_SELCHANGE as u16 => self.reload_words()?,
            LIST if notification == LBN_SELCHANGE as u16 => {
                let i = unsafe { SendMessageW(self.list, LB_GETCURSEL, None, None) }.0;
                if let Some((term, forms)) = usize::try_from(i)
                    .ok()
                    .and_then(|i| self.word_entries.get(i))
                {
                    set(self.search, term);
                    set(self.detail, forms);
                }
            }
            ACCEPT => {
                let term = read(self.search).trim().to_owned();
                let forms = read(self.detail);
                if term.is_empty() {
                    return Err("Enter a term or phrase".into());
                }
                let mut edit = Settings::edit()?;
                if self.replacements() {
                    if term.chars().count() > 4096 || forms.len() > 65536 {
                        return Err("Replacement is too long".into());
                    }
                    if let Some(rule) = edit
                        .settings
                        .replacements
                        .iter_mut()
                        .find(|r| r.phrase == term)
                    {
                        rule.replacement = forms;
                    } else {
                        edit.settings
                            .replacements
                            .push(crate::replacements::Replacement {
                                phrase: term,
                                replacement: forms,
                            });
                    }
                } else {
                    let entry = dictionary::DictionaryEntry {
                        term,
                        sounds_like: forms
                            .split(',')
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(str::to_owned)
                            .collect(),
                    };
                    // Saving an exact term explicitly replaces its forms; imports merge.
                    edit.settings.dictionary.retain(|e| e.term != entry.term);
                    edit.settings.dictionary.push(entry);
                    edit.settings.dictionary =
                        dictionary::normalize(std::mem::take(&mut edit.settings.dictionary))?;
                }
                edit.commit()?;
                drop(edit);
                self.reload_words()?;
                return self.words_status("Saved");
            }
            REMOVE => {
                let i = unsafe { SendMessageW(self.list, LB_GETCURSEL, None, None) }.0;
                let term = usize::try_from(i)
                    .ok()
                    .and_then(|i| self.word_entries.get(i))
                    .map(|e| e.0.clone())
                    .ok_or("Select an entry to remove")?;
                let mut edit = Settings::edit()?;
                if self.replacements() {
                    edit.settings.replacements.retain(|r| r.phrase != term);
                } else {
                    edit.settings.dictionary.retain(|e| e.term != term);
                }
                edit.commit()?;
                drop(edit);
                self.reload_words()?;
                return self.words_status("Removed");
            }
            IMPORT if !self.replacements() => {
                if let Some(path) = import_path(self.hwnd)? {
                    let entries = dictionary::read_import(&path)?;
                    let count = entries.len();
                    // Import requires explicit acceptance before modifying settings.
                    let prompt =
                        super::super::wide(&format!("Add or merge {count} dictionary entries?"));
                    if unsafe {
                        MessageBoxW(
                            Some(self.hwnd),
                            PCWSTR(prompt.as_ptr()),
                            w!("Import dictionary"),
                            MB_OKCANCEL,
                        )
                    } != IDOK
                    {
                        return Ok(());
                    }
                    let mut edit = Settings::edit()?;
                    edit.settings.dictionary =
                        dictionary::merge(&edit.settings.dictionary, entries)?;
                    edit.commit()?;
                    drop(edit);
                    self.reload_words()?;
                    return self.words_status("Imported");
                }
            }
            EXPORT_WORDS if !self.replacements() => {
                if let Some(path) = export_path_named(self.hwnd, "Dictionary.txt")? {
                    write_export(&path, &dictionary::to_text(&Settings::load()?.dictionary))?;
                    return self.words_status("Exported");
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn words_status(&self, text: &str) -> Result<(), String> {
        if let Ok(h) = unsafe { GetDlgItem(Some(self.hwnd), WORD_STATUS as i32) } {
            set(h, text);
        }
        Ok(())
    }
    pub(super) fn words_command(&mut self, id: usize, notification: u16) {
        if let Err(error) = self.words_action(id, notification) {
            let _ = self.words_status(&error);
        }
    }
    pub(super) fn words_layout(&self) {
        for (child, [x, y, w, h]) in &self.controls {
            unsafe {
                let _ = MoveWindow(
                    *child,
                    self.scale(*x),
                    self.scale(*y),
                    self.scale(*w),
                    self.scale(*h),
                    true,
                );
            }
        }
    }
}
fn import_path(hwnd: HWND) -> Result<Option<std::path::PathBuf>, String> {
    let mut file = [0u16; 32768];
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: hwnd,
        lpstrFile: windows::core::PWSTR(file.as_mut_ptr()),
        nMaxFile: file.len() as u32,
        lpstrFilter: w!("Dictionary files\0*.txt\0All files\0*.*\0\0"),
        Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
        ..Default::default()
    };
    if unsafe { GetOpenFileNameW(&mut dialog) }.as_bool() {
        let end = file.iter().position(|v| *v == 0).unwrap_or(file.len());
        return Ok(Some(std::path::PathBuf::from(String::from_utf16_lossy(
            &file[..end],
        ))));
    }
    let error = unsafe { CommDlgExtendedError() }.0;
    if error != 0 {
        return Err(format!("Could not choose dictionary file: {error}"));
    }
    Ok(None)
}
