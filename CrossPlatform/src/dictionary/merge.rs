//! Keep the plain decode and take only the biased decode's dictionary words.
//!
//! Hotword biasing finds terms the plain decoder misses, but its boost also
//! nudges unrelated words ("Code Turns My Voice", "invoices" → "receipts").
//! Both word sequences are aligned. A differing biased span is used only when
//! it is exactly one dictionary term; the term then replaces the plain words
//! that sound closest to it, and every other plain word and its punctuation stay.
//! If those plain words are all common English ("the cloud" vs "Claude"), the
//! biased reading is too likely a boosted homophone, and the plain words stay.

use super::DictionaryEntry;
use super::correct;

/// A differing plain span longer than this is too ambiguous to place a term in.
const MAX_PLAIN_SPAN: usize = 8;

pub fn adopt_terms(plain: &str, biased: &str, entries: &[DictionaryEntry]) -> String {
    let left: Vec<&str> = plain.split_whitespace().collect();
    let right: Vec<&str> = biased.split_whitespace().collect();
    let left_keys: Vec<String> = left.iter().map(|word| correct::key(word)).collect();
    let right_keys: Vec<String> = right.iter().map(|word| correct::key(word)).collect();
    let mut output: Vec<String> = Vec::with_capacity(left.len());
    let (mut i, mut j) = (0, 0);
    for (next_i, next_j) in matches(&left_keys, &right_keys)
        .into_iter()
        .chain([(left.len(), right.len())])
    {
        let plain_span = &left[i..next_i];
        let term = (j < next_j)
            .then(|| correct::exact_term(&right[j..next_j].join(" "), entries))
            .flatten();
        match term {
            Some(term) if plain_span.len() <= MAX_PLAIN_SPAN => {
                output.extend(place(plain_span, &left_keys[i..next_i], term));
            }
            _ => output.extend(plain_span.iter().map(|word| word.to_string())),
        }
        if next_i < left.len() {
            output.push(left[next_i].to_string());
        }
        (i, j) = (next_i + 1, next_j + 1);
    }
    output.join(" ")
}

/// Replace the run of plain words closest in sound to `term`, keeping the
/// words around it and the punctuation at its edges.
fn place(words: &[&str], keys: &[String], term: &str) -> Vec<String> {
    if words.is_empty() {
        return vec![term.to_string()];
    }
    let target = correct::key(term);
    let mut best = (usize::MAX, 0, 0);
    for start in 0..words.len() {
        let mut joined = String::new();
        for end in start + 1..=words.len() {
            joined.push_str(&keys[end - 1]);
            let cost = distance(&joined, &target);
            if cost < best.0 {
                best = (cost, start, end);
            }
        }
    }
    let (_, start, end) = best;
    let replaced = words[start..end].join(" ");
    if super::common::is_common(&replaced) && correct::key(&replaced) != target {
        return words.iter().map(|word| word.to_string()).collect();
    }
    let lead = edge(words[start], true);
    let trail = edge(words[end - 1], false);
    let mut placed: Vec<String> = words[..start].iter().map(|word| word.to_string()).collect();
    placed.push(format!("{lead}{term}{trail}"));
    placed.extend(words[end..].iter().map(|word| word.to_string()));
    placed
}

/// Leading or trailing punctuation of a whitespace word.
fn edge(word: &str, leading: bool) -> &str {
    let core = |ch: char| ch.is_alphanumeric();
    if leading {
        &word[..word.find(core).unwrap_or(word.len())]
    } else {
        &word[word.rfind(core).map_or(0, |index| {
            index + word[index..].chars().next().map_or(0, char::len_utf8)
        })..]
    }
}

fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, left) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, right) in b.iter().enumerate() {
            let next = (row[j + 1] + 1)
                .min(row[j] + 1)
                .min(diagonal + usize::from(left != *right));
            diagonal = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}

/// Index pairs of a longest common subsequence of word keys, in order.
fn matches(left: &[String], right: &[String]) -> Vec<(usize, usize)> {
    let width = right.len() + 1;
    let mut lengths = vec![0_u32; (left.len() + 1) * width];
    for a in (0..left.len()).rev() {
        for b in (0..right.len()).rev() {
            lengths[a * width + b] = if left[a] == right[b] {
                lengths[(a + 1) * width + b + 1] + 1
            } else {
                lengths[(a + 1) * width + b].max(lengths[a * width + b + 1])
            };
        }
    }
    let (mut a, mut b, mut pairs) = (0, 0, Vec::new());
    while a < left.len() && b < right.len() {
        if left[a] == right[b] {
            pairs.push((a, b));
            a += 1;
            b += 1;
        } else if lengths[(a + 1) * width + b] >= lengths[a * width + b + 1] {
            a += 1;
        } else {
            b += 1;
        }
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dictionary() -> Vec<DictionaryEntry> {
        ["Koett", "Supabase", "Olle Dyberg"]
            .iter()
            .map(|term| DictionaryEntry::new(term, &[]))
            .collect()
    }

    #[test]
    fn takes_the_biased_span_that_spells_a_term() {
        assert_eq!(
            adopt_terms(
                "Superbase handles the login.",
                "Supabase handles the Login.",
                &dictionary()
            ),
            "Supabase handles the login."
        );
    }

    #[test]
    fn keeps_plain_casing_when_biased_text_has_no_term() {
        let plain = "The super base price does not include tax.";
        let biased = "The Super Base Price does not include Tax.";
        assert_eq!(adopt_terms(plain, biased, &dictionary()), plain);
    }

    #[test]
    fn multiword_terms_and_different_lengths_align() {
        assert_eq!(
            adopt_terms(
                "Hi this is Old Dieberg calling.",
                "Hi This is Olle Dyberg calling.",
                &dictionary()
            ),
            "Hi this is Olle Dyberg calling."
        );
    }

    #[test]
    fn biased_rewrites_around_a_term_are_not_adopted() {
        // Codex review of PR #8: "invoices" must not become "receipts".
        assert_eq!(
            adopt_terms(
                "Send Superbase invoices tomorrow.",
                "Send Supabase receipts tomorrow.",
                &dictionary()
            ),
            "Send Superbase invoices tomorrow."
        );
    }

    #[test]
    fn a_term_replaces_only_the_closest_plain_words() {
        assert_eq!(
            adopt_terms(
                "Pay the Superbase invoices.",
                "Pay Supabase.",
                &dictionary()
            ),
            "Pay the Supabase invoices."
        );
    }

    #[test]
    fn common_plain_words_are_not_turned_into_terms() {
        let dictionary = [DictionaryEntry::new("Claude", &[])];
        let plain = "It lives in the cloud.";
        assert_eq!(
            adopt_terms(plain, "It lives in the Claude.", &dictionary),
            plain
        );
    }

    #[test]
    fn plain_punctuation_survives_around_the_term() {
        assert_eq!(
            adopt_terms("Ask (koet), please", "Ask Koett please", &dictionary()),
            "Ask (Koett), please"
        );
    }

    #[test]
    fn identical_or_empty_input_is_unchanged() {
        assert_eq!(
            adopt_terms("koett works", "koett works", &dictionary()),
            "koett works"
        );
        assert_eq!(adopt_terms("", "Koett", &dictionary()), "Koett");
        assert_eq!(adopt_terms("plain words", "", &dictionary()), "plain words");
    }
}
