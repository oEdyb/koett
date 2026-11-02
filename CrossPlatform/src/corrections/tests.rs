use super::*;

fn temporary(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "koett-corrections-{name}-{}.json",
        std::process::id()
    ))
}

#[test]
fn record_replaces_and_reverting_removes_the_entry() {
    let mut corrections = Corrections::default();
    corrections.record("a", "ask Grok", "ask Groq", 1);
    corrections.record("a", "ask Grok", "ask Groq now", 2);
    assert_eq!(corrections.entries.len(), 1);
    assert_eq!(
        corrections.get("a").map(|c| c.corrected.as_str()),
        Some("ask Groq now")
    );
    corrections.record("a", "ask Grok", " ask Grok ", 3);
    assert!(corrections.get("a").is_none());
}

#[test]
fn proposals_skip_dismissed_and_dictionary_forms_and_flag_everyday_words() {
    let mut corrections = Corrections::default();
    corrections.record(
        "a",
        "ask Grok about Superbase",
        "ask Groq about Supabase",
        1,
    );
    corrections.record("b", "all will review", "Olle will review", 2);
    let dictionary = vec![DictionaryEntry::new("Supabase", &["Superbase"])];
    let proposals = corrections.proposals(&dictionary);
    let forms: Vec<(&str, bool)> = proposals
        .iter()
        .map(|p| (p.suggestion.heard.as_str(), p.everyday))
        .collect();
    assert_eq!(forms, [("Grok", false), ("all", true)]);

    corrections.dismiss(&proposals[0].suggestion);
    let left = corrections.proposals(&dictionary);
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].suggestion.term, "Olle");
}

#[test]
fn file_round_trips_and_missing_file_is_empty() {
    let path = temporary("round-trip");
    assert_eq!(
        Corrections::load_from(&path).unwrap(),
        Corrections::default()
    );
    let mut corrections = Corrections::default();
    corrections.record("a", "ask Grok", "ask Groq", 7);
    corrections.save_to(&path).unwrap();
    let loaded = Corrections::load_from(&path).unwrap();
    assert_eq!(loaded.version, VERSION);
    assert_eq!(loaded.entries, corrections.entries);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn unreadable_file_is_an_error_and_kept() {
    let path = temporary("invalid");
    std::fs::write(&path, b"not json").unwrap();
    assert!(Corrections::load_from(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"not json");
    std::fs::remove_file(path).unwrap();
}

#[cfg(unix)]
#[test]
fn saved_file_is_private() {
    // Codex review 2 (B1): the file was created 0644.
    use std::os::unix::fs::PermissionsExt as _;
    let folder =
        std::env::temp_dir().join(format!("koett-corrections-private-{}", std::process::id()));
    let path = folder.join("Corrections.json");
    Corrections::default().save_to(&path).unwrap();
    let mode =
        |path: &std::path::Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&path), 0o600);
    assert_eq!(mode(&folder), 0o700);
    assert_eq!(
        std::fs::read_dir(&folder).unwrap().count(),
        1,
        "no temporary file left"
    );
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn corrections_and_dismissals_are_bounded_oldest_first() {
    // Codex review 2 (B4).
    let mut corrections = Corrections::default();
    for n in 0..MAX_CORRECTIONS + 5 {
        corrections.record(&format!("id{n}"), "a Grok", "a Groq", n as i64);
    }
    assert_eq!(corrections.entries.len(), MAX_CORRECTIONS);
    assert!(corrections.get("id4").is_none() && corrections.get("id5").is_some());
    for n in 0..MAX_DISMISSED + 3 {
        corrections.dismiss(&Suggestion {
            heard: format!("h{n}"),
            term: "T".into(),
            count: 1,
        });
    }
    assert_eq!(corrections.dismissed.len(), MAX_DISMISSED);
    assert_eq!(corrections.dismissed[0].heard, "h3");
}
