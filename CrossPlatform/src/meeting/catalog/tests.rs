use std::fs;
use std::time::{Duration, SystemTime};

use super::super::bundle::Bundle;
use super::super::bundle::records::{HistoryReceipt, Intent, Manifest, Publication, Seal};
use super::super::{Backend, CloseGoal, MeetingId, TrackSeal, Tracks};
use super::*;

struct TestRoot(PathBuf);

impl TestRoot {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("koett-catalog-test-{}", MeetingId::new())))
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn create(root: &Path, started_at_ms: u64) -> Bundle {
    Bundle::create(
        root,
        Manifest {
            info: SessionInfo {
                id: MeetingId::new(),
                started_at_ms,
            },
            mic: None,
            backend_pref: Backend::Parakeet,
            keep_audio: false,
            export_dir: None,
        },
    )
    .unwrap()
}

fn seal(bundle: &Bundle, seconds: u64, goal: CloseGoal) {
    let cutoff = Offset(seconds * u64::from(SAMPLE_RATE));
    let track = TrackSeal {
        valid_bytes: 12,
        captured_until: cutoff,
    };
    bundle.write_intent(&Intent { cutoff, goal }).unwrap();
    bundle
        .write_seal(&Seal {
            cutoff,
            tracks: Tracks {
                you: track,
                other: track,
            },
            mark_issues: Vec::new(),
        })
        .unwrap();
}

fn publish(bundle: &Bundle) {
    publish_markdown(bundle, "---\ntype: meeting-transcript\n---\n\n# Meeting\n");
}

fn publish_markdown(bundle: &Bundle, markdown: &str) {
    bundle
        .publish(
            markdown,
            Publication {
                history: HistoryReceipt {
                    id: format!("meeting:{}", bundle.manifest().info.id),
                    created_at_ms: bundle.manifest().info.started_at_ms,
                    model: "parakeet-v3".into(),
                    text_sha256: String::new(),
                },
                assets: vec!["assets/mark-0000016000-0001.png".into()],
                backends: [("parakeet-v3".into(), 1)].into_iter().collect(),
                fallback: false,
            },
        )
        .unwrap();
}

fn crash(bundle: &Bundle, after: Duration) {
    let written = SystemTime::UNIX_EPOCH
        + Duration::from_millis(bundle.manifest().info.started_at_ms)
        + after;
    for name in ["you.journal", "other.journal"] {
        let path = bundle.directory().join("audio").join(name);
        fs::write(&path, b"KOETTJNL").unwrap();
    }
    fs::File::options()
        .write(true)
        .open(bundle.directory().join("audio/other.journal"))
        .unwrap()
        .set_modified(written)
        .unwrap();
    fs::File::options()
        .write(true)
        .open(bundle.directory().join("audio/you.journal"))
        .unwrap()
        .set_modified(written - Duration::from_secs(4))
        .unwrap();
}

fn seconds(meeting: &Listed) -> Option<u64> {
    meeting
        .duration
        .map(|offset| offset.0 / u64::from(SAMPLE_RATE))
}

#[test]
fn lists_finished_held_and_crashed_meetings_newest_first_without_the_live_one() {
    let root = TestRoot::new();
    let transcribed = create(&root.0, 1_000_000);
    seal(&transcribed, 3_130, CloseGoal::Finish);
    publish(&transcribed);
    let held = create(&root.0, 2_000_000);
    seal(&held, 75, CloseGoal::Hold);
    let crashed = create(&root.0, 3_000_000);
    crash(&crashed, Duration::from_secs(312));
    let ids = [&transcribed, &held, &crashed].map(|bundle| bundle.manifest().info.id);
    drop((transcribed, held, crashed));
    let live = create(&root.0, 4_000_000);

    let meetings = list(&root.0).unwrap();

    assert_eq!(
        meetings
            .iter()
            .map(|meeting| (meeting.info.id, seconds(meeting), meeting.recoverable))
            .collect::<Vec<_>>(),
        [
            (ids[2], Some(312), true),
            (ids[1], Some(75), true),
            (ids[0], Some(3_130), false),
        ]
    );
    assert_eq!(
        meetings[1].state,
        Listing::NotTranscribed(RecoveryReason::HeldByUser)
    );
    assert_eq!(
        meetings[0].state,
        Listing::NotTranscribed(RecoveryReason::Interrupted)
    );
    let Listing::Transcribed(artifact) = &meetings[2].state else {
        panic!("a published meeting lists its transcript");
    };
    assert_eq!(meetings[2].folder, root.0.join(ids[0].to_string()));
    assert_eq!(artifact.markdown, meetings[2].folder.join("transcript.md"));
    assert_eq!(
        artifact.assets,
        [meetings[2].folder.join("assets/mark-0000016000-0001.png")]
    );
    assert_eq!(
        recoverable(&root.0)
            .unwrap()
            .iter()
            .map(|meeting| meeting.info.id)
            .collect::<Vec<_>>(),
        [ids[2], ids[1]]
    );
    assert!(
        !meetings
            .iter()
            .any(|meeting| meeting.info.id == live.manifest().info.id),
        "the live meeting belongs to the tray"
    );
}

#[test]
fn a_meeting_that_crashed_before_any_audio_has_no_duration() {
    let root = TestRoot::new();
    drop(create(&root.0, 5_000));

    let meetings = list(&root.0).unwrap();

    assert_eq!(meetings.len(), 1);
    assert_eq!(meetings[0].duration, None);
    assert!(meetings[0].recoverable);
}

#[test]
fn copy_gives_the_committed_transcript_with_absolute_marks_even_while_it_is_transcribed_again() {
    let root = TestRoot::new();
    let meeting = create(&root.0, 1_000_000);
    seal(&meeting, 62, CloseGoal::Finish);
    let id = meeting.manifest().info.id;
    let folder = root.0.join(id.to_string());
    let markdown = "---\ntype: meeting-transcript\n---\n\n# Meeting\n\n[00:01] ![Mark 1](assets/mark-0000016000-0001.png)\n\n[00:02] **You:** See [notes](assets/notes.md).\n";
    publish_markdown(&meeting, markdown);

    assert!(
        list(&root.0).unwrap().is_empty(),
        "the lock hides a meeting while it is transcribed again"
    );
    let copied = copy_text(&root.0, id).unwrap().unwrap();

    let mark = url::Url::from_file_path(folder.join("assets/mark-0000016000-0001.png")).unwrap();
    assert_eq!(
        copied,
        markdown.replace("(assets/mark-0000016000-0001.png)", &format!("(<{mark}>)"))
    );
    assert!(copied.starts_with("---\ntype: meeting-transcript\n"));

    fs::write(folder.join("transcript.md"), "edited by hand\n").unwrap();
    assert_eq!(
        copy_text(&root.0, id).unwrap_err().kind(),
        io::ErrorKind::InvalidData,
        "a transcript that no longer matches its receipt is not copied as committed"
    );

    drop(meeting);
    fs::remove_dir_all(&folder).unwrap();
    assert_eq!(copy_text(&root.0, id).unwrap(), None);
}
