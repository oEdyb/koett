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

#[test]
fn import_caps_forms_per_word_quickly() {
    let forms: Vec<String> = (0..50_000).map(|n| format!("form {n}")).collect();
    let text = format!("Groq = {}", forms.join(", "));
    let started = std::time::Instant::now();
    let error = parse_text(&text).unwrap_err();
    assert!(error.contains("more than 32"), "{error}");
    assert!(started.elapsed() < std::time::Duration::from_millis(500));
}

#[test]
fn import_caps_total_forms_and_words() {
    let many_forms: String = (0..200)
        .map(|word| {
            let forms: Vec<String> = (0..30).map(|n| format!("w{word} f{n}")).collect();
            format!("Word{word} = {}\n", forms.join(", "))
        })
        .collect();
    assert!(parse_text(&many_forms).unwrap_err().contains("4000"));
    let many_words: String = (0..=MAX_ENTRIES).map(|n| format!("Word{n}\n")).collect();
    assert!(parse_text(&many_words).unwrap_err().contains("2000"));
}

#[test]
fn repeated_forms_are_deduplicated() {
    let text = "Groq = grok, grok, grock\nGroq = grok";
    assert_eq!(
        parse_text(text).unwrap(),
        vec![DictionaryEntry::new("Groq", &["grok", "grock"])]
    );
}

#[test]
fn add_form_creates_or_extends_entries_and_covers_reports_them() {
    let entries = add_form(&[], "Groq", "Grok").unwrap();
    assert_eq!(entries, vec![DictionaryEntry::new("Groq", &["Grok"])]);
    assert!(covers(&entries, "Groq", "grok"));
    let entries = add_form(&entries, "Koett", "koett").unwrap();
    assert_eq!(entries[1], DictionaryEntry::new("Koett", &[]));
    assert!(covers(&entries, "Koett", "KOETT"));
    assert!(!covers(&entries, "Koett", "coat"));
}
