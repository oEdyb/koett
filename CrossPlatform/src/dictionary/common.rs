//! Common English words (SCOWL levels 10 and 20). Biased decoding hears
//! "cloud" as a dictionary "Claude"; when the plain decode wrote only common
//! words, those words stay. "Fix a word" uses the same list to warn before a
//! "sounds like" form would rewrite ordinary speech.

use std::collections::HashSet;
use std::sync::OnceLock;

const LIST: &str = include_str!("common-words.txt");

fn words() -> &'static HashSet<&'static str> {
    static WORDS: OnceLock<HashSet<&'static str>> = OnceLock::new();
    WORDS.get_or_init(|| {
        LIST.lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
            .collect()
    })
}

/// True when the phrase has words and every one is a common English word.
pub fn is_common(phrase: &str) -> bool {
    let mut parts = phrase
        .split(|ch: char| !ch.is_alphabetic())
        .filter(|part| !part.is_empty())
        .peekable();
    parts.peek().is_some() && parts.all(|part| words().contains(part.to_lowercase().as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_words_and_phrases_are_common() {
        for phrase in ["all", "Cloud", "calling", "code", "open code", "super base"] {
            assert!(is_common(phrase), "{phrase}");
        }
    }

    #[test]
    fn misheard_names_and_jargon_are_not_common() {
        for phrase in [
            "Grok",
            "Superbase",
            "Towery",
            "Quinn",
            "fek",
            "Claude",
            "",
            "C++",
        ] {
            assert!(!is_common(phrase), "{phrase}");
        }
    }
}
