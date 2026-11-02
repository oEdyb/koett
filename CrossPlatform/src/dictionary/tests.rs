use super::*;

#[test]
fn text_round_trip_keeps_terms_and_forms() {
    let entries = vec![
        DictionaryEntry::new("Koett", &[]),
        DictionaryEntry::new("PostgreSQL", &["post gres", "postgres q l"]),
        DictionaryEntry::new("Göteborg", &["yeah te bori"]),
    ];
    assert_eq!(parse_text(&to_text(&entries)).unwrap(), entries);
}

#[test]
fn parse_ignores_comments_blank_lines_bom_and_extra_space() {
    let parsed = parse_text("\u{feff}# words\n\n  Hogia  =  ho gia ,, hoggia \nfika\n").unwrap();
    assert_eq!(
        parsed,
        vec![
            DictionaryEntry::new("Hogia", &["ho gia", "hoggia"]),
            DictionaryEntry::new("fika", &[]),
        ]
    );
}

#[test]
fn parse_rejects_forms_without_a_term() {
    assert!(parse_text("Koett\n = cot").unwrap_err().contains("Line 2"));
}

#[test]
fn normalize_merges_repeated_terms_and_drops_self_forms() {
    let merged = normalize(vec![
        DictionaryEntry::new("Groq", &["grok"]),
        DictionaryEntry::new(" Groq ", &["grok", "Groq", "grock"]),
        DictionaryEntry::new("  ", &[]),
    ])
    .unwrap();
    assert_eq!(
        merged,
        vec![DictionaryEntry::new("Groq", &["grok", "grock"])]
    );
}

#[test]
fn normalize_rejects_overlong_terms() {
    let long = "x".repeat(MAX_TERM_CHARS + 1);
    assert!(normalize(vec![DictionaryEntry::new(&long, &[])]).is_err());
}

#[test]
fn merge_keeps_existing_order_and_adds_new_forms() {
    let existing = vec![DictionaryEntry::new("Koett", &["cot"])];
    let merged = merge(
        &existing,
        vec![
            DictionaryEntry::new("Nordicrank", &[]),
            DictionaryEntry::new("Koett", &["quett"]),
        ],
    )
    .unwrap();
    assert_eq!(
        merged,
        vec![
            DictionaryEntry::new("Koett", &["cot", "quett"]),
            DictionaryEntry::new("Nordicrank", &[]),
        ]
    );
}
