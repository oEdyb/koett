//! Turn fixed transcripts into "sounds like" suggestions.
//!
//! Words are aligned with Levenshtein over whitespace tokens. Each edited run
//! of one to three words becomes "heard → term". Large rewrites, punctuation
//! fixes and ordinary capitalization ("put" → "Put") are ignored. Suggestions
//! are ranked by how many entries made the same fix; they are never applied
//! without the user's explicit acceptance.

use std::collections::{BTreeMap, BTreeSet};

const MAX_TOKENS: usize = 256;
const MAX_SPAN_WORDS: usize = 3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Suggestion {
    pub heard: String,
    pub term: String,
    pub count: usize,
}

/// `pairs` are (original, corrected); `known` lists dictionary terms, which are
/// accepted even when written in lowercase.
pub fn learn(pairs: &[(&str, &str)], known: &[String]) -> Vec<Suggestion> {
    let mut counts: BTreeMap<(String, String), usize> = BTreeMap::new();
    for &(original, corrected) in pairs {
        for fix in fixes(original, corrected, known) {
            *counts.entry(fix).or_default() += 1;
        }
    }
    let mut suggestions: Vec<Suggestion> = counts
        .into_iter()
        .map(|((heard, term), count)| Suggestion { heard, term, count })
        .collect();
    suggestions.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.term.cmp(&b.term))
            .then_with(|| a.heard.cmp(&b.heard))
    });
    suggestions
}

/// Distinct fixes in one entry, so an entry counts once however often it repeats a fix.
fn fixes(original: &str, corrected: &str, known: &[String]) -> BTreeSet<(String, String)> {
    let a = tokens(original);
    let b = tokens(corrected);
    let mut found = BTreeSet::new();
    if a.len().max(b.len()) > MAX_TOKENS {
        return found;
    }
    let steps = align(&a, &b);
    let edits = steps.iter().filter(|step| !step.2).count();
    if edits == 0 || edits > 3.max(a.len().max(b.len()) / 3) {
        return found;
    }
    for span in steps.split(|step| step.2) {
        let heard: Vec<&str> = span
            .iter()
            .filter_map(|step| step.0.map(|i| a[i]))
            .collect();
        let term: Vec<&str> = span
            .iter()
            .filter_map(|step| step.1.map(|j| b[j]))
            .collect();
        let sized = |words: &[&str]| (1..=MAX_SPAN_WORDS).contains(&words.len());
        if !sized(&heard) || !sized(&term) {
            continue;
        }
        let (heard, term) = (heard.join(" "), term.join(" "));
        if heard != term && distinctive(&term, known) {
            found.insert((heard, term));
        }
    }
    found
}

/// Alignment steps: (original index, corrected index, unchanged).
fn align(a: &[&str], b: &[&str]) -> Vec<(Option<usize>, Option<usize>, bool)> {
    let width = b.len() + 1;
    let mut cost = vec![0_usize; (a.len() + 1) * width];
    for i in 0..=a.len() {
        for j in 0..=b.len() {
            cost[i * width + j] = match (i, j) {
                (0, _) => j,
                (_, 0) => i,
                _ => (cost[(i - 1) * width + j] + 1)
                    .min(cost[i * width + j - 1] + 1)
                    .min(cost[(i - 1) * width + j - 1] + usize::from(a[i - 1] != b[j - 1])),
            };
        }
    }
    let (mut i, mut j) = (a.len(), b.len());
    let mut steps = Vec::new();
    while i > 0 || j > 0 {
        let same = i > 0 && j > 0 && a[i - 1] == b[j - 1];
        if i > 0
            && j > 0
            && cost[i * width + j] == cost[(i - 1) * width + j - 1] + usize::from(!same)
        {
            steps.push((Some(i - 1), Some(j - 1), same));
            i -= 1;
            j -= 1;
        } else if i > 0 && cost[i * width + j] == cost[(i - 1) * width + j] + 1 {
            steps.push((Some(i - 1), None, false));
            i -= 1;
        } else {
            steps.push((None, Some(j - 1), false));
            j -= 1;
        }
    }
    steps.reverse();
    steps
}

fn tokens(text: &str) -> Vec<&str> {
    text.split_whitespace()
        .map(|word| word.trim_matches(|ch: char| !ch.is_alphanumeric() && ch != '-'))
        .filter(|word| !word.is_empty())
        .collect()
}

/// A term worth learning: a known dictionary term, or a capitalized word that is
/// not just an ordinary word at the start of a sentence.
fn distinctive(term: &str, known: &[String]) -> bool {
    known.iter().any(|known| known == term)
        || term.chars().any(char::is_uppercase) && !crate::dictionary::is_common(term)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn learned(pairs: &[(&str, &str)]) -> Vec<(String, String, usize)> {
        learn(pairs, &[])
            .into_iter()
            .map(|s| (s.heard, s.term, s.count))
            .collect()
    }

    #[test]
    fn repeated_fixes_rank_first_and_keep_unicode() {
        let pairs = [
            ("visit Gothenburg today", "visit Göteborg today"),
            ("visit Gothenburg now", "visit Göteborg now"),
            ("ask Grok", "ask Groq"),
        ];
        assert_eq!(
            learned(&pairs)[0],
            ("Gothenburg".into(), "Göteborg".into(), 2)
        );
    }

    #[test]
    fn multiword_spans_are_learned() {
        assert_eq!(
            learned(&[("use fluid audio now", "use FluidAudio now")]),
            [("fluid audio".into(), "FluidAudio".into(), 1)]
        );
    }

    #[test]
    fn punctuation_case_and_rewrites_are_ignored() {
        let pairs = [
            ("a cat.", "a cat!"),
            ("a cot", "a cat"),
            ("put it here", "Put it here"),
            ("ONE TWO THREE FOUR FIVE", "six seven eight nine ten"),
        ];
        assert!(learned(&pairs).is_empty());
    }

    #[test]
    fn known_lowercase_terms_are_learned() {
        let known = ["fika".to_string()];
        let suggestions = learn(&[("grab a fek later", "grab a fika later")], &known);
        assert_eq!(suggestions[0].term, "fika");
    }

    #[test]
    fn one_entry_counts_a_fix_once() {
        let pairs = [("all and all", "Olle and Olle")];
        assert_eq!(learned(&pairs), [("all".into(), "Olle".into(), 1)]);
    }
}
