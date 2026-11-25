use std::os::unix::process::ExitStatusExt;
use std::process::Command;

use crate::history::HistoryStore;
use crate::meeting::transcript::history_text;

use super::*;

const ID: &str = "550e8400-e29b-41d4-a716-446655440000";
const MARKDOWN: &str = "---\ntype: meeting-transcript\ndate: 2025-11-24T16:05:12Z\n---\n\n# Meeting\n\n[00:00] **You:** Recovery needle. <!-- und parakeet-v3 -->\n";
const CHILD_ROOT: &str = "KOETT_S5_PUBLISH_GATE_ROOT";
const CHILD_BOUNDARY: &str = "KOETT_S5_PUBLISH_GATE_BOUNDARY";

fn fixture_manifest(root: &Path) -> Manifest {
    let mut manifest = manifest();
    manifest.info.id = MeetingId::parse(ID).unwrap();
    manifest.export_dir = Some(root.join("export"));
    manifest
}

fn publish_fixture(bundle: &Bundle, database: &Path, stage: Option<Receipt>) {
    let receipt = stage.unwrap_or_else(|| {
        let mut history = history(bundle.manifest());
        history.text_sha256 = sha256(history_text(MARKDOWN).unwrap().as_bytes());
        bundle
            .publish(
                MARKDOWN,
                Publication {
                    history,
                    assets: Vec::new(),
                    backends: [("parakeet-v3".into(), 1)].into_iter().collect(),
                    fallback: false,
                },
            )
            .unwrap()
    });
    let seal = bundle.read_seal().unwrap().unwrap();
    bundle.project(
        &receipt,
        MARKDOWN,
        Some(seal.cutoff),
        Projections::pending(bundle.manifest()),
        Ok(database),
    );
}

fn killed_child(root: &Path, boundary: String) {
    let bundle = Bundle::create_with_writer(
        &root.join("meetings"),
        fixture_manifest(root),
        AtomicWriter {
            fault: Some(Box::new(move |name, stage| {
                if name == boundary && stage == AfterRename {
                    // SIGKILL exercises process death without running destructors.
                    unsafe { libc::kill(libc::getpid(), libc::SIGKILL) };
                }
                Ok(())
            })),
        },
    )
    .unwrap();
    fs::write(
        bundle.directory().join("audio/you.journal"),
        b"fixture audio",
    )
    .unwrap();
    bundle.write_seal(&sample_seal()).unwrap();
    publish_fixture(&bundle, &root.join("history.sqlite3"), None);
    panic!("publish boundary was not reached");
}

#[test]
fn kill_at_every_publish_boundary_converges_to_one_history_row() {
    if let Some(root) = std::env::var_os(CHILD_ROOT) {
        killed_child(Path::new(&root), std::env::var(CHILD_BOUNDARY).unwrap());
        return;
    }
    for boundary in [
        RECEIPT,
        TRANSCRIPT,
        "retention",
        "history",
        "export",
        PROJECTIONS,
    ] {
        let root = TestRoot::new();
        let status = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "meeting::bundle::tests::publish_gate::kill_at_every_publish_boundary_converges_to_one_history_row"])
            .env(CHILD_ROOT, &root.0)
            .env(CHILD_BOUNDARY, boundary)
            .output().unwrap();
        assert_eq!(
            status.status.signal(),
            Some(libc::SIGKILL),
            "{boundary}: {status:?}"
        );

        let bundle = Bundle::open(&root.0.join("meetings"), MeetingId::parse(ID).unwrap()).unwrap();
        let database = root.0.join("history.sqlite3");
        match bundle.read_state() {
            DiskState::Recoverable { .. } if boundary == RECEIPT => {
                assert!(bundle.directory().join("audio/you.journal").is_file());
                publish_fixture(&bundle, &database, None);
            }
            DiskState::Complete { receipt, .. } => {
                publish_fixture(&bundle, &database, Some(receipt))
            }
            state => panic!("unexpected state after {boundary}: {state:?}"),
        }
        let DiskState::Complete {
            receipt,
            projections,
            audio_retained,
            ..
        } = bundle.read_state()
        else {
            panic!("{boundary} did not complete");
        };
        assert_eq!(receipt.transcript_sha256, sha256(MARKDOWN.as_bytes()));
        assert_eq!(
            fs::read_to_string(bundle.directory().join(TRANSCRIPT)).unwrap(),
            MARKDOWN
        );
        assert!(!audio_retained);
        assert_eq!(
            projections,
            Projections {
                history: HistoryProjection::Indexed,
                export: ExportProjection::Exported
            }
        );
        let store = HistoryStore::open(&database).unwrap();
        assert_eq!(store.page("", None, 50, None).unwrap().len(), 1);
        let record = store.get(&receipt.publication.history.id).unwrap().unwrap();
        assert_eq!(record.text, history_text(MARKDOWN).unwrap());
        assert_eq!(fs::read_dir(root.0.join("export")).unwrap().count(), 1);
    }
}

#[test]
fn a_different_note_is_kept_and_the_export_takes_the_next_free_name() {
    let root = TestRoot::new();
    let bundle = Bundle::create(&root.0.join("meetings"), fixture_manifest(&root.0)).unwrap();
    bundle.write_seal(&sample_seal()).unwrap();
    let export = root.0.join("export");
    fs::create_dir(&export).unwrap();
    let note = export.join(NOTE);
    let existing = format!("---\nkoett_id: {ID}\n---\n\nEarlier decode, edited by hand.\n");
    fs::write(&note, &existing).unwrap();

    publish_fixture(&bundle, &root.0.join("history.sqlite3"), None);

    let DiskState::Complete { projections, .. } = bundle.read_state() else {
        panic!("missing publication");
    };
    assert_eq!(projections.history, HistoryProjection::Indexed);
    assert_eq!(projections.export, ExportProjection::Exported);
    assert_eq!(fs::read_to_string(&note).unwrap(), existing);
    let copy = export.join(NOTE.replace(".md", " (2).md"));
    assert!(
        fs::read_to_string(copy)
            .unwrap()
            .contains("Recovery needle.")
    );
    assert_eq!(fs::read_dir(&export).unwrap().count(), 2);
}

#[test]
fn projection_file_failure_leaves_a_complete_transcript_with_pending_repairs() {
    let root = TestRoot::new();
    let bundle = Bundle::create(&root.0.join("meetings"), fixture_manifest(&root.0)).unwrap();
    bundle.write_seal(&sample_seal()).unwrap();
    let database = root.0.join("history.sqlite3");
    let projection_path = bundle.directory().join(PROJECTIONS);
    fs::create_dir(&projection_path).unwrap();

    publish_fixture(&bundle, &database, None);

    let DiskState::Complete {
        receipt,
        projections,
        ..
    } = bundle.read_state()
    else {
        panic!("projection bookkeeping blocked publication");
    };
    assert!(projections.has_pending());
    assert_eq!(receipt.transcript_sha256, sha256(MARKDOWN.as_bytes()));
    fs::remove_dir(projection_path).unwrap();
    publish_fixture(&bundle, &database, Some(receipt));

    let DiskState::Complete { projections, .. } = bundle.read_state() else {
        panic!("missing publication");
    };
    assert!(!projections.has_pending());
    assert_eq!(
        HistoryStore::open(database)
            .unwrap()
            .page("", None, 50, None)
            .unwrap()
            .len(),
        1
    );
}

const NOTE: &str = "2025-11-24 1605 Koett meeting 550e8400.md";

#[test]
fn vault_export_cleans_up_a_stale_temporary_file() {
    let root = TestRoot::new();
    let bundle = Bundle::create(&root.0.join("meetings"), fixture_manifest(&root.0)).unwrap();
    bundle.write_seal(&sample_seal()).unwrap();
    let export = root.0.join("export");
    fs::create_dir(&export).unwrap();
    let stale = export.join(format!(".koett-export-{}.tmp", MeetingId::new()));
    let unrelated = export.join(format!(".{NOTE}.tmp-{}", MeetingId::new()));
    fs::write(&stale, "interrupted export").unwrap();
    fs::write(&unrelated, "another file").unwrap();

    publish_fixture(&bundle, &root.0.join("history.sqlite3"), None);

    assert!(export.join(NOTE).is_file());
    assert!(!stale.exists());
    assert!(unrelated.is_file());
}

#[test]
fn vault_export_writes_a_note_with_the_meeting_id_and_vault_permissions() {
    let root = TestRoot::new();
    let bundle = Bundle::create(&root.0.join("meetings"), fixture_manifest(&root.0)).unwrap();
    bundle.write_seal(&sample_seal()).unwrap();

    publish_fixture(&bundle, &root.0.join("history.sqlite3"), None);

    let path = root.0.join("export").join(NOTE);
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o644
    );
    assert!(
        fs::read_to_string(path)
            .unwrap()
            .contains(&format!("\nkoett_id: {ID}\n"))
    );
}

#[test]
fn vault_export_stays_pending_when_every_name_holds_a_different_note() {
    let root = TestRoot::new();
    let bundle = Bundle::create(&root.0.join("meetings"), fixture_manifest(&root.0)).unwrap();
    bundle.write_seal(&sample_seal()).unwrap();
    let export = root.0.join("export");
    fs::create_dir(&export).unwrap();
    let stem = NOTE.strip_suffix(".md").unwrap();
    let names: Vec<String> = std::iter::once(NOTE.to_owned())
        .chain((2..=9).map(|copy| format!("{stem} ({copy}).md")))
        .collect();
    for name in &names {
        fs::write(export.join(name), format!("Kept {name}\n")).unwrap();
    }

    publish_fixture(&bundle, &root.0.join("history.sqlite3"), None);

    let DiskState::Complete { projections, .. } = bundle.read_state() else {
        panic!("missing publication");
    };
    assert_eq!(projections.export, ExportProjection::Pending);
    for name in &names {
        assert_eq!(
            fs::read_to_string(export.join(name)).unwrap(),
            format!("Kept {name}\n")
        );
    }
    assert_eq!(fs::read_dir(&export).unwrap().count(), names.len());
}

#[test]
fn vault_export_finds_its_earlier_copy_before_creating_another() {
    let root = TestRoot::new();
    let bundle = Bundle::create(&root.0.join("meetings"), fixture_manifest(&root.0)).unwrap();
    bundle.write_seal(&sample_seal()).unwrap();
    let export = root.0.join("export");
    fs::create_dir(&export).unwrap();
    fs::write(export.join(NOTE), "A note by hand.\n").unwrap();
    publish_fixture(&bundle, &root.0.join("history.sqlite3"), None);
    let copy = export.join(NOTE.replace(".md", " (2).md"));
    let exported = fs::read(&copy).unwrap();

    fs::remove_file(export.join(NOTE)).unwrap();
    let note = String::from_utf8(exported.clone()).unwrap();
    crate::meeting::bundle::files::AtomicWriter::default()
        .write_export(&export, NOTE, &note)
        .unwrap();

    assert!(!export.join(NOTE).exists());
    assert_eq!(fs::read(&copy).unwrap(), exported);
}

#[test]
fn vault_export_treats_unreadable_and_linked_notes_as_occupied() {
    let root = TestRoot::new();
    let bundle = Bundle::create(&root.0.join("meetings"), fixture_manifest(&root.0)).unwrap();
    bundle.write_seal(&sample_seal()).unwrap();
    let export = root.0.join("export");
    fs::create_dir(&export).unwrap();
    fs::write(export.join(NOTE), [0xff_u8, 0xfe, 0x00]).unwrap();
    let elsewhere = root.0.join("elsewhere.md");
    fs::write(&elsewhere, "not here").unwrap();
    std::os::unix::fs::symlink(&elsewhere, export.join(NOTE.replace(".md", " (2).md"))).unwrap();

    publish_fixture(&bundle, &root.0.join("history.sqlite3"), None);

    let DiskState::Complete { projections, .. } = bundle.read_state() else {
        panic!("missing publication");
    };
    assert_eq!(projections.export, ExportProjection::Exported);
    assert_eq!(fs::read(export.join(NOTE)).unwrap(), [0xff_u8, 0xfe, 0x00]);
    assert_eq!(fs::read_to_string(&elsewhere).unwrap(), "not here");
    assert!(
        fs::read_to_string(export.join(NOTE.replace(".md", " (3).md")))
            .unwrap()
            .contains("Recovery needle.")
    );
}
