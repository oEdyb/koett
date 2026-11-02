//! Rewrite spans that spell a dictionary term or one of its "sounds like" forms.
//!
//! Matching ignores case, spaces and hyphens ("sherpa onnx", "post gres q l" with
//! form "post gres q l"), but never punctuation, so sentences keep their shape.
//! Terms with their own punctuation (`C++`, `C#`, `Node.js`) match only when the
//! text has that punctuation, and the whole literal is replaced. Text and keys are
//! NFC, so decomposed "Göteborg" and "Göteborg.dev" match too. One pass; written
//! terms are never matched again.

use std::borrow::Cow;

use icu_normalizer::ComposingNormalizerBorrowed;

use super::DictionaryEntry;

const MAX_SPAN_WORDS: usize = 4;

/// A key to find and the term to write. Terms match across any word split
/// ("sherpa onnx"); a sounds-like form only with its own word count, so the
/// form "Superbase" leaves an ordinary "super base" alone. A literal target
/// keeps its punctuation and is compared character by character.
struct Target<'a> {
    key: String,
    words: Option<usize>,
    literal: Option<String>,
    term: &'a str,
}

impl<'a> Target<'a> {
    fn new(form: &str, words: Option<usize>, term: &'a str) -> Self {
        Self {
            key: key(form),
            words,
            literal: significant_punctuation(form).then(|| squash(form)),
            term,
        }
    }
}

/// One word: its byte range in the text and its lowercase alphanumeric key.
struct Word {
    start: usize,
    end: usize,
    key: String,
}

/// A match: replaced byte range and the index of the first word after it.
struct Found<'a> {
    start: usize,
    end: usize,
    next: usize,
    term: &'a str,
}

pub fn apply(text: &str, entries: &[DictionaryEntry]) -> String {
    let targets = targets(entries, true);
    if targets.is_empty() {
        return text.to_string();
    }
    // Offsets below index the NFC text, so literal and word matches agree.
    let text = nfc(text);
    let text = text.as_ref();
    let words = words(text);
    let mut output = String::with_capacity(text.len());
    let mut copied = 0;
    let mut index = 0;
    while index < words.len() {
        match find(text, &words, index, &targets) {
            Some(found) if found.start >= copied => {
                output.push_str(&text[copied..found.start]);
                output.push_str(found.term);
                copied = found.end;
                index = found.next;
            }
            _ => index += 1,
        }
    }
    output.push_str(&text[copied..]);
    output
}

/// The term when the whole text, apart from outer punctuation, spells exactly one term.
pub(super) fn exact_term<'a>(text: &str, entries: &'a [DictionaryEntry]) -> Option<&'a str> {
    let targets = targets(entries, false);
    let text = nfc(text);
    let text = text.as_ref();
    let words = words(text);
    let found = find(text, &words, 0, &targets)?;
    let outside = |slice: &str| slice.chars().all(|ch| !part_of_word(ch));
    (found.next == words.len() && outside(&text[..found.start]) && outside(&text[found.end..]))
        .then_some(found.term)
}

/// Every term (and, if asked, every form), longest keys first so longer phrases win.
fn targets(entries: &[DictionaryEntry], forms: bool) -> Vec<Target<'_>> {
    let mut targets: Vec<Target> = entries
        .iter()
        .flat_map(|entry| {
            let term = Target::new(&entry.term, None, &entry.term);
            let forms = entry
                .sounds_like
                .iter()
                .filter(move |_| forms)
                .map(|form| Target::new(form, Some(words(form).len()), &entry.term));
            std::iter::once(term).chain(forms)
        })
        .filter(|target| !target.key.is_empty())
        .collect();
    targets.sort_by_key(|target| std::cmp::Reverse(target.key.len()));
    targets
}

fn find<'a>(text: &str, words: &[Word], index: usize, targets: &[Target<'a>]) -> Option<Found<'a>> {
    if index >= words.len() {
        return None;
    }
    literal_match(text, words, index, targets).or_else(|| word_match(text, words, index, targets))
}

fn word_match<'a>(
    text: &str,
    words: &[Word],
    index: usize,
    targets: &[Target<'a>],
) -> Option<Found<'a>> {
    let words = &words[index..];
    let mut joined = String::new();
    let mut best = None;
    for count in 1..=MAX_SPAN_WORDS.min(words.len()) {
        if count > 1 && !joinable(&text[words[count - 2].end..words[count - 1].start]) {
            break;
        }
        joined.push_str(&words[count - 1].key);
        if let Some(target) = targets.iter().find(|target| {
            target.literal.is_none()
                && target.key == joined
                && target.words.is_none_or(|words| words == count)
        }) {
            best = Some(Found {
                start: words[0].start,
                end: words[count - 1].end,
                next: index + count,
                term: target.term,
            });
        }
    }
    best
}

/// A literal target starting at this word, possibly with leading punctuation
/// (".NET"). The longest literal wins; word boundaries are required on both sides.
fn literal_match<'a>(
    text: &str,
    words: &[Word],
    index: usize,
    targets: &[Target<'a>],
) -> Option<Found<'a>> {
    let word_start = words[index].start;
    targets
        .iter()
        .filter_map(|target| {
            let literal = target.literal.as_deref()?;
            let lead: String = literal
                .chars()
                .take_while(|ch| !part_of_word(*ch))
                .collect();
            let start = word_start.checked_sub(lead.len())?;
            if text.get(start..word_start)? != lead {
                return None;
            }
            let end = literal_end(text, start, literal)?;
            let before = text[..start].chars().next_back();
            let after = text[end..].chars().next();
            (!before.is_some_and(part_of_word) && !after.is_some_and(part_of_word)).then(|| Found {
                start,
                end,
                next: words
                    .iter()
                    .position(|word| word.start >= end)
                    .unwrap_or(words.len()),
                term: target.term,
            })
        })
        .max_by_key(|found| found.end)
}

/// End of `literal` in `text` from `start`, case-insensitive, skipping spaces
/// and hyphens between characters the way `squash` does.
fn literal_end(text: &str, start: usize, literal: &str) -> Option<usize> {
    let mut expected = literal.chars().peekable();
    let mut end = start;
    for (offset, ch) in text[start..].char_indices() {
        let Some(&next) = expected.peek() else { break };
        if loose(ch) && end > start {
            continue;
        }
        if !ch.to_lowercase().eq(next.to_lowercase()) {
            return None;
        }
        expected.next();
        end = start + offset + ch.len_utf8();
    }
    expected.peek().is_none().then_some(end)
}

fn joinable(gap: &str) -> bool {
    gap.chars().all(loose)
}

fn loose(ch: char) -> bool {
    ch.is_whitespace() || ch == '-'
}

/// Punctuation that belongs to the term: anything besides letters, digits,
/// spaces, hyphens and combining marks.
fn significant_punctuation(form: &str) -> bool {
    form.chars().any(|ch| !part_of_word(ch) && !loose(ch))
}

fn squash(form: &str) -> String {
    nfc(form).chars().filter(|ch| !loose(*ch)).collect()
}

fn words(text: &str) -> Vec<Word> {
    let mut words = Vec::new();
    let mut start = None;
    for (offset, ch) in text.char_indices().chain([(text.len(), ' ')]) {
        match (start, part_of_word(ch)) {
            (None, true) => start = Some(offset),
            (Some(begin), false) => {
                words.push(Word {
                    start: begin,
                    end: offset,
                    key: key(&text[begin..offset]),
                });
                start = None;
            }
            _ => {}
        }
    }
    words
}

fn part_of_word(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_' || ('\u{0300}'..='\u{036f}').contains(&ch)
}

fn nfc(text: &str) -> Cow<'_, str> {
    ComposingNormalizerBorrowed::new_nfc().normalize(text)
}

pub(super) fn key(text: &str) -> String {
    nfc(text)
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(term: &str, forms: &[&str]) -> DictionaryEntry {
        DictionaryEntry::new(term, forms)
    }

    #[test]
    fn restores_spelling_across_case_spaces_and_hyphens() {
        let entries = [entry("sherpa-onnx", &[]), entry("PostgreSQL", &[])];
        assert_eq!(
            apply("We use Sherpa Onnx and postgre-sql.", &entries),
            "We use sherpa-onnx and PostgreSQL."
        );
    }

    #[test]
    fn sounds_like_forms_write_the_term() {
        let entries = [entry("Supabase", &["super base"]), entry("Groq", &["grok"])];
        assert_eq!(
            apply("Super base and Grok, then super-base!", &entries),
            "Supabase and Groq, then Supabase!"
        );
    }

    #[test]
    fn sounds_like_forms_keep_their_word_count() {
        let entries = [
            entry("Supabase", &["Superbase"]),
            entry("Fortnox", &["four knocks"]),
        ];
        assert_eq!(
            apply(
                "The super base price; Superbase pays via four knocks.",
                &entries
            ),
            "The super base price; Supabase pays via Fortnox."
        );
    }

    #[test]
    fn punctuation_between_words_blocks_a_match() {
        let entries = [entry("Supabase", &["super base"])];
        assert_eq!(apply("Super. Base", &entries), "Super. Base");
    }

    #[test]
    fn partial_words_and_plurals_stay_unchanged() {
        let entries = [entry("Koett", &["cot"]), entry("Parakeet", &[])];
        assert_eq!(
            apply("A cottage, cots and parakeets.", &entries),
            "A cottage, cots and parakeets."
        );
    }

    #[test]
    fn longest_span_wins_and_written_terms_do_not_cascade() {
        let entries = [
            entry("Olle Dyberg", &["all dyberg"]),
            entry("Olle", &["all"]),
        ];
        assert_eq!(
            apply("all Dyberg and all", &entries),
            "Olle Dyberg and Olle"
        );
    }

    #[test]
    fn unicode_terms_and_keys_match() {
        let entries = [entry("Göteborg", &["gataborg"]), entry("fika", &[])];
        assert_eq!(
            apply("Gataborg för en FIKA, göteborg.", &entries),
            "Göteborg för en fika, Göteborg."
        );
    }

    #[test]
    fn punctuated_terms_replace_the_whole_literal() {
        let entries = [entry("C++", &[]), entry("C#", &[]), entry(".NET", &[])];
        assert_eq!(
            apply("c++ code, C++ and c# on .net", &entries),
            "C++ code, C++ and C# on .NET"
        );
    }

    #[test]
    fn punctuated_terms_need_their_punctuation() {
        let entries = [entry("C++", &[]), entry("Node.js", &[])];
        assert_eq!(
            apply("Plan C, node js and Node.js.", &entries),
            "Plan C, node js and Node.js."
        );
    }

    #[test]
    fn decomposed_letters_match_composed_terms() {
        let entries = [entry("Göteborg", &[])];
        assert_eq!(apply("i go\u{308}teborg idag", &entries), "i Göteborg idag");
    }

    #[test]
    fn punctuation_only_spans_are_not_terms() {
        // Codex review 2 (A1): "Hello , world" aligns "," alone and used to panic.
        let entries = [entry("Koett", &[])];
        assert_eq!(exact_term(",", &entries), None);
        assert_eq!(exact_term("", &entries), None);
    }

    #[test]
    fn decomposed_text_matches_punctuated_terms() {
        // Codex review 2 (A4).
        let entries = [entry("Göteborg.dev", &[])];
        assert_eq!(
            apply("see go\u{308}teborg.dev now", &entries),
            "see Göteborg.dev now"
        );
    }

    #[test]
    fn exact_term_requires_the_whole_span() {
        let entries = [entry("Supabase", &["Superbase"]), entry("Olle Dyberg", &[])];
        assert_eq!(exact_term("Supabase,", &entries), Some("Supabase"));
        assert_eq!(exact_term("Olle Dyberg", &entries), Some("Olle Dyberg"));
        assert_eq!(exact_term("Supabase receipts", &entries), None);
        assert_eq!(exact_term("Superbase", &entries), None);
    }

    #[test]
    fn empty_dictionary_returns_text_unchanged() {
        assert_eq!(apply(" keep  this ", &[]), " keep  this ");
    }
}
