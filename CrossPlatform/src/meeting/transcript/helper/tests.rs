use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::Ordering;
use std::thread;

use super::*;
use crate::meeting::bundle::records::Manifest;
use crate::meeting::transcript::{Decoder, MeetingDecoder, PARAKEET, WordOwnership};
use crate::meeting::{Backend, Offset, SessionInfo, Span};

const STUB: &str = include_str!("stub.py");

struct LocalDecoder;

impl Decoder for LocalDecoder {
    fn decode(&mut self, windows: &[Window]) -> Result<Vec<DecodedWindow>, String> {
        Ok(windows
            .iter()
            .map(|window| DecodedWindow {
                id: window.id,
                words: Vec::new(),
                lang: "und".into(),
                backend: PARAKEET.into(),
            })
            .collect())
    }
}

fn fixture(mode: &str) -> (Scratch, Bundle, PathBuf) {
    let id = MeetingId::new();
    let root = std::env::temp_dir().join(format!("koett-asr-test-{id}"));
    DirBuilder::new().mode(0o700).create(&root).unwrap();
    let bundle = Bundle::create(
        &root,
        Manifest {
            info: SessionInfo {
                id,
                started_at_ms: 0,
            },
            mic: None,
            backend_pref: Backend::Gpu,
            keep_audio: true,
            export_dir: None,
        },
    )
    .unwrap();
    let python = root.join(mode);
    fs::write(&python, STUB).unwrap();
    fs::set_permissions(&python, fs::Permissions::from_mode(0o700)).unwrap();
    (Scratch(root), bundle, python)
}

fn windows(sequences: std::ops::Range<u64>) -> Vec<Window> {
    sequences
        .map(|sequence| Window {
            id: WindowId {
                channel: Channel::You,
                sequence,
            },
            first: Offset(0),
            core: Span {
                first: Offset(0),
                end: Offset(16_000),
            },
            ownership: WordOwnership::default(),
            samples: vec![0.; 16_000],
        })
        .collect()
}

fn decoder<'a>(helper: Helper, cancelled: &'a AtomicBool) -> MeetingDecoder<'a, LocalDecoder> {
    MeetingDecoder {
        helper: Some(helper),
        fallback: LocalDecoder,
        cancelled,
        batch_base: Duration::from_millis(700),
        per_window: Duration::ZERO,
    }
}

fn backends(results: &[DecodedWindow]) -> Vec<&str> {
    results
        .iter()
        .map(|result| result.backend.as_str())
        .collect()
}

fn assert_cleaned(bundle: &Bundle) {
    assert!(!bundle.directory().join("job").exists());
    let pgid = fs::read_to_string(bundle.directory().join("helper.pgid")).unwrap();
    let status = Command::new("pgrep").args(["-g", &pgid]).status().unwrap();
    assert_eq!(status.code(), Some(1));
}

#[test]
fn helper_failure_keeps_valid_prefix_and_falls_back_for_rest_of_job() {
    for mode in [
        "crash",
        "hang",
        "broken-timestamps",
        "error",
        "duplicate",
        "wrong-backend",
        "unknown-language",
    ] {
        let (_root, bundle, python) = fixture(mode);
        let cancelled = AtomicBool::new(false);
        let helper = Helper::start(&bundle, &python, &cancelled).unwrap();
        let mut decoder = decoder(helper, &cancelled);
        let first = decoder.decode(&windows(0..4)).unwrap();
        assert_eq!(
            backends(&first),
            [SWEDISH, SWEDISH, PARAKEET, PARAKEET],
            "{mode}"
        );
        assert_cleaned(&bundle);
        let later = decoder.decode(&windows(4..6)).unwrap();
        assert_eq!(backends(&later), [PARAKEET, PARAKEET], "{mode}");
        assert_cleaned(&bundle);
    }
}

#[test]
fn startup_failure_removes_job_and_reaps_child() {
    let (_root, bundle, python) = fixture("no-ready");
    assert!(Helper::start(&bundle, &python, &AtomicBool::new(false)).is_err());
    assert_cleaned(&bundle);
}

#[test]
fn split_reply_survives_intermediate_read_timeout_and_job_files_are_removed() {
    let (_root, bundle, python) = fixture("split");
    let cancelled = AtomicBool::new(false);
    let mut decoder = decoder(
        Helper::start(&bundle, &python, &cancelled).unwrap(),
        &cancelled,
    );
    let results = decoder.decode(&windows(0..4)).unwrap();
    assert_eq!(backends(&results), [SWEDISH; 4]);
    let paths = fs::read_dir(bundle.directory().join("job"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(paths, ["asr_helper.py".into(), "helper.log".into()].into());
    drop(decoder);
    assert_cleaned(&bundle);
}

#[test]
fn cancelling_a_stalled_batch_ends_decode_and_reaps_child() {
    let (_root, bundle, python) = fixture("hang");
    let cancelled = AtomicBool::new(false);
    let helper = Helper::start(&bundle, &python, &cancelled).unwrap();
    let mut decoder = decoder(helper, &cancelled);
    decoder.batch_base = Duration::from_secs(8);
    thread::scope(|scope| {
        scope.spawn(|| {
            thread::sleep(Duration::from_millis(100));
            cancelled.store(true, Ordering::Release);
        });
        assert!(
            matches!(decoder.decode(&windows(0..4)), Err(error) if error.contains("cancelled"))
        );
    });
    assert_cleaned(&bundle);
}
