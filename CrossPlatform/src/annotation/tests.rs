use super::*;

fn timed(script: &[(&str, f32)]) -> Vec<TimedWord> {
    script
        .iter()
        .enumerate()
        .map(|(index, (text, start))| TimedWord {
            text: (*text).to_owned(),
            start: *start,
            end: script
                .get(index + 1)
                .map_or(start + 0.3, |next| next.1.min(start + 0.4)),
        })
        .collect()
}

#[test]
fn parakeet_tokens_group_into_words_with_first_token_times() {
    let tokens: Vec<String> = [" Well", ",", " I", " don", "'", "t", " w", "ish", "."]
        .map(str::to_owned)
        .to_vec();
    let times = [0.4, 0.64, 0.72, 0.88, 0.96, 1.04, 1.04, 1.2, 1.28];
    let words = words_from_tokens("Well, I don't wish.", &tokens, &times, 10.0, 2.0).unwrap();
    let texts: Vec<&str> = words.iter().map(|word| word.text.as_str()).collect();
    assert_eq!(texts, ["Well,", "I", "don't", "wish."]);
    assert_eq!(words[2].start, 10.88);
    assert_eq!(words[2].end, 11.04);
    assert!((words[3].end - 11.58).abs() < 1e-4);
}

#[test]
fn mismatched_tokens_are_rejected_instead_of_mistimed() {
    let tokens = vec![" Hello".to_owned(), " there".to_owned()];
    assert!(words_from_tokens("Hello world", &tokens, &[0.0, 0.5], 0.0, 1.0).is_none());
    assert!(words_from_tokens("Hello there", &tokens, &[0.0], 0.0, 1.0).is_none());
}

#[test]
fn estimated_words_cover_the_segment_in_order() {
    let words = estimate_words("one three", 2.0, 4.0);
    assert_eq!(words[0].start, 2.0);
    assert!((words[0].end - 2.8).abs() < 1e-4);
    assert_eq!(words[1].end, 4.0);
}

#[test]
fn timed_text_joins_parts_like_segment_transcription() {
    let mut joined = TimedText::default();
    joined.push("Hello there.", estimate_words("Hello there.", 0.0, 1.0));
    joined.push("", Vec::new());
    joined.push("Next.", estimate_words("Next.", 3.0, 3.5));
    assert_eq!(joined.text, "Hello there. Next.");
    assert_eq!(joined.words.len(), 3);
}

#[test]
fn keyframe_owns_its_clause_from_before_the_stroke() {
    // "This button is broken." then drawing begins on "is".
    let words = timed(&[
        ("Hi.", 0.0),
        ("This", 1.0),
        ("button", 1.3),
        ("is", 1.6),
        ("broken.", 1.9),
        ("And", 3.0),
        ("this", 3.3),
        ("too.", 3.6),
    ]);
    let ranges = keyframe_word_ranges(&words, &[1.7, 3.4]);
    assert_eq!(ranges, vec![0..5, 5..8]);
}

#[test]
fn stroke_in_a_pause_after_a_sentence_takes_the_next_clause() {
    let words = timed(&[("First.", 0.0), ("Second", 2.0), ("part.", 2.3)]);
    assert_eq!(keyframe_word_ranges(&words, &[0.1, 1.0]), vec![0..1, 1..3]);
}

#[test]
fn long_pause_breaks_an_unpunctuated_clause() {
    let words = timed(&[("look", 0.0), ("here", 0.3), ("then", 3.0), ("there", 3.3)]);
    assert_eq!(keyframe_word_ranges(&words, &[0.2, 3.1]), vec![0..2, 2..4]);
}

#[test]
fn drawings_inside_one_clause_split_at_the_word_in_progress() {
    let words = timed(&[("one", 0.0), ("two", 0.3), ("three", 0.6)]);
    let ranges = keyframe_word_ranges(&words, &[0.1, 0.4, 0.7]);
    assert_eq!(ranges, vec![0..1, 1..2, 2..3]);
}

#[test]
fn summary_matches_numbered_contact_sheet_bullets() {
    let words = timed(&[("Fix", 0.0), ("this.", 0.4), ("And", 65.0), ("that.", 65.3)]);
    let summary = keyframe_summary("Fix this. And that.", &words, &[0.2, 65.1, 70.0]);
    assert_eq!(
        summary,
        "- #1 (0:00) “Fix this.”\n- #2 (1:05) “And that.”\n- #3 (1:10) marked"
    );
}

#[test]
fn summary_maps_ranges_onto_rewritten_text() {
    let words = timed(&[
        ("um", 0.0),
        ("fix", 0.3),
        ("this.", 0.6),
        ("and", 5.0),
        ("that.", 5.3),
    ]);
    let summary = keyframe_summary("Fix this. And that.", &words, &[0.4, 5.1]);
    assert_eq!(summary, "- #1 (0:00) “Fix this.”\n- #2 (0:05) “And that.”");
}

#[test]
fn summary_without_timing_keeps_the_whole_transcript_on_the_first_keyframe() {
    let summary = keyframe_summary("All of it.", &[], &[1.0, 2.0]);
    assert_eq!(summary, "- #1 (0:01) “All of it.”\n- #2 (0:02) marked");
}

#[test]
fn plain_dictation_is_unchanged_without_keyframes() {
    assert_eq!(keyframe_summary("Just text.", &[], &[]), "Just text.");
}
