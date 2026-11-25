use super::*;
use crate::meeting::{Offset, SessionInfo};
use std::path::PathBuf;

const TEXT: &str = "# Meeting · 2026-11-25 14:05\n\n[00:02] ![Mark 1](assets/mark-0000032000-0001.png)\n\n[00:38] **Other:** Den blå knappen &lt;b> &amp; inställningarna.\n\n*[01:02–01:40] No You audio*\n\n[01:41] **You:** I see it now.\n";

fn summary(id: &str, created_at_ms: i64, preview: &str) -> HistorySummary {
    HistorySummary {
        id: id.into(),
        created_at_ms,
        model: "parakeet-v3".into(),
        preview: preview.into(),
    }
}

fn listed(started_at_ms: u64, state: Listing, recoverable: bool) -> Listed {
    let id = MeetingId::new();
    let folder = PathBuf::from(format!("/meetings/{id}"));
    Listed {
        info: SessionInfo { id, started_at_ms },
        duration: Some(Offset(16_000 * 90)),
        folder,
        state,
        recoverable,
    }
}

fn transcribed(started_at_ms: u64, fallback: bool) -> Listed {
    let mut meeting = listed(
        started_at_ms,
        Listing::NotTranscribed(RecoveryReason::Interrupted),
        fallback,
    );
    meeting.state = Listing::Transcribed(Artifact {
        markdown: meeting.folder.join("transcript.md"),
        assets: vec![meeting.folder.join("assets/mark-0000032000-0001.png")],
        backends: Default::default(),
        fallback,
        audio_retained: fallback,
    });
    meeting
}

fn indexed_row(meeting: &Listed) -> HistorySummary {
    summary(
        &format!("meeting:{}", meeting.info.id),
        meeting.info.started_at_ms as i64,
        TEXT,
    )
}

fn meeting_row(row: &Row) -> &MeetingRow {
    match row {
        Row::Meeting(meeting) => meeting,
        Row::Dictation(summary) => panic!("{} is a dictation", summary.id),
    }
}

#[test]
fn a_meeting_row_offers_its_folder_and_never_a_paste_or_a_dictation_action() {
    let published = transcribed(5_000, false);
    let rows = page_rows(
        vec![
            indexed_row(&published),
            summary("dictation-1", 4_000, "Hello"),
        ],
        vec![published.clone()],
        "",
        None,
        None,
        |_| true,
    );

    assert_eq!(
        rows[1].actions(),
        [Action::Copy, Action::Export, Action::FixWord]
    );
    assert_eq!(
        rows[0].actions(),
        [Action::Copy, Action::OpenTranscript, Action::OpenFolder]
    );
    assert_eq!(meeting_row(&rows[0]).listed.as_ref(), Some(&published));
    assert_eq!(
        meeting_row(&rows[0]).excerpt(),
        Some(Excerpt::Speech(
            "Other: Den blå knappen <b> & inställningarna. You: I see it now.".into()
        ))
    );
    for row in &rows {
        if let Row::Meeting(_) = row {
            assert!(
                !row.actions()
                    .iter()
                    .any(|action| matches!(action, Action::Export | Action::FixWord)),
                "a meeting row is not edited or exported as a dictation"
            );
        }
    }
}

#[test]
fn meetings_without_text_join_the_page_they_started_in() {
    let held = listed(
        3_000,
        Listing::NotTranscribed(RecoveryReason::HeldByUser),
        true,
    );
    let older = listed(
        500,
        Listing::NotTranscribed(RecoveryReason::Interrupted),
        true,
    );
    let newer = listed(
        9_000,
        Listing::NotTranscribed(RecoveryReason::DecodeFailed("x".into())),
        true,
    );
    let page = vec![summary("b", 4_000, "first"), summary("a", 2_000, "second")];
    let next = HistoryCursor {
        created_at_ms: 2_000,
        id: "a".into(),
    };
    let before = HistoryCursor {
        created_at_ms: 8_000,
        id: "c".into(),
    };

    let rows = page_rows(
        page,
        vec![newer, held.clone(), older],
        "",
        Some(&before),
        Some(&next),
        |_| false,
    );

    assert_eq!(
        rows.iter().map(Row::id).collect::<Vec<_>>(),
        ["b".into(), format!("meeting:{}", held.info.id), "a".into()]
    );
    let held_row = meeting_row(&rows[1]);
    assert_eq!(held_row.excerpt(), Some(Excerpt::Status("Not transcribed")));
    assert_eq!(rows[1].actions(), [Action::OpenFolder, Action::Transcribe]);
    assert_eq!(
        held_row.transcribe(&MeetingView::Idle),
        ("Transcribe", true)
    );
}

#[test]
fn a_search_shows_only_what_history_matched() {
    let held = listed(
        3_000,
        Listing::NotTranscribed(RecoveryReason::HeldByUser),
        true,
    );
    let rows = page_rows(
        vec![summary("b", 4_000, "first")],
        vec![held],
        "first",
        None,
        None,
        |_| false,
    );
    assert_eq!(rows.iter().map(Row::id).collect::<Vec<_>>(), ["b"]);
}

#[test]
fn a_folder_history_already_indexed_is_not_listed_twice() {
    let lost_transcript = listed(
        3_000,
        Listing::NotTranscribed(RecoveryReason::Interrupted),
        true,
    );
    let id = format!("meeting:{}", lost_transcript.info.id);
    let rows = page_rows(Vec::new(), vec![lost_transcript], "", None, None, |row| {
        row == id
    });
    assert!(rows.is_empty());
}

#[test]
fn a_meeting_row_opens_with_its_first_turns_and_says_when_nobody_spoke() {
    let words = "[00:01] **You:** However, <!-- x -->\n\n[00:02] **You:** that is not true.\n\n[00:03] **Other:** Men det stämmer.\n";
    let row = |preview: &str| MeetingRow {
        id: MeetingId::new(),
        started_at_ms: 1,
        summary: Some(summary("meeting:x", 1, preview)),
        listed: None,
    };
    assert_eq!(
        row(&words.replace(" <!-- x -->", "")).excerpt(),
        Some(Excerpt::Speech(
            "You: However, that is not true. Other: Men det stämmer.".into()
        ))
    );
    assert_eq!(
        row("[00:01] **You:** However, that is not true.\n\n[00:03] **Other:** Men det st")
            .excerpt(),
        Some(Excerpt::Speech("You: However, that is not true.".into())),
        "History's 240-character preview cut the last turn"
    );
    assert_eq!(
        row("[00:01] **You:** However, that is not tr").excerpt(),
        Some(Excerpt::Speech("You: However, that is not tr".into()))
    );
    assert_eq!(
        row("# Meeting · 2026-11-25 14:05\n\n[00:02] ![Mark 1](assets/mark.png)\n").excerpt(),
        Some(Excerpt::Status("No speech"))
    );
}

#[test]
fn a_fallback_transcript_can_be_transcribed_again_once_the_meeting_is_idle() {
    let fallback = transcribed(5_000, true);
    let rows = page_rows(
        vec![indexed_row(&fallback)],
        vec![fallback.clone()],
        "",
        None,
        None,
        |_| true,
    );
    let row = meeting_row(&rows[0]);
    assert_eq!(
        rows[0].actions(),
        [
            Action::Copy,
            Action::OpenTranscript,
            Action::OpenFolder,
            Action::Transcribe
        ]
    );
    assert_eq!(
        row.transcribe(&MeetingView::Idle),
        ("Transcribe again", true)
    );
    let processing = MeetingView::Processing {
        info: fallback.info.clone(),
        done: 0.0,
    };
    assert_eq!(row.transcribe(&processing), ("Transcribing…", false));
    let other = MeetingView::Processing {
        info: SessionInfo {
            id: MeetingId::new(),
            started_at_ms: 1,
        },
        done: 0.0,
    };
    assert_eq!(row.transcribe(&other), ("Transcribe again", false));
}

#[test]
fn a_meeting_whose_folder_is_gone_still_copies_its_history_text() {
    let gone = transcribed(5_000, false);
    let rows = page_rows(vec![indexed_row(&gone)], Vec::new(), "", None, None, |_| {
        true
    });
    assert_eq!(rows[0].actions(), [Action::Copy]);
}

#[test]
fn the_reading_pane_drops_markdown_but_keeps_every_word() {
    assert_eq!(
        reading(TEXT),
        [
            Reading::Mark {
                time: "00:02".into(),
                name: "Mark 1".into()
            },
            Reading::Turn {
                time: "00:38".into(),
                speaker: "Other".into(),
                text: "Den blå knappen <b> & inställningarna.".into()
            },
            Reading::Missing {
                span: "01:02–01:40".into(),
                what: "No You audio".into()
            },
            Reading::Turn {
                time: "01:41".into(),
                speaker: "You".into(),
                text: "I see it now.".into()
            },
        ]
    );
    assert_eq!(
        reading("loose line &amp; more"),
        [Reading::Other("loose line & more".into())]
    );
}

#[test]
fn copy_finds_each_receipt_listed_mark_relative_to_the_folder() {
    let meeting = transcribed(5_000, false);
    let Listing::Transcribed(artifact) = &meeting.state else {
        unreachable!()
    };
    assert_eq!(
        relative_assets(&meeting.folder, artifact),
        ["assets/mark-0000032000-0001.png"]
    );
}
