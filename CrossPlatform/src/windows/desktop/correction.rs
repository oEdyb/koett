//! Corrections never rewrite history or adopt a dictionary suggestion implicitly.
use super::*;
use crate::corrections::{Corrections, Proposal, Suggestion};
const SAVE_CORRECTION: usize = 80;
const ADD: usize = 81;
const DISMISS: usize = 82;
const NOTICE: usize = 83;
const WARNING: usize = 84;
pub(super) struct Editor {
    record: crate::history::HistoryRecord,
    corrections: Corrections,
    learned: Vec<Suggestion>,
    proposals: Vec<Proposal>,
    pending: Option<mpsc::Receiver<Vec<Suggestion>>>,
}
impl Editor {
    pub(super) fn new(record: crate::history::HistoryRecord) -> Result<Self, String> {
        Ok(Self {
            record,
            corrections: Corrections::load()?,
            learned: vec![],
            proposals: vec![],
            pending: None,
        })
    }
    pub(super) fn create(&mut self, w: &mut Window) -> Result<(), String> {
        w.control(
            w!("STATIC"),
            "What Koett wrote",
            Default::default(),
            0,
            [24, 24, 412, 24],
        )?;
        w.search = w.control(
            w!("EDIT"),
            &self.record.text,
            WS_TABSTOP
                | WS_BORDER
                | WS_VSCROLL
                | WINDOW_STYLE(ES_MULTILINE as u32 | ES_READONLY as u32 | ES_AUTOVSCROLL as u32),
            85,
            [24, 56, 412, 248],
        )?;
        w.control(
            w!("STATIC"),
            "What you said",
            Default::default(),
            0,
            [464, 24, 440, 24],
        )?;
        let corrected = self
            .corrections
            .get(&self.record.id)
            .map(|c| c.corrected.as_str())
            .unwrap_or(&self.record.text);
        w.detail = w.control(
            w!("EDIT"),
            corrected,
            WS_TABSTOP
                | WS_BORDER
                | WS_VSCROLL
                | WINDOW_STYLE(ES_MULTILINE as u32 | ES_AUTOVSCROLL as u32),
            86,
            [464, 56, 440, 248],
        )?;
        w.control(
            w!("BUTTON"),
            "Save correction",
            WS_TABSTOP,
            SAVE_CORRECTION,
            [24, 328, 176, 32],
        )?;
        w.control(
            w!("STATIC"),
            "",
            Default::default(),
            NOTICE,
            [224, 328, 680, 48],
        )?;
        w.control(
            w!("STATIC"),
            "Dictionary suggestions",
            Default::default(),
            0,
            [24, 392, 880, 24],
        )?;
        w.list = w.control(
            w!("LISTBOX"),
            "Dictionary suggestions",
            WS_TABSTOP | WS_BORDER | WS_VSCROLL | WINDOW_STYLE(LBS_NOTIFY as u32),
            LIST,
            [24, 424, 880, 112],
        )?;
        w.control(
            w!("BUTTON"),
            "Add to dictionary",
            WS_TABSTOP,
            ADD,
            [24, 552, 192, 32],
        )?;
        w.control(
            w!("BUTTON"),
            "Dismiss",
            WS_TABSTOP,
            DISMISS,
            [232, 552, 144, 32],
        )?;
        w.control(
            w!("STATIC"),
            "",
            Default::default(),
            WARNING,
            [24, 600, 880, 48],
        )?;
        self.learn()?;
        self.refresh(w)
    }
    fn message(w: &Window, id: usize, text: &str) {
        if let Ok(h) = unsafe { GetDlgItem(Some(w.hwnd), id as i32) } {
            set(h, text);
        }
    }
    fn learn(&mut self) -> Result<(), String> {
        let dictionary = Settings::load()?.dictionary;
        let corrections = self.corrections.clone();
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(corrections.learned(&dictionary));
        });
        Ok(())
    }
    fn refresh(&mut self, w: &Window) -> Result<(), String> {
        self.proposals = self
            .corrections
            .filter(self.learned.clone(), &Settings::load()?.dictionary);
        unsafe {
            SendMessageW(w.list, LB_RESETCONTENT, None, None);
        }
        for p in &self.proposals {
            let text = super::super::wide(&format!(
                "{} → {} ({} corrections)",
                p.suggestion.heard, p.suggestion.term, p.suggestion.count
            ));
            unsafe {
                SendMessageW(
                    w.list,
                    LB_ADDSTRING,
                    None,
                    Some(LPARAM(text.as_ptr() as isize)),
                );
            }
        }
        if !self.proposals.is_empty() {
            unsafe {
                SendMessageW(w.list, LB_SETCURSEL, Some(WPARAM(0)), None);
            }
        }
        self.selection(w);
        Ok(())
    }
    fn selected(&self, w: &Window) -> Option<Proposal> {
        let i = unsafe { SendMessageW(w.list, LB_GETCURSEL, None, None) }.0;
        usize::try_from(i)
            .ok()
            .and_then(|i| self.proposals.get(i))
            .cloned()
    }
    fn selection(&self, w: &Window) {
        let selected = self.selected(w);
        for id in [ADD, DISMISS] {
            if let Ok(h) = unsafe { GetDlgItem(Some(w.hwnd), id as i32) } {
                unsafe {
                    let _ = windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow(
                        h,
                        selected.is_some() && self.pending.is_none(),
                    );
                }
                if id == ADD {
                    set(
                        h,
                        if selected.as_ref().is_some_and(|p| p.everyday) {
                            "Add anyway"
                        } else {
                            "Add to dictionary"
                        },
                    );
                }
            }
        }
        Self::message(
            w,
            WARNING,
            &selected
                .filter(|p| p.everyday)
                .map(|p| {
                    format!(
                        "“{}” is an everyday word. Adding it changes every occurrence you dictate.",
                        p.suggestion.heard
                    )
                })
                .unwrap_or_default(),
        );
    }
    pub(super) fn poll(&mut self, w: &Window) {
        if let Some(learned) = self.pending.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.pending = None;
            self.learned = learned;
            if let Err(e) = self.refresh(w) {
                Self::message(w, NOTICE, &e);
            }
        }
    }
    fn action(&mut self, w: &Window, id: usize, notification: u16) -> Result<(), String> {
        match id {
            SAVE_CORRECTION => {
                let corrected = read(w.detail).replace("\r\n", "\n");
                if corrected.trim().is_empty() {
                    return Err("Enter the corrected transcript".into());
                }
                let mut next = Corrections::load()?;
                next.record(
                    &self.record.id,
                    &self.record.text,
                    &corrected,
                    (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64,
                );
                next.save()?;
                self.corrections = next;
                self.learn()?;
                self.selection(w);
                Self::message(w, NOTICE, "Correction saved. Dictionary unchanged.");
            }
            LIST if notification == LBN_SELCHANGE as u16 => self.selection(w),
            ADD if self.pending.is_none() => {
                let p = self.selected(w).ok_or("Select a suggestion")?;
                let mut edit = Settings::edit()?;
                edit.settings.dictionary = crate::dictionary::add_form(
                    &edit.settings.dictionary,
                    &p.suggestion.term,
                    &p.suggestion.heard,
                )?;
                edit.commit()?;
                drop(edit);
                self.refresh(w)?;
                Self::message(w, NOTICE, "Added to dictionary");
            }
            DISMISS if self.pending.is_none() => {
                let p = self.selected(w).ok_or("Select a suggestion")?;
                let mut next = Corrections::load()?;
                next.dismiss(&p.suggestion);
                next.save()?;
                self.corrections = next;
                self.refresh(w)?;
                Self::message(w, NOTICE, "Dismissed");
            }
            _ => {}
        }
        Ok(())
    }
    pub(super) fn command(&mut self, w: &Window, id: usize, notification: u16) {
        if let Err(error) = self.action(w, id, notification) {
            Self::message(w, NOTICE, &error);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn dictionary_and_corrections_match_the_swift_parity_fixture() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../Tests/KoettTests/Dictionary/Fixtures/parity.json"
        )))
        .unwrap();
        let dictionary: Vec<crate::dictionary::DictionaryEntry> =
            serde_json::from_value(fixture["input"]["dictionary"].clone()).unwrap();
        for (input, expected) in fixture["input"]["spell"]
            .as_array()
            .unwrap()
            .iter()
            .zip(fixture["expected"]["spell"].as_array().unwrap())
        {
            assert_eq!(
                crate::dictionary::apply(input.as_str().unwrap(), &dictionary),
                expected.as_str().unwrap()
            );
        }
        for (pair, expected) in fixture["input"]["adopt"]
            .as_array()
            .unwrap()
            .iter()
            .zip(fixture["expected"]["adopt"].as_array().unwrap())
        {
            assert_eq!(
                crate::dictionary::adopt_terms(
                    pair[0].as_str().unwrap(),
                    pair[1].as_str().unwrap(),
                    &dictionary
                ),
                expected.as_str().unwrap()
            );
        }
        let known: Vec<String> = dictionary.iter().map(|e| e.term.clone()).collect();
        for (pair, expected) in fixture["input"]["learn"]
            .as_array()
            .unwrap()
            .iter()
            .zip(fixture["expected"]["learn"].as_array().unwrap())
        {
            let learned = crate::corrections::learn(
                &[(pair[0].as_str().unwrap(), pair[1].as_str().unwrap())],
                &known,
            );
            let values: Vec<_> = learned
                .into_iter()
                .map(|s| serde_json::json!([s.heard, s.term, s.count]))
                .collect();
            assert_eq!(serde_json::Value::Array(values), *expected);
        }
    }
}
