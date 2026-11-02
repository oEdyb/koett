//! Olle's own words: terms Koett should always spell his way.
//!
//! Each entry is a written term plus optional "sounds like" forms the recognizer
//! tends to produce instead. Entries live in `Settings.dictionary` and travel as
//! plain text, one entry per line: `Term` or `Term = heard as, other form`.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

mod common;
mod correct;
mod merge;

pub use common::is_common;
pub use correct::apply;
pub use merge::adopt_terms;

pub const MAX_ENTRIES: usize = 2_000;
pub const MAX_TERM_CHARS: usize = 80;
pub const MAX_FORMS_PER_TERM: usize = 32;
pub const MAX_FORMS: usize = 4_000;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct DictionaryEntry {
    pub term: String,
    pub sounds_like: Vec<String>,
}

impl DictionaryEntry {
    pub fn new(term: &str, sounds_like: &[&str]) -> Self {
        Self {
            term: term.to_string(),
            sounds_like: sounds_like.iter().map(|form| form.to_string()).collect(),
        }
    }
}

/// Trim, drop empty forms and merge entries that repeat a term. Fails fast at
/// the size limits, so a huge import cannot stall the Settings window.
pub fn normalize(entries: Vec<DictionaryEntry>) -> Result<Vec<DictionaryEntry>, String> {
    let mut result: Vec<DictionaryEntry> = Vec::new();
    let mut positions: HashMap<String, usize> = HashMap::new();
    let mut seen: HashSet<(usize, String)> = HashSet::new();
    let mut forms = 0;
    for entry in entries {
        let term = clean(&entry.term);
        if term.is_empty() {
            if entry.sounds_like.iter().any(|form| !clean(form).is_empty()) {
                return Err("Each \"sounds like\" form needs a term.".into());
            }
            continue;
        }
        check_length(&term)?;
        let index = match positions.get(&term) {
            Some(&index) => index,
            None => {
                if result.len() == MAX_ENTRIES {
                    return Err(format!("The dictionary holds at most {MAX_ENTRIES} words."));
                }
                positions.insert(term.clone(), result.len());
                result.push(DictionaryEntry {
                    term,
                    sounds_like: Vec::new(),
                });
                result.len() - 1
            }
        };
        for form in entry.sounds_like.iter().map(|form| clean(form)) {
            let known = &mut result[index];
            if form.is_empty() || form == known.term || !seen.insert((index, form.clone())) {
                continue;
            }
            check_length(&form)?;
            if known.sounds_like.len() == MAX_FORMS_PER_TERM {
                return Err(format!(
                    "\"{}\" has more than {MAX_FORMS_PER_TERM} \"sounds like\" forms.",
                    known.term
                ));
            }
            forms += 1;
            if forms > MAX_FORMS {
                return Err(format!(
                    "The dictionary holds at most {MAX_FORMS} \"sounds like\" forms."
                ));
            }
            known.sounds_like.push(form);
        }
    }
    Ok(result)
}

fn check_length(value: &str) -> Result<(), String> {
    if value.chars().count() > MAX_TERM_CHARS {
        return Err(format!(
            "\"{value}\" is longer than {MAX_TERM_CHARS} characters."
        ));
    }
    Ok(())
}

/// Parse the plain-text exchange format. Blank lines and `#` comments are ignored.
pub fn parse_text(text: &str) -> Result<Vec<DictionaryEntry>, String> {
    let mut entries = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let line = line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (term, forms) = line.split_once('=').unwrap_or((line, ""));
        if clean(term).is_empty() {
            return Err(format!("Line {} has no term before \"=\".", number + 1));
        }
        entries.push(DictionaryEntry {
            term: term.to_string(),
            sounds_like: forms.split(',').map(str::to_string).collect(),
        });
    }
    normalize(entries)
}

pub fn to_text(entries: &[DictionaryEntry]) -> String {
    let mut text = String::from("# Koett dictionary: Term = sounds like, other form\n");
    for entry in entries {
        text.push_str(&entry.term);
        if !entry.sounds_like.is_empty() {
            text.push_str(" = ");
            text.push_str(&entry.sounds_like.join(", "));
        }
        text.push('\n');
    }
    text
}

/// Imported entries come after existing ones; a repeated term gains new forms.
pub fn merge(
    existing: &[DictionaryEntry],
    imported: Vec<DictionaryEntry>,
) -> Result<Vec<DictionaryEntry>, String> {
    normalize(existing.iter().cloned().chain(imported).collect())
}

/// The dictionary already writes `term` when Koett hears `heard`.
pub fn covers(entries: &[DictionaryEntry], term: &str, heard: &str) -> bool {
    let heard_key = correct::key(heard);
    entries.iter().any(|entry| {
        entry.term == term
            && (correct::key(&entry.term) == heard_key
                || entry
                    .sounds_like
                    .iter()
                    .any(|form| correct::key(form) == heard_key))
    })
}

/// Add `term` with `heard` as a sounds-like form (none for a spelling-only fix).
pub fn add_form(
    entries: &[DictionaryEntry],
    term: &str,
    heard: &str,
) -> Result<Vec<DictionaryEntry>, String> {
    let forms = if correct::key(heard) == correct::key(term) {
        Vec::new()
    } else {
        vec![heard.to_string()]
    };
    merge(
        entries,
        vec![DictionaryEntry {
            term: term.to_string(),
            sounds_like: forms,
        }],
    )
}

fn clean(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests;
