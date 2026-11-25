use super::super::bundle::marks::MarkRef;
use super::super::bundle::records::Manifest;
use super::super::{MeetingId, SessionInfo};
use super::*;

fn second(s: u64) -> Offset {
    Offset(s * u64::from(SAMPLE_RATE))
}

fn span(first: u64, end: u64) -> Span {
    Span {
        first: second(first),
        end: second(end),
    }
}

fn word(channel: Channel, first: u64, end: u64, text: &str, lang: &str, backend: &str) -> Point {
    Point::Word(SpeechWord {
        channel,
        span: span(first, end),
        text: text.into(),
        lang: lang.into(),
        backend: backend.into(),
    })
}

fn manifest() -> Manifest {
    Manifest {
        info: SessionInfo {
            id: MeetingId::parse("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            started_at_ms: 0,
        },
        mic: None,
        backend_pref: Backend::Parakeet,
        keep_audio: false,
        export_dir: None,
    }
}

#[test]
fn renders_stable_turns_marks_and_missing_audio() {
    use Channel::{Other, You};
    let points = vec![
        word(Other, 10, 12, "Hej.", "sv", "kb-whisper-large"),
        word(You, 10, 11, "I", "en", PARAKEET),
        word(You, 10, 12, "see.", "en", PARAKEET),
        word(You, 8, 11, "Crossing.", "en", PARAKEET),
        Point::Mark(MarkRef {
            id: 1,
            offset: second(10),
            relative_path: "assets/mark-0000160000-0001.png".into(),
        }),
        Point::MissingAudio {
            channel: Other,
            span: span(10, 15),
        },
        Point::MissingAudio {
            channel: You,
            span: span(10, 15),
        },
        word(Other, 12, 13, "Backend.", "sv", PARAKEET),
        word(Other, 13, 14, "Language.", "en", PARAKEET),
        Point::MissingAudio {
            channel: Other,
            span: span(14, 18),
        },
        word(Other, 14, 15, "Too short.", "en", PARAKEET),
        word(You, 7200, 7201, "Coat.", "en", PARAKEET),
    ];
    let dictionary = [DictionaryEntry::new("Koett", &["coat"])];
    let output = markdown::render(
        &manifest(),
        Offset(second(7201).0 + 15999),
        timeline::merge(points),
        &dictionary,
        false,
    )
    .unwrap();
    assert_eq!(
        output.markdown,
        concat!(
            "---\ntype: meeting-transcript\nkoett_id: 550e8400-e29b-41d4-a716-446655440000\n",
            "date: 1970-01-01T00:00:00Z\nduration: \"120:01\"\n",
            "asr: {kb-whisper-large: 1, parakeet-v3: 6}\n---\n\n",
            "# Meeting · 1970-01-01 00:00\n\n",
            "[00:08] **You:** Crossing. <!-- en parakeet-v3 -->\n\n",
            "*[00:10–00:15] No You audio*\n\n",
            "*[00:10–00:15] No Other audio*\n\n",
            "[00:10] ![Mark 1](assets/mark-0000160000-0001.png)\n\n",
            "[00:10] **You:** I see. <!-- en parakeet-v3 -->\n\n",
            "[00:10] **Other:** Hej. <!-- sv kb-whisper-large -->\n\n",
            "[00:12] **Other:** Backend. <!-- sv parakeet-v3 -->\n\n",
            "[00:13] **Other:** Language. <!-- en parakeet-v3 -->\n\n",
            "[00:14] **Other:** Too short. <!-- en parakeet-v3 -->\n\n",
            "[120:00] **You:** Koett. <!-- en parakeet-v3 -->\n",
        )
    );
    let history = history_text(&output.markdown).unwrap();
    assert!(!history.contains("<!--"));
    assert!(!history.contains("koett_id"));
    assert!(history.contains("[120:00] **You:** Koett."));
    assert_eq!(
        crate::meeting::bundle::sha256(history.as_bytes()),
        output.publication.history.text_sha256
    );
}

#[test]
fn splits_turns_on_pause_and_length() {
    use Channel::You;
    let points = (0..=61)
        .chain([64])
        .map(|s| word(You, s, s + 1, "word", "en", PARAKEET))
        .collect();
    let entries = timeline::merge(points);
    let lengths = entries
        .iter()
        .map(|entry| {
            let timeline::Entry::Turn(turn) = entry else {
                panic!("expected turn")
            };
            turn.words.len()
        })
        .collect::<Vec<_>>();
    assert_eq!(lengths, [60, 2, 1]);
}

fn window() -> Window {
    Window {
        id: WindowId {
            channel: Channel::You,
            sequence: 0,
        },
        first: second(5),
        core: Span {
            first: Offset(second(5).0 + 4800),
            end: Offset(second(7).0 - 4800),
        },
        samples: vec![0.; 32000],
    }
}

fn result(window: &Window, times: &[(f32, f32)]) -> DecodedWindow {
    DecodedWindow {
        id: window.id,
        words: times
            .iter()
            .map(|&(start, end)| TimedWord {
                text: "word".into(),
                start,
                end,
            })
            .collect(),
        lang: "sv".into(),
        backend: PARAKEET.into(),
    }
}

#[test]
fn rejects_nonfinite_reversed_outside_or_decreasing_timestamps() {
    let window = window();
    for invalid in [
        vec![(f32::NAN, 1.0)],
        vec![(0.0, f32::INFINITY)],
        vec![(-0.1, 0.2)],
        vec![(0.0, 2.06)],
        vec![(1.0, 0.9)],
        vec![(0.8, 0.9), (0.7, 1.0)],
    ] {
        assert!(
            validate(
                std::slice::from_ref(&window),
                vec![result(&window, &invalid)]
            )
            .is_err(),
            "{invalid:?}"
        );
    }
}

#[test]
fn rejects_missing_duplicate_and_unexpected_results() {
    let window = window();
    let windows = std::slice::from_ref(&window);
    assert!(validate(windows, vec![]).is_err());
    assert!(validate(windows, vec![result(&window, &[]), result(&window, &[])]).is_err());
    let mut extra = result(&window, &[]);
    extra.id.sequence += 1;
    assert!(validate(windows, vec![extra, result(&window, &[])]).is_err());
}

#[test]
fn clips_context_by_midpoint_and_converts_retained_words_to_meeting_frames() {
    let window = window();
    let results = vec![result(
        &window,
        &[(0.1, 0.2), (0.2, 0.5), (1.6, 1.9), (1.9, 2.03)],
    )];
    let validated = validate(&[window], results).unwrap();
    assert_eq!(validated[0].words.len(), 1);
    assert_eq!(
        validated[0].words[0].span,
        Span {
            first: Offset(83200),
            end: Offset(88000)
        }
    );
}

#[test]
fn vault_note_keeps_a_mark_in_an_absolute_file_url() {
    let mark = MarkRef {
        id: 1,
        offset: second(1),
        relative_path: "assets/mark-0000016000-0001.png".into(),
    };
    let manifest = manifest();
    let rendered = markdown::render(
        &manifest,
        second(3),
        vec![timeline::Entry::Mark(mark)],
        &[],
        false,
    )
    .unwrap();
    let (name, note) = vault_note(
        &manifest,
        second(3),
        &rendered.publication,
        &rendered.markdown,
        std::path::Path::new("/tmp/Koett meetings/#1"),
    )
    .unwrap();
    assert_eq!(name, "1970-01-01 0000 Koett meeting 550e8400.md");
    assert!(note.contains(
        "[00:01] ![Mark 1](<file:///tmp/Koett%20meetings/%231/assets/mark-0000016000-0001.png>)"
    ));
}

fn ms_word(channel: Channel, first_ms: u64, end_ms: u64, text: &str) -> Point {
    Point::Word(SpeechWord {
        channel,
        span: Span {
            first: Offset(first_ms * 16),
            end: Offset(end_ms * 16),
        },
        text: text.into(),
        lang: "und".into(),
        backend: PARAKEET.into(),
    })
}

fn outline(entries: &[timeline::Entry]) -> Vec<String> {
    entries
        .iter()
        .map(|entry| match entry {
            timeline::Entry::Turn(turn) => format!(
                "{}@{}:{}",
                match turn.speaker {
                    timeline::Speaker::You => "You",
                    timeline::Speaker::Other(_) => "Other",
                },
                turn.span.first.0 / 16,
                turn.words
                    .iter()
                    .map(|word| word.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            timeline::Entry::Mark(mark) => format!("Mark@{}", mark.offset.0 / 16),
            timeline::Entry::MissingAudio { .. } => "Missing".into(),
        })
        .collect()
}

#[test]
fn review_crosstalk_keeps_one_turn_per_side() {
    use Channel::{Other, You};
    let mut points = Vec::new();
    for i in 0..25 {
        points.push(ms_word(You, i * 400, i * 400 + 350, "y"));
        points.push(ms_word(Other, i * 400 + 200, i * 400 + 550, "o"));
    }
    let entries = timeline::merge(points);
    assert_eq!(
        outline(&entries),
        [
            format!("You@0:{}", vec!["y"; 25].join(" ")),
            format!("Other@200:{}", vec!["o"; 25].join(" ")),
        ]
    );
}

#[test]
fn review_exchange_in_a_pause_keeps_conversation_order() {
    use Channel::{Other, You};
    let points = vec![
        ms_word(You, 0, 500, "Do"),
        ms_word(You, 600, 1000, "you"),
        ms_word(You, 1100, 2000, "see?"),
        ms_word(Other, 2300, 2600, "Yes."),
        ms_word(You, 3000, 3400, "Click"),
        ms_word(You, 3500, 3800, "it."),
    ];
    assert_eq!(
        outline(&timeline::merge(points)),
        ["You@0:Do you see?", "Other@2300:Yes.", "You@3000:Click it."]
    );
}

#[test]
fn review_backchannel_under_speech_does_not_split_the_speaker() {
    use Channel::{Other, You};
    let points = vec![
        ms_word(You, 0, 400, "one"),
        ms_word(You, 450, 900, "two"),
        ms_word(Other, 900, 1000, "mm"),
        ms_word(You, 950, 1400, "three"),
    ];
    assert_eq!(
        outline(&timeline::merge(points)),
        ["You@0:one two three", "Other@900:mm"]
    );
}

#[test]
fn review_marks_own_no_words_during_overlap_and_crossing_words_stay() {
    use Channel::{Other, You};
    let mark = |ms: u64| {
        Point::Mark(MarkRef {
            id: 1,
            offset: Offset(ms * 16),
            relative_path: "assets/m.png".into(),
        })
    };
    let points = vec![
        ms_word(You, 0, 400, "a"),
        ms_word(Other, 100, 500, "b"),
        ms_word(You, 4800, 5300, "crossing"),
        ms_word(Other, 4900, 5000, "c"),
        mark(5000),
        ms_word(You, 5000, 5400, "tie"),
        ms_word(Other, 5000, 5400, "tie2"),
        ms_word(You, 5500, 5900, "d"),
    ];
    assert_eq!(
        outline(&timeline::merge(points)),
        [
            "You@0:a",
            "Other@100:b",
            "You@4800:crossing",
            "Other@4900:c",
            "Mark@5000",
            "You@5000:tie d",
            "Other@5000:tie2",
        ]
    );
}

#[test]
fn a_whole_word_in_a_pause_splits_a_turn_even_if_the_other_track_then_gaps() {
    use Channel::{Other, You};
    let points = vec![
        ms_word(You, 0, 400, "question"),
        ms_word(Other, 500, 700, "yes"),
        Point::MissingAudio {
            channel: Other,
            span: span(1, 3),
        },
        ms_word(You, 1100, 1400, "answer"),
    ];
    assert_eq!(
        outline(&timeline::merge(points)),
        [
            "You@0:question",
            "Other@500:yes",
            "Missing",
            "You@1100:answer"
        ]
    );
}

#[test]
fn a_gap_only_splits_its_own_track() {
    use Channel::{Other, You};
    let points = vec![
        ms_word(You, 800, 1000, "one"),
        ms_word(Other, 850, 1000, "before"),
        Point::MissingAudio {
            channel: Other,
            span: span(1, 2),
        },
        ms_word(You, 1100, 1400, "two"),
        ms_word(Other, 2000, 2200, "after"),
    ];
    assert_eq!(
        outline(&timeline::merge(points)),
        [
            "You@800:one two",
            "Other@850:before",
            "Missing",
            "Other@2000:after"
        ]
    );
}
