//! Rewrite spans that spell a dictionary term or one of its "sounds like" forms.
//!
//! Matching ignores case, spaces and hyphens ("sherpa onnx", "post gres q l" with
//! form "post gres q l"), but never punctuation, so sentences keep their shape.
//! It runs once over the original text; written terms are never matched again.

use super::DictionaryEntry;

const MAX_SPAN_WORDS: usize = 4;

/// One word: its byte range in the text and its lowercase alphanumeric key.
struct Word {
    start: usize,
    end: usize,
    key: String,
}

pub fn apply(text: &str, entries: &[DictionaryEntry]) -> String {
    let targets = targets(entries);
    if targets.is_empty() {
        return text.to_string();
    }
    let words = words(text);
    let mut output = String::with_capacity(text.len());
    let mut copied = 0;
    let mut index = 0;
    while index < words.len() {
        match longest_match(text, &words[index..], &targets) {
            Some((count, term)) => {
                output.push_str(&text[copied..words[index].start]);
                output.push_str(term);
                copied = words[index + count - 1].end;
                index += count;
            }
            None => index += 1,
        }
    }
    output.push_str(&text[copied..]);
    output
}

/// True when the text spells any dictionary term (not just a "sounds like" form).
pub(super) fn contains_term(text: &str, entries: &[DictionaryEntry]) -> bool {
    let targets: Vec<(String, &str)> = entries
        .iter()
        .map(|entry| (key(&entry.term), entry.term.as_str()))
        .filter(|(key, _)| !key.is_empty())
        .collect();
    let words = words(text);
    (0..words.len()).any(|index| longest_match(text, &words[index..], &targets).is_some())
}

/// (normalized key, written term), longest keys first so longer phrases win.
fn targets(entries: &[DictionaryEntry]) -> Vec<(String, &str)> {
    let mut targets: Vec<(String, &str)> = entries
        .iter()
        .flat_map(|entry| {
            std::iter::once(&entry.term)
                .chain(&entry.sounds_like)
                .map(move |form| (key(form), entry.term.as_str()))
        })
        .filter(|(key, _)| !key.is_empty())
        .collect();
    targets.sort_by_key(|(key, _)| std::cmp::Reverse(key.len()));
    targets
}

fn longest_match<'a>(
    text: &str,
    words: &[Word],
    targets: &[(String, &'a str)],
) -> Option<(usize, &'a str)> {
    let mut joined = String::new();
    let mut best = None;
    for count in 1..=MAX_SPAN_WORDS.min(words.len()) {
        if count > 1 && !joinable(&text[words[count - 2].end..words[count - 1].start]) {
            break;
        }
        joined.push_str(&words[count - 1].key);
        if let Some((_, term)) = targets.iter().find(|(key, _)| *key == joined) {
            best = Some((count, *term));
        }
    }
    best
}

fn joinable(gap: &str) -> bool {
    gap.chars().all(|ch| ch.is_whitespace() || ch == '-')
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

pub(super) fn key(text: &str) -> String {
    text.chars()
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
            apply("Superbase and Grok, then super-base!", &entries),
            "Supabase and Groq, then Supabase!"
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
    fn empty_dictionary_returns_text_unchanged() {
        assert_eq!(apply(" keep  this ", &[]), " keep  this ");
    }
}
