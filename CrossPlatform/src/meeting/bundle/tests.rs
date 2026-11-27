use std::os::unix::fs::PermissionsExt;
use std::thread;
use std::time::{Duration, Instant};

use super::files::WriteBoundary::{self, AfterRename, BeforeRename};
use super::marks::{MarkRef, list_marks};
use super::records::{ExportProjection, HistoryProjection, HistoryReceipt, JobError, MarkIssue};
use super::scan::{DiskState, scan, scan_all};
use super::*;
use crate::meeting::InterruptCause;

struct TestRoot(PathBuf);

impl TestRoot {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("koett-bundle-test-{}", MeetingId::new())))
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn manifest() -> Manifest {
    Manifest {
        info: SessionInfo {
            id: MeetingId::new(),
            started_at_ms: 1_764_000_312_000,
        },
        mic: None,
        backend_pref: Backend::Parakeet,
        keep_audio: false,
        export_dir: None,
    }
}

fn sample_seal() -> Seal {
    Seal {
        cutoff: Offset(80_000),
        tracks: Tracks {
            you: TrackSeal {
                valid_bytes: 30,
                captured_until: Offset(40_000),
            },
            other: TrackSeal {
                valid_bytes: 40,
                captured_until: Offset(50_000),
            },
        },
        mark_issues: Vec::new(),
    }
}

fn history(manifest: &Manifest) -> HistoryReceipt {
    HistoryReceipt {
        id: format!("meeting:{}", manifest.info.id),
        created_at_ms: manifest.info.started_at_ms,
        model: "parakeet-v3".into(),
        text_sha256: sha256(b"You: Hej"),
    }
}

fn publish(bundle: &Bundle, markdown: &str, fallback: bool) -> Result<Receipt, WriteError> {
    bundle.publish(
        markdown,
        Publication {
            history: history(bundle.manifest()),
            assets: Vec::new(),
            backends: [("parakeet-v3".into(), 1)].into_iter().collect(),
            fallback,
        },
    )
}

const PROJECTED: Projections = Projections {
    history: HistoryProjection::Indexed,
    export: ExportProjection::Disabled,
};

fn write_until_failure(
    root: &Path,
    manifest: Manifest,
    file: &'static str,
    point: WriteBoundary,
) -> io::Result<()> {
    let bundle = Bundle::create_with_writer(
        root,
        manifest,
        AtomicWriter {
            fault: Some(Box::new(move |name, stage| {
                if (name, stage) == (file, point) {
                    Err(io::Error::other(file))
                } else {
                    Ok(())
                }
            })),
        },
    )?;
    fs::write(bundle.directory().join("audio/you.journal"), b"record")?;
    bundle.write_intent(&Intent {
        cutoff: Offset(80_000),
        goal: CloseGoal::Hold,
    })?;
    bundle.write_seal(&sample_seal())?;
    publish(&bundle, "# Hej\n", false)?;
    bundle.write_projections(&PROJECTED)?;
    Ok(())
}

#[test]
fn scan_follows_the_last_committed_file_at_every_failed_write() {
    use RecoveryReason::{HeldByUser, Interrupted};

    for (file, before, after) in [
        (INTENT, "interrupted", "interrupted"),
        (SEAL, "interrupted", "held"),
        (RECEIPT, "held", "held"),
        (TRANSCRIPT, "held", "complete_pending"),
        (PROJECTIONS, "complete_pending", "complete_indexed"),
    ] {
        for (point, expected) in [(BeforeRename, before), (AfterRename, after)] {
            let root = TestRoot::new();
            let manifest = manifest();
            let id = manifest.info.id;
            let error = write_until_failure(&root.0, manifest, file, point).unwrap_err();
            assert!(error.to_string().contains(file));
            let state = scan(&root.0, id).unwrap().unwrap();
            let observed = match state {
                DiskState::Recoverable {
                    seal: None,
                    reason: Interrupted,
                    ..
                } => "interrupted",
                DiskState::Recoverable {
                    seal: Some(_),
                    reason: HeldByUser,
                    ..
                } => "held",
                DiskState::Complete { projections, .. } if projections == PROJECTED => {
                    "complete_indexed"
                }
                DiskState::Complete { projections, .. }
                    if projections.history == HistoryProjection::Pending =>
                {
                    "complete_pending"
                }
                other => panic!("unexpected state at {file}: {other:?}"),
            };
            assert_eq!(observed, expected, "fault during {file}");
        }
    }
}

#[test]
fn failed_manifest_write_removes_the_new_bundle_for_retry() {
    for point in [BeforeRename, AfterRename] {
        let root = TestRoot::new();
        let manifest = manifest();
        let id = manifest.info.id;
        let error = write_until_failure(&root.0, manifest.clone(), MANIFEST, point).unwrap_err();
        assert!(error.to_string().contains(MANIFEST));
        assert_eq!(scan(&root.0, id).unwrap(), None);
        assert!(Bundle::create(&root.0, manifest).is_ok());
    }
}

#[test]
fn another_handle_holding_the_lock_is_active() {
    let root = TestRoot::new();
    let manifest = manifest();
    let id = manifest.info.id;
    let created = Bundle::create(&root.0, manifest.clone()).unwrap();
    let locked_path = created.directory().join(LOCK);
    drop(created);

    let other_handle = File::open(locked_path).unwrap();
    other_handle.try_lock().unwrap();
    assert_eq!(
        scan(&root.0, id).unwrap(),
        Some(DiskState::Active {
            id,
            manifest: Some(manifest)
        })
    );
    assert!(
        matches!(Bundle::open(&root.0, id), Err(error) if error.kind() == io::ErrorKind::WouldBlock)
    );
    drop(other_handle);
    assert!(matches!(
        scan(&root.0, id).unwrap(),
        Some(DiskState::Recoverable {
            reason: RecoveryReason::Interrupted,
            ..
        })
    ));
    assert!(Bundle::open(&root.0, id).is_ok());
}

#[test]
fn bundle_can_open_while_scan_reads_the_transcript() {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    use std::process::Command;

    let root = TestRoot::new();
    let manifest = manifest();
    let id = manifest.info.id;
    let bundle = Bundle::create(&root.0, manifest).unwrap();
    let markdown = "# Hej\n";
    publish(&bundle, markdown, false).unwrap();
    let transcript = bundle.directory().join(TRANSCRIPT);
    drop(bundle);
    fs::remove_file(&transcript).unwrap();
    assert!(
        Command::new("mkfifo")
            .arg(&transcript)
            .status()
            .unwrap()
            .success()
    );

    let scan_root = root.0.clone();
    let scanner = thread::spawn(move || scan(&scan_root, id).unwrap());
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut transcript = loop {
        match OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&transcript)
        {
            Ok(file) => break file,
            Err(error)
                if error.raw_os_error() == Some(libc::ENXIO) && Instant::now() < deadline =>
            {
                thread::sleep(Duration::from_millis(2));
            }
            Err(error) => panic!("scan did not open the transcript: {error}"),
        }
    };
    let opened = Bundle::open(&root.0, id);
    transcript.write_all(markdown.as_bytes()).unwrap();
    drop(transcript);
    assert!(matches!(
        scanner.join().unwrap(),
        Some(DiskState::Complete { .. })
    ));
    opened.unwrap();
}

#[test]
fn a_bundle_is_private_created_once_and_has_flat_v1_records() {
    let root = TestRoot::new();
    let manifest = manifest();
    let bundle = Bundle::create(&root.0, manifest.clone()).unwrap();
    let directory = bundle.directory();
    for directory in [&root.0, directory, &directory.join("audio")] {
        assert_eq!(
            directory.metadata().unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    let duplicate = Bundle::create(&root.0, manifest.clone());
    assert!(matches!(duplicate, Err(error) if error.kind() == io::ErrorKind::AlreadyExists));
    assert_eq!(
        read_json::<serde_json::Value>(&directory.join(MANIFEST)).unwrap(),
        serde_json::json!({
            "version": 1, "id": manifest.info.id.to_string(), "started_at_ms": 1764000312000_u64,
            "mic": null, "backend_pref": "parakeet", "keep_audio": false, "export_dir": null,
        })
    );
    bundle
        .write_intent(&Intent {
            cutoff: Offset(80_000),
            goal: CloseGoal::Finish,
        })
        .unwrap();
    assert_eq!(
        read_json::<serde_json::Value>(&directory.join(INTENT)).unwrap(),
        serde_json::json!({
            "goal": "finish", "cutoff": 80000,
        })
    );
    bundle.write_seal(&sample_seal()).unwrap();
    assert_eq!(
        read_json::<serde_json::Value>(&directory.join(SEAL)).unwrap(),
        serde_json::json!({
            "cutoff": 80000, "you": {"valid_bytes": 30, "captured_until": 40000},
            "other": {"valid_bytes": 40, "captured_until": 50000}, "mark_issues": [],
        })
    );
}

#[test]
fn job_directory_is_private_and_can_be_reopened() {
    let root = TestRoot::new();
    let bundle = Bundle::create(&root.0, manifest()).unwrap();
    let job = bundle.create_job_directory().unwrap();
    assert_eq!(job, bundle.directory().join("job"));
    assert_eq!(job.metadata().unwrap().permissions().mode() & 0o777, 0o700);
    assert_eq!(bundle.create_job_directory().unwrap(), job);
}

#[test]
fn scan_preserves_the_interrupted_intents_typed_reason() {
    for (cause, expected) in [
        (InterruptCause::Shutdown, RecoveryReason::Interrupted),
        (
            InterruptCause::CaptureFault("mic disconnected".into()),
            RecoveryReason::CaptureFailed("mic disconnected".into()),
        ),
        (
            InterruptCause::IntentUncertain("intent sync failed".into()),
            RecoveryReason::Interrupted,
        ),
    ] {
        let root = TestRoot::new();
        let manifest = manifest();
        let id = manifest.info.id;
        let bundle = Bundle::create(&root.0, manifest).unwrap();
        let intent = Intent {
            cutoff: Offset(80_000),
            goal: CloseGoal::Interrupted(cause),
        };
        bundle.write_intent(&intent).unwrap();
        bundle.write_seal(&sample_seal()).unwrap();
        let saved = bundle.read_intent().unwrap();
        drop(bundle);

        let Some(DiskState::Recoverable { reason, .. }) = scan(&root.0, id).unwrap() else {
            panic!("interrupted meeting should be recoverable");
        };
        assert_eq!(reason, expected, "{intent:?}");
        assert_eq!(saved, Some(intent));
    }
}

#[test]
fn legacy_interrupted_intent_keeps_its_cutoff_without_classifying_the_issue() {
    let root = TestRoot::new();
    let manifest = manifest();
    let id = manifest.info.id;
    let bundle = Bundle::create(&root.0, manifest).unwrap();
    fs::write(
        bundle.directory().join(INTENT),
        br#"{"goal":"interrupted","cutoff":80000,"issue":"shutdown"}"#,
    )
    .unwrap();
    bundle.write_seal(&sample_seal()).unwrap();
    assert_eq!(
        bundle.read_intent().unwrap().unwrap().cutoff,
        Offset(80_000)
    );
    drop(bundle);
    assert!(matches!(
        scan(&root.0, id).unwrap(),
        Some(DiskState::Recoverable {
            reason: RecoveryReason::Interrupted,
            ..
        })
    ));
}

#[test]
fn scan_checks_transcript_hash_and_prefers_the_last_job_error() {
    let root = TestRoot::new();
    let manifest = manifest();
    let id = manifest.info.id;
    let bundle = Bundle::create(&root.0, manifest).unwrap();
    bundle.write_seal(&sample_seal()).unwrap();
    bundle
        .write_intent(&Intent {
            cutoff: Offset(80_000),
            goal: CloseGoal::Interrupted(InterruptCause::CaptureFault("mic disconnected".into())),
        })
        .unwrap();
    let receipt = publish(&bundle, "# Hej\n", false).unwrap();
    bundle
        .write_job(&Job {
            attempts: 1,
            last_error: JobError::Decode("decoder failed".into()),
        })
        .unwrap();
    let transcript = bundle.directory().join(TRANSCRIPT);
    drop(bundle);

    assert!(
        matches!(scan(&root.0, id).unwrap(), Some(DiskState::Complete { receipt: recorded, .. }) if receipt == recorded)
    );
    fs::write(transcript, "# Edited\n").unwrap();
    assert!(
        matches!(scan(&root.0, id).unwrap(), Some(DiskState::Recoverable { reason: RecoveryReason::DecodeFailed(issue), .. }) if issue == "decoder failed")
    );
    fs::remove_file(root.0.join(id.to_string()).join(JOB)).unwrap();
    assert!(
        matches!(scan(&root.0, id).unwrap(), Some(DiskState::Recoverable { reason: RecoveryReason::CaptureFailed(issue), .. }) if issue == "mic disconnected")
    );
}

#[test]
fn interrupted_republish_does_not_report_an_older_job_error() {
    let root = TestRoot::new();
    let manifest = manifest();
    let id = manifest.info.id;
    let mut bundle = Bundle::create(&root.0, manifest).unwrap();
    bundle.write_seal(&sample_seal()).unwrap();
    publish(&bundle, "# First\n", true).unwrap();
    bundle.write_projections(&PROJECTED).unwrap();
    bundle
        .write_job(&Job {
            attempts: 1,
            last_error: JobError::Decode("helper crashed".into()),
        })
        .unwrap();
    bundle.writer.fault = Some(Box::new(|name, point| {
        if (name, point) == (TRANSCRIPT, BeforeRename) {
            Err(io::Error::other("transcript write failed"))
        } else {
            Ok(())
        }
    }));
    assert!(matches!(
        publish(&bundle, "# Second\n", false),
        Err(WriteError::BeforeRename(_))
    ));
    drop(bundle);

    assert!(matches!(
        scan(&root.0, id).unwrap(),
        Some(DiskState::Recoverable {
            reason: RecoveryReason::Interrupted,
            ..
        })
    ));
}

#[test]
fn scan_all_preserves_corrupt_bundles_and_orders_newest_first() {
    let root = TestRoot::new();
    let first = manifest();
    let mut newest = manifest();
    newest.info.started_at_ms = first.info.started_at_ms + 1;
    let corrupt = manifest();
    drop(Bundle::create(&root.0, first.clone()).unwrap());
    drop(Bundle::create(&root.0, newest.clone()).unwrap());
    let bundle = Bundle::create(&root.0, corrupt.clone()).unwrap();
    fs::write(bundle.directory().join(MANIFEST), b"{").unwrap();
    drop(bundle);
    fs::create_dir(root.0.join("unrelated")).unwrap();

    let states = scan_all(&root.0).unwrap();
    assert_eq!(
        states.iter().map(DiskState::id).collect::<Vec<_>>(),
        [newest.info.id, first.info.id, corrupt.info.id]
    );
    assert!(matches!(states[2], DiskState::Unreadable { .. }));
}

#[test]
fn retention_keeps_audio_until_preferred_transcript_is_committed() {
    let root = TestRoot::new();
    let bundle = Bundle::create(&root.0, manifest()).unwrap();
    let audio = bundle.directory().join("audio/you.journal");
    fs::write(&audio, b"record").unwrap();
    assert!(bundle.apply_retention().is_err());
    assert!(audio.is_file());
    publish(&bundle, "# Fallback\n", true).unwrap();
    bundle.apply_retention().unwrap();
    assert!(audio.is_file());
    bundle.write_projections(&PROJECTED).unwrap();
    publish(&bundle, "# Preferred\n", false).unwrap();
    bundle.apply_retention().unwrap();
    bundle.apply_retention().unwrap();
    assert!(!audio.exists());

    let id = bundle.manifest().info.id;
    drop(bundle);
    assert!(matches!(
        scan(&root.0, id).unwrap(),
        Some(DiskState::Complete {
            audio_retained: false,
            projections: Projections {
                history: HistoryProjection::Pending,
                ..
            },
            ..
        })
    ));
}

#[test]
fn keep_audio_preserves_a_preferred_capture() {
    let root = TestRoot::new();
    let mut manifest = manifest();
    manifest.keep_audio = true;
    let bundle = Bundle::create(&root.0, manifest).unwrap();
    let audio = bundle.directory().join("audio/you.journal");
    fs::write(&audio, b"record").unwrap();
    publish(&bundle, "# Hej\n", false).unwrap();
    bundle.apply_retention().unwrap();
    assert!(audio.is_file());
}

#[test]
fn mark_commit_undo_and_late_submit_are_visible_on_disk() {
    let root = TestRoot::new();
    let mut bundle = Bundle::create(&root.0, manifest()).unwrap();
    let writer = bundle.start_marks().unwrap();
    let sink = writer.sink();
    let first = bundle.directory().join("assets/mark-0000656000-0001.png");
    assert!(sink.submit(1, Offset(656_000), b"first".to_vec()));
    let deadline = Instant::now() + Duration::from_secs(3);
    while !first.is_file() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(fs::read(&first).unwrap(), b"first");
    assert!(sink.undo(1));
    assert!(!sink.submit(1, Offset(656_000), b"late".to_vec()));
    assert!(sink.begin(2, Offset(700_000)));
    assert!(sink.undo(2));
    assert!(!sink.submit(2, Offset(700_000), b"late".to_vec()));
    assert!(sink.submit(3, Offset(800_000), b"kept".to_vec()));
    assert!(sink.submit(10_000, Offset(10_000_000_000), b"wide".to_vec()));
    assert!(writer.close(Offset(10_000_000_000)).is_empty());
    assert!(!first.exists());
    fs::write(bundle.directory().join("assets/.tmp-4"), b"uncommitted").unwrap();
    assert_eq!(
        list_marks(bundle.directory(), None).unwrap(),
        [
            MarkRef {
                id: 3,
                offset: Offset(800_000),
                relative_path: "assets/mark-0000800000-0003.png".into()
            },
            MarkRef {
                id: 10_000,
                offset: Offset(10_000_000_000),
                relative_path: "assets/mark-10000000000-10000.png".into()
            },
        ]
    );
}

#[test]
fn mark_close_waits_for_registered_capture_and_rejects_after_cutoff() {
    let root = TestRoot::new();
    let mut bundle = Bundle::create(&root.0, manifest()).unwrap();
    let writer = bundle.start_marks().unwrap();
    let sink = writer.sink();
    assert!(sink.begin(1, Offset(30)));
    assert!(sink.begin(2, Offset(40)));

    thread::scope(|scope| {
        let close = scope.spawn(move || writer.close_for(Offset(30), Duration::from_secs(2)));
        let deadline = Instant::now() + Duration::from_secs(1);
        let mut id = 100;
        while sink.begin(id, Offset(50)) {
            assert!(Instant::now() < deadline, "close did not start");
            id += 1;
            thread::sleep(Duration::from_millis(2));
        }
        assert!(!sink.submit(2, Offset(40), b"after cutoff".to_vec()));
        assert!(!sink.submit(5, Offset(31), b"new after cutoff".to_vec()));
        assert!(sink.begin(3, Offset(29)));
        assert!(sink.submit(3, Offset(29), b"late begin".to_vec()));
        assert!(sink.submit(4, Offset(30), b"late submit".to_vec()));
        assert!(sink.submit(1, Offset(30), b"at cutoff".to_vec()));
        assert!(close.join().unwrap().is_empty());
    });
    assert_eq!(
        list_marks(bundle.directory(), None)
            .unwrap()
            .iter()
            .map(|mark| mark.id)
            .collect::<Vec<_>>(),
        [3, 1, 4]
    );
}

#[test]
fn mark_drain_reports_misses_and_excludes_late_files_after_seal() {
    let root = TestRoot::new();
    let mut bundle = Bundle::create(&root.0, manifest()).unwrap();
    let writer = bundle.start_marks().unwrap();
    let sink = writer.sink();
    assert!(sink.begin(1, Offset(30)));
    assert!(sink.begin(2, Offset(40)));
    assert!(sink.miss(2, "crop failed".into()));
    assert!(sink.begin(4, Offset(41)));
    assert!(!sink.submit(4, Offset(42), b"mismatched offset".to_vec()));
    let mark_issues = writer.close_for(Offset(50), Duration::from_millis(20));
    assert_eq!(
        mark_issues,
        [
            MarkIssue {
                id: 1,
                issue: "mark drain timed out".into()
            },
            MarkIssue {
                id: 2,
                issue: "crop failed".into()
            },
            MarkIssue {
                id: 4,
                issue: "image offset 42 differs from first ink offset 41".into(),
            },
        ]
    );
    assert!(!sink.submit(1, Offset(30), b"too late".to_vec()));
    let seal = Seal {
        cutoff: Offset(50),
        mark_issues,
        ..sample_seal()
    };
    bundle.write_seal(&seal).unwrap();
    fs::write(
        bundle.directory().join("assets/mark-0000000030-0001.png"),
        b"late rename",
    )
    .unwrap();
    fs::write(
        bundle.directory().join("assets/mark-0000000051-0003.png"),
        b"after cutoff",
    )
    .unwrap();
    assert!(
        list_marks(bundle.directory(), bundle.read_seal().unwrap().as_ref())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn scan_reports_a_held_lock_during_manifest_creation() {
    let root = TestRoot::new();
    let id = MeetingId::new();
    let directory = root.0.join(id.to_string());
    fs::create_dir_all(&directory).unwrap();
    let lock = File::create(directory.join(LOCK)).unwrap();
    lock.try_lock().unwrap();

    assert_eq!(
        scan(&root.0, id).unwrap(),
        Some(DiskState::Active { id, manifest: None })
    );
    drop(lock);
    assert!(matches!(
        scan(&root.0, id).unwrap(),
        Some(DiskState::Unreadable { .. })
    ));
}

#[test]
fn mark_close_unlinks_an_already_committed_mark_after_cutoff() {
    let root = TestRoot::new();
    let mut bundle = Bundle::create(&root.0, manifest()).unwrap();
    let writer = bundle.start_marks().unwrap();
    let sink = writer.sink();
    let after = bundle.directory().join("assets/mark-0000000031-0001.png");
    assert!(sink.submit(1, Offset(31), b"after cutoff".to_vec()));
    let deadline = Instant::now() + Duration::from_secs(3);
    while !after.is_file() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(2));
    }
    assert!(after.is_file());
    assert!(writer.close(Offset(30)).is_empty());
    assert!(!after.exists());
}

#[test]
fn lock_retry_waits_out_a_brief_scan_and_nothing_else() {
    let mut calls = 0;
    let result = retry_while_scanned(Duration::from_secs(5), || {
        calls += 1;
        if calls < 3 {
            Err(std::fs::TryLockError::WouldBlock)
        } else {
            Ok(())
        }
    });
    assert!(result.is_ok());
    assert_eq!(calls, 3);

    let started = Instant::now();
    let result = retry_while_scanned(Duration::from_millis(20), || {
        Err(std::fs::TryLockError::WouldBlock)
    });
    assert!(matches!(result, Err(std::fs::TryLockError::WouldBlock)));
    assert!(started.elapsed() >= Duration::from_millis(20));

    let mut calls = 0;
    let result = retry_while_scanned(Duration::from_secs(5), || {
        calls += 1;
        Err(std::fs::TryLockError::Error(io::Error::other("denied")))
    });
    assert!(matches!(result, Err(std::fs::TryLockError::Error(_))));
    assert_eq!(calls, 1);
}

mod publish_gate;
