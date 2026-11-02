//! "Fix a word": corrections Olle makes to history entries, and the
//! dictionary suggestions learned from them.
//!
//! Corrections live in their own file, keyed by history entry ID, so the
//! history database (shared with the Mac app) and its text stay untouched.
//! Suggestions are only proposals: nothing reaches the dictionary until the
//! user adds it, and forms made of everyday words carry a warning.

mod learn;

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::dictionary::DictionaryEntry;
pub use learn::{Suggestion, learn};

const VERSION: u32 = 1;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Corrections {
    pub version: u32,
    pub entries: Vec<Correction>,
    pub dismissed: Vec<Dismissed>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Correction {
    pub id: String,
    pub original: String,
    pub corrected: String,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Dismissed {
    pub heard: String,
    pub term: String,
}

/// A suggestion ready to show: `everyday` means the form is ordinary speech.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    pub suggestion: Suggestion,
    pub everyday: bool,
}

impl Corrections {
    pub fn load() -> Result<Self, String> {
        Self::load_from(&crate::paths::corrections_file()?)
    }

    pub fn save(&self) -> Result<(), String> {
        self.save_to(&crate::paths::corrections_file()?)
    }

    /// A missing file is empty; an unreadable one is an error, never overwritten.
    pub fn load_from(path: &Path) -> Result<Self, String> {
        match fs::read(path) {
            Ok(data) => serde_json::from_slice(&data)
                .map_err(|error| format!("could not read {}: {error}", path.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(format!("could not read {}: {error}", path.display())),
        }
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        let parent = path.parent().ok_or("corrections file has no folder")?;
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let saved = Self {
            version: VERSION,
            ..self.clone()
        };
        let data = serde_json::to_vec_pretty(&saved).map_err(|error| error.to_string())?;
        let partial = path.with_extension(format!("{}.part", std::process::id()));
        fs::write(&partial, data)
            .and_then(|()| fs::rename(&partial, path))
            .map_err(|error| format!("could not save corrections: {error}"))
    }

    pub fn get(&self, id: &str) -> Option<&Correction> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// One correction per entry: a new save replaces the old one, and saving
    /// the original text again removes it.
    pub fn record(&mut self, id: &str, original: &str, corrected: &str, now_ms: i64) {
        self.entries.retain(|entry| entry.id != id);
        if corrected.trim() != original.trim() && !corrected.trim().is_empty() {
            self.entries.push(Correction {
                id: id.to_string(),
                original: original.to_string(),
                corrected: corrected.trim().to_string(),
                updated_at_ms: now_ms,
            });
        }
    }

    pub fn dismiss(&mut self, suggestion: &Suggestion) {
        if !self.is_dismissed(suggestion) {
            self.dismissed.push(Dismissed {
                heard: suggestion.heard.clone(),
                term: suggestion.term.clone(),
            });
        }
    }

    fn is_dismissed(&self, suggestion: &Suggestion) -> bool {
        self.dismissed
            .iter()
            .any(|known| known.heard == suggestion.heard && known.term == suggestion.term)
    }

    /// Suggestions not yet dismissed and not already in the dictionary.
    pub fn proposals(&self, dictionary: &[DictionaryEntry]) -> Vec<Proposal> {
        let pairs: Vec<(&str, &str)> = self
            .entries
            .iter()
            .map(|entry| (entry.original.as_str(), entry.corrected.as_str()))
            .collect();
        let known: Vec<String> = dictionary.iter().map(|entry| entry.term.clone()).collect();
        learn(&pairs, &known)
            .into_iter()
            .filter(|suggestion| !self.is_dismissed(suggestion))
            .filter(|suggestion| {
                !crate::dictionary::covers(dictionary, &suggestion.term, &suggestion.heard)
            })
            .map(|suggestion| Proposal {
                everyday: crate::dictionary::is_common(&suggestion.heard),
                suggestion,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
