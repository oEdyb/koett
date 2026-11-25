use std::os::unix::process::ExitStatusExt;
use std::process::Command;

use crate::history::HistoryStore;
use crate::meeting::transcript::history_text;

use super::*;

const ID: &str = "550e8400-e29b-41d4-a716-446655440000";
const MARKDOWN: &str = "---\ntype: meeting-transcript\n---\n\n# Meeting\n\n[00:00] **You:** Recovery needle. <!-- und parakeet-v3 -->\n";
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
fn conflicting_vault_note_is_preserved_while_history_indexes() {
    let root = TestRoot::new();
    let bundle = Bundle::create(&root.0.join("meetings"), fixture_manifest(&root.0)).unwrap();
    bundle.write_seal(&sample_seal()).unwrap();
    let export = root.0.join("export");
    fs::create_dir(&export).unwrap();
    let note = export.join("2025-11-24 1605 Koett meeting 550e8400.md");
    fs::write(&note, "existing material").unwrap();

    publish_fixture(&bundle, &root.0.join("history.sqlite3"), None);

    let DiskState::Complete { projections, .. } = bundle.read_state() else {
        panic!("missing publication");
    };
    assert_eq!(projections.history, HistoryProjection::Indexed);
    assert_eq!(projections.export, ExportProjection::Pending);
    assert_eq!(fs::read_to_string(note).unwrap(), "existing material");
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
