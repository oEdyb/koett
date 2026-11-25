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
