//! Keep the plain decode and take only the biased decode's dictionary words.
//!
//! Hotword biasing finds terms the plain decoder misses, but its boost also
//! nudges unrelated words toward capitalized pieces ("Code Turns My Voice").
//! Aligning both word sequences and adopting a differing biased span only
//! when it spells a dictionary term keeps every other word as the plain decode
//! wrote it.

use super::DictionaryEntry;
use super::correct;

pub fn adopt_terms(plain: &str, biased: &str, entries: &[DictionaryEntry]) -> String {
    let left: Vec<&str> = plain.split_whitespace().collect();
    let right: Vec<&str> = biased.split_whitespace().collect();
    let left_keys: Vec<String> = left.iter().map(|word| correct::key(word)).collect();
    let right_keys: Vec<String> = right.iter().map(|word| correct::key(word)).collect();
    let mut output: Vec<&str> = Vec::with_capacity(left.len());
    let (mut i, mut j) = (0, 0);
    for (next_i, next_j) in matches(&left_keys, &right_keys)
        .into_iter()
        .chain([(left.len(), right.len())])
    {
        let span = &right[j..next_j];
        if !span.is_empty() && correct::contains_term(&span.join(" "), entries) {
            output.extend_from_slice(span);
        } else {
            output.extend_from_slice(&left[i..next_i]);
        }
        if next_i < left.len() {
            output.push(left[next_i]);
        }
        (i, j) = (next_i + 1, next_j + 1);
    }
    output.join(" ")
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
                "Hi this is all the bird calling.",
                "Hi This is Olle Dyberg calling.",
                &dictionary()
            ),
            "Hi this is Olle Dyberg calling."
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
