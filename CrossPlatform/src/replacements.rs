use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Replacement {
    pub phrase: String,
    pub replacement: String,
}

/// Case-sensitive literal phrases, longest match first; list order breaks ties.
/// A single pass over the original prevents replacement text from cascading.
pub fn apply(text: &str, rules: &[Replacement]) -> String {
    let mut output = String::with_capacity(text.len());
    let mut offset = 0;
    while offset < text.len() {
        let rest = &text[offset..];
        let matched = rules
            .iter()
            .enumerate()
            .filter(|(_, rule)| {
                !rule.phrase.is_empty()
                    && rest.starts_with(&rule.phrase)
                    && boundary(
                        text[..offset].chars().next_back(),
                        rule.phrase.chars().next(),
                    )
                    && boundary(
                        rest[rule.phrase.len()..].chars().next(),
                        rule.phrase.chars().next_back(),
                    )
            })
            .max_by_key(|(index, rule)| (rule.phrase.len(), std::cmp::Reverse(*index)));
        if let Some((_, rule)) = matched {
            output.push_str(&rule.replacement);
            offset += rule.phrase.len();
        } else {
            let ch = rest.chars().next().expect("nonempty remainder");
            output.push(ch);
            offset += ch.len_utf8();
        }
    }
    output
}

fn boundary(neighbor: Option<char>, edge: Option<char>) -> bool {
    !edge.is_some_and(word) || !neighbor.is_some_and(word)
}
fn word(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_' || ('\u{0300}'..='\u{036f}').contains(&ch)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rule(a: &str, b: &str) -> Replacement {
        Replacement {
            phrase: a.into(),
            replacement: b.into(),
        }
    }
    #[test]
    fn longest_original_phrase_wins_without_cascades() {
        assert_eq!(
            apply(
                "New York York",
                &[
                    rule("New", "old"),
                    rule("New York", "York"),
                    rule("York", "Y")
                ]
            ),
            "York Y"
        );
    }
    #[test]
    fn unicode_boundaries_and_case_are_explicit() {
        assert_eq!(
            apply("ö öl Ö ö_ ö\u{301}", &[rule("ö", "YES")]),
            "YES öl Ö ö_ ö\u{301}"
        );
    }
    #[test]
    fn equal_phrases_use_first_rule() {
        assert_eq!(apply("a", &[rule("a", "1"), rule("a", "2")]), "1");
    }
}
