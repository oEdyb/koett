use super::*;
use std::{fs, path::PathBuf};
use time::macros::datetime;
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "koett-history-test-{}-{}",
            std::process::id(),
            NEXT_FAILED_TRANSCRIPT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn db(&self) -> PathBuf {
        self.0.join("history.sqlite3")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn record(id: &str) -> HistoryRecord {
    HistoryRecord {
        id: id.into(),
        created_at_ms: 100,
        model: "Parakeet".into(),
        text: "Héllo, 世界! literal OR NEAR punctuation.".into(),
        raw_text: None,
    }
}
#[test]
fn paging_search_retries_restart_and_conflicts() {
    let temp = Temp::new();
    let store = HistoryStore::open(temp.db()).unwrap();
    for i in 0..110 {
        store.save(&record(&format!("{i:03}"))).unwrap();
    }
    store.save(&record("000")).unwrap();
    let mut conflict = record("000");
    conflict.text = "changed".into();
    assert!(store.save(&conflict).is_err());
    let first = store.page("", None, 1000).unwrap();
    assert_eq!(first.len(), 50);
    assert_eq!(first[0].id, "109");
    let last = first.last().unwrap();
    let cursor = HistoryCursor {
        id: last.id.clone(),
        created_at_ms: last.created_at_ms,
    };
    assert_eq!(store.page("", Some(&cursor), 50).unwrap()[0].id, "059");
    for query in [
        "Héllo, 世界!",
        "OR",
        "NEAR",
        "\"literal\"",
        "literal*",
        "hello OR",
    ] {
        assert!(!store.page(query, None, 50).unwrap().is_empty(), "{query}");
    }
    assert!(store.page("*** : ()", None, 50).unwrap().is_empty());
    drop(store);
    assert_eq!(
        HistoryStore::open(temp.db()).unwrap().get("000").unwrap(),
        Some(record("000"))
    );
}
#[test]
fn interrupted_transaction_and_empty_save() {
    let temp = Temp::new();
    let store = HistoryStore::open(temp.db()).unwrap();
    {
        let tx = store.connection.unchecked_transaction().unwrap();
        tx.execute(
            "INSERT INTO transcripts VALUES ('interrupted',1,'model','text',NULL)",
            [],
        )
        .unwrap();
    }
    assert!(store.get("interrupted").unwrap().is_none());
    let mut empty = record("empty");
    empty.text = " \n".into();
    store.save(&empty).unwrap();
    assert!(store.get("empty").unwrap().is_none());
    let path = temp.0.join("empty.sqlite3");
    append_transcript_to(&path, datetime!(2026-08-24 12:30 UTC), "model", " ").unwrap();
    assert!(!path.exists());
}
#[test]
fn fallback_and_private_files() {
    let temp = Temp::new();
    let primary = temp.0.join("blocked");
    fs::create_dir(&primary).unwrap();
    let fallback = append_transcript_with_fallback_to(
        &primary,
        &temp.0.join("fallback"),
        datetime!(2026-08-24 12:30 UTC),
        "model",
        "Saved safely.",
    )
    .unwrap()
    .unwrap();
    assert!(
        fs::read_to_string(fallback)
            .unwrap()
            .contains("Saved safely.")
    );
    let store = HistoryStore::open(temp.db()).unwrap();
    store.save(&record("private")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(temp.db()).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
#[test]
fn legacy_unicode_markers_raw_cleaned_retry_and_original_bytes() {
    let temp = Temp::new();
    let store = HistoryStore::open(temp.db()).unwrap();
    let source = temp.0.join("Transcripts.md");
    let bytes=b"# Koett Transcripts\n\nSaved locally.\n\n<!-- koett-transcript:YWJj -->\n## 2026-08-24T12:30:00Z\n\nModel: Parakeet\n\nRaw transcript:\n\nraw text\n\nCleaned transcript:\n\nClean text\n## dictated heading\n\n<!-- koett-transcript:ZGVm -->\n## 2026-08-24T12:30:00Z\n\nModel: Parakeet\n\nClean text\n\n";
    fs::write(&source, bytes).unwrap();
    assert_eq!(store.import_legacy(&source).unwrap(), 3);
    assert_eq!(store.import_legacy(&source).unwrap(), 3);
    assert_eq!(store.page("", None, 50).unwrap().len(), 3);
    let rows: Vec<Vec<u8>> = store
        .connection
        .prepare("SELECT content FROM legacy_fragments ORDER BY byte_offset")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows.concat(), bytes);
    assert_eq!(fs::read(&source).unwrap(), bytes);
    let selected = store.page("raw", None, 50).unwrap();
    assert_eq!(selected.len(), 1);
    assert_eq!(
        store
            .get(&selected[0].id)
            .unwrap()
            .unwrap()
            .raw_text
            .as_deref(),
        Some("raw text")
    );
    fs::write(
        &source,
        "## 2026-08-24T12:30:00Z\n\nModel: Rust\n\nHéllo 世界\n\n",
    )
    .unwrap();
    assert_eq!(store.import_legacy(&source).unwrap(), 1);
}
#[test]
fn malformed_huge_and_ambiguous_are_lossless() {
    let temp = Temp::new();
    let store = HistoryStore::open(temp.db()).unwrap();
    let source = temp.0.join("legacy");
    let mut bytes = vec![b'x'; 1024 * 1024 + 3];
    bytes.extend_from_slice(&[0xff, 0]);
    fs::write(&source, &bytes).unwrap();
    assert!(store.import_legacy(&source).unwrap() > 0);
    let rows: Vec<Vec<u8>> = store
        .connection
        .prepare("SELECT content FROM legacy_fragments ORDER BY byte_offset")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows.concat(), bytes);
    fs::write(&source,"## 2026-08-24T12:30:00Z\n\nModel: Rust\n\ntext\n## 2026-08-24T12:31:00Z\n\nModel: could be dictated\n\ntext").unwrap();
    assert_eq!(store.import_legacy(&source).unwrap(), 1);
}
#[test]
#[ignore = "explicit 100k-entry performance measurement"]
fn benchmark_100k() {
    let temp = Temp::new();
    let store = HistoryStore::open(temp.db()).unwrap();
    let tx = store.connection.unchecked_transaction().unwrap();
    {
        let mut insert = tx
            .prepare("INSERT INTO transcripts VALUES (?1,?2,'Parakeet',?3,NULL)")
            .unwrap();
        for i in 0..100_000 {
            insert
                .execute(rusqlite::params![
                    format!("bench-{i:06}"),
                    i,
                    format!("Transcript {i} searchable words and Unicode 世界.")
                ])
                .unwrap();
        }
    }
    tx.commit().unwrap();
    drop(store);
    let start = std::time::Instant::now();
    let store = HistoryStore::open(temp.db()).unwrap();
    let open = start.elapsed();
    let start = std::time::Instant::now();
    assert_eq!(store.page("", None, 50).unwrap().len(), 50);
    let recent = start.elapsed();
    let start = std::time::Instant::now();
    assert_eq!(store.page("searchable 世界", None, 50).unwrap().len(), 50);
    let search = start.elapsed();
    eprintln!("100k open={open:?} recent={recent:?} search={search:?}");
}

#[test]
fn raw_fallback_can_be_reimported_with_exact_identity() {
    let temp = Temp::new();
    let primary = temp.0.join("blocked");
    fs::create_dir(&primary).unwrap();
    let mut expected = record("recovery-id");
    expected.text = "  Cleaned 世界.  ".into();
    expected.raw_text = Some("  Raw héllo.  ".into());
    let fallback = append_record_with_fallback_to(&primary, &temp.0.join("fallback"), &expected)
        .unwrap()
        .unwrap();
    let store = HistoryStore::open(temp.db()).unwrap();
    assert_eq!(store.import_legacy(&fallback).unwrap(), 1);
    assert_eq!(store.get(&expected.id).unwrap(), Some(expected));
}

#[test]
fn interrupted_import_retries_without_duplicate_records() {
    let temp = Temp::new();
    let source = temp.0.join("legacy.md");
    let bytes = "<!-- koett-transcript:b25l -->\n## 2026-08-24T12:30:00Z\n\nModel: Parakeet\n\nRepeated.\n\n<!-- koett-transcript:dHdv -->\n## 2026-08-24T12:30:00Z\n\nModel: Parakeet\n\nRepeated.\n\n";
    fs::write(&source, bytes).unwrap();
    let store = HistoryStore::open(temp.db()).unwrap();
    store.connection.execute_batch("CREATE TRIGGER simulate_interruption BEFORE INSERT ON transcripts WHEN NEW.id='two' BEGIN SELECT RAISE(ABORT,'interrupted'); END;").unwrap();
    assert!(store.import_legacy(&source).is_err());
    assert_eq!(store.page("", None, 50).unwrap().len(), 0);
    store
        .connection
        .execute_batch("DROP TRIGGER simulate_interruption")
        .unwrap();
    drop(store);
    let store = HistoryStore::open(temp.db()).unwrap();
    assert_eq!(store.import_legacy(&source).unwrap(), 2);
    assert_eq!(store.page("", None, 50).unwrap().len(), 2);
    fs::write(&source,format!("{bytes}<!-- koett-transcript:dGhyZWU= -->\n## 2026-08-24T12:30:00Z\n\nModel: Parakeet\n\nRepeated.\n\n")).unwrap();
    assert_eq!(store.import_legacy(&source).unwrap(), 3);
    assert_eq!(store.page("", None, 50).unwrap().len(), 3);
}

#[test]
fn marked_dictation_preserves_timestamp_headings_and_previews_are_bounded() {
    let temp = Temp::new();
    let source = temp.0.join("legacy.md");
    let body = format!(
        "Héllo\n## 2026-08-24T12:31:00Z\n\nModel: dictated text\n\n{}",
        "界".repeat(1000)
    );
    fs::write(&source,format!("<!-- koett-transcript:aGVhZGluZw== -->\n## 2026-08-24T12:30:00Z\n\nModel: Parakeet\n\n{body}\n\n")).unwrap();
    let store = HistoryStore::open(temp.db()).unwrap();
    assert_eq!(store.import_legacy(&source).unwrap(), 1);
    assert_eq!(store.get("heading").unwrap().unwrap().text, body);
    assert_eq!(
        store.page("", None, 50).unwrap()[0].preview.chars().count(),
        240
    );
}

#[test]
fn huge_unmarked_archive_is_searchable_with_unknown_original_metadata() {
    let temp = Temp::new();
    let source = temp.0.join("unmarked.md");
    let mut text =
        String::from("# Koett Transcripts\n\nSaved locally by Koett. Audio is not stored.\n");
    for index in 0..20_000 {
        text.push_str(&format!("\n## 2026-08-24T12:30:00Z\n\nModel: Parakeet\n\nDictation {index} with Unicode 世界.\n\n"));
    }
    text.push_str("Find this archiveendingneedle after the last original entry.\n");
    fs::write(&source, &text).unwrap();
    let store = HistoryStore::open(temp.db()).unwrap();
    let count = store.import_legacy(&source).unwrap();
    assert!(count > 1);
    let results = store.page("archiveendingneedle", None, 50).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].created_at_ms, 0);
    assert_eq!(results[0].model, "Imported archive");
    assert_eq!(store.import_legacy(&source).unwrap(), count);
    assert_eq!(store.page("", None, 50).unwrap().len(), count);
    assert_eq!(fs::read(&source).unwrap(), text.as_bytes());
    let original: Vec<Vec<u8>> = store
        .connection
        .prepare("SELECT content FROM legacy_fragments ORDER BY byte_offset")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(original.concat(), text.as_bytes());
}

#[test]
fn archive_chunks_complete_unicode_scalars_and_reconstruct_exact_text() {
    const MAX_CHUNK: usize = 256 * 1024;
    for shift in 0..4 {
        let temp = Temp::new();
        let source = temp.0.join("unicode.md");
        let text = format!(
            "{}{}\nUnicodeboundaryneedle 世界 café.\n",
            "a".repeat(MAX_CHUNK - 7 + shift),
            "🦀界é".repeat(90_000)
        );
        fs::write(&source, &text).unwrap();
        let store = HistoryStore::open(temp.db()).unwrap();
        let count = store.import_legacy(&source).unwrap();
        let mut chunks: Vec<_> = store
            .page("", None, 50)
            .unwrap()
            .into_iter()
            .map(|summary| {
                assert_eq!(summary.model, "Imported archive");
                let offset = summary
                    .id
                    .rsplit('-')
                    .next()
                    .unwrap()
                    .parse::<usize>()
                    .unwrap();
                let record = store.get(&summary.id).unwrap().unwrap();
                assert!(record.text.len() <= MAX_CHUNK);
                (offset, record.text)
            })
            .collect();
        assert_eq!(chunks.len(), count);
        chunks.sort_by_key(|(offset, _)| *offset);
        let reconstructed: String = chunks.into_iter().map(|(_, text)| text).collect();
        assert_eq!(reconstructed, text);
        assert_eq!(
            store
                .page("Unicodeboundaryneedle 世界 café", None, 50)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(store.import_legacy(&source).unwrap(), count);
    }
}

#[test]
fn identical_archive_chunks_at_different_offsets_remain_distinct() {
    let temp = Temp::new();
    let source = temp.0.join("repeated.md");
    let chunk = format!(
        "chunkneedle{}\n",
        " ".repeat(256 * 1024 - 3 - "chunkneedle\n".len())
    );
    fs::write(&source, format!("{chunk}{chunk}")).unwrap();
    let store = HistoryStore::open(temp.db()).unwrap();
    assert_eq!(store.import_legacy(&source).unwrap(), 2);
    let rows = store.page("chunkneedle", None, 50).unwrap();
    assert_eq!(rows.len(), 2);
    assert_ne!(rows[0].id, rows[1].id);
    assert_eq!(
        store.get(&rows[0].id).unwrap().unwrap().text,
        store.get(&rows[1].id).unwrap().unwrap().text
    );
    assert_eq!(store.import_legacy(&source).unwrap(), 2);
    assert_eq!(store.page("chunkneedle", None, 50).unwrap().len(), 2);
}

#[test]
fn previously_preserved_only_archives_are_indexed_on_explicit_retry() {
    use sha2::{Digest, Sha256};
    let temp = Temp::new();
    let source = temp.0.join("old-import.md");
    let text = "Old preservation-only archive with newlysearchableneedle.\n";
    fs::write(&source, text).unwrap();
    let store = HistoryStore::open(temp.db()).unwrap();
    let old_hash: String = Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    store
        .connection
        .execute(
            "INSERT INTO history_imports VALUES (?1,?2,0)",
            rusqlite::params![old_hash, i64::try_from(text.len()).unwrap()],
        )
        .unwrap();
    assert_eq!(store.import_legacy(&source).unwrap(), 1);
    assert_eq!(
        store.page("newlysearchableneedle", None, 50).unwrap().len(),
        1
    );
}

#[cfg(target_os = "linux")]
#[test]
fn default_open_does_not_read_legacy_archive() {
    const PROBE: &str = "KOETT_HISTORY_DEFAULT_OPEN_PROBE";
    if std::env::var_os(PROBE).is_none() {
        let temp = Temp::new();
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "history::tests::default_open_does_not_read_legacy_archive",
                "--nocapture",
            ])
            .env(PROBE, "1")
            .env("XDG_DATA_HOME", &temp.0)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        return;
    }
    let legacy = crate::paths::history_file().unwrap();
    fs::create_dir_all(&legacy).unwrap();
    let store = HistoryStore::open_default().unwrap();
    assert!(store.page("", None, 50).unwrap().is_empty());
    assert!(store.get("absent").unwrap().is_none());
    assert!(store.import_default_legacy().is_err());
    fs::remove_dir(&legacy).unwrap();
    assert_eq!(store.import_default_legacy().unwrap(), 0);
    fs::write(
        &legacy,
        "Explicitstartupimportneedle in an ambiguous archive.\n",
    )
    .unwrap();
    assert!(
        HistoryStore::open_default()
            .unwrap()
            .page("Explicitstartupimportneedle", None, 50)
            .unwrap()
            .is_empty()
    );
    assert_eq!(store.import_default_legacy().unwrap(), 1);
    assert_eq!(
        store
            .page("Explicitstartupimportneedle", None, 50)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn interrupted_archive_chunk_import_retries_without_duplicates() {
    let temp = Temp::new();
    let source = temp.0.join("archive-interruption.md");
    let first = format!(
        "firstchunkneedle{}\n",
        " ".repeat(256 * 1024 - 3 - "firstchunkneedle\n".len())
    );
    fs::write(&source, format!("{first}lastchunkneedle\n")).unwrap();
    let store = HistoryStore::open(temp.db()).unwrap();
    store.connection.execute_batch("CREATE TRIGGER interrupt_archive BEFORE INSERT ON transcripts WHEN NEW.text LIKE '%lastchunkneedle%' BEGIN SELECT RAISE(ABORT,'interrupted'); END;").unwrap();
    assert!(store.import_legacy(&source).is_err());
    assert_eq!(store.page("firstchunkneedle", None, 50).unwrap().len(), 0);
    store
        .connection
        .execute_batch("DROP TRIGGER interrupt_archive")
        .unwrap();
    assert_eq!(store.import_legacy(&source).unwrap(), 2);
    assert_eq!(store.import_legacy(&source).unwrap(), 2);
    assert_eq!(store.page("", None, 50).unwrap().len(), 2);
    assert_eq!(store.page("lastchunkneedle", None, 50).unwrap().len(), 1);
}

fn unmarked(text: &str) -> String {
    format!("## 2026-08-24T12:30:00Z\n\nModel: Parakeet\n\n{text}\n\n")
}

#[test]
fn changed_unmarked_source_replaces_searchable_version_and_preserves_bytes() {
    let temp = Temp::new();
    let source = temp.0.join("growing.md");
    let first = unmarked("firstneedle");
    fs::write(&source, &first).unwrap();
    let store = HistoryStore::open(temp.db()).unwrap();
    assert_eq!(store.import_legacy(&source).unwrap(), 1);
    let old_id = store.page("firstneedle", None, 50).unwrap()[0].id.clone();
    let second = format!("{first}{}", unmarked("secondneedle"));
    fs::write(&source, &second).unwrap();
    for _ in 0..2 {
        assert_eq!(store.import_legacy(&source).unwrap(), 1);
        assert_eq!(store.page("firstneedle", None, 50).unwrap().len(), 1);
        assert_eq!(store.page("secondneedle", None, 50).unwrap().len(), 1);
        assert!(store.get(&old_id).unwrap().is_none());
    }
    let preserved: i64 = store
        .connection
        .query_row(
            "SELECT sum(length(content)) FROM legacy_fragments",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        preserved,
        i64::try_from(first.len() + second.len()).unwrap()
    );
    // Returning to an old complete digest must restore rows removed by reconciliation.
    fs::write(&source, &first).unwrap();
    assert_eq!(store.import_legacy(&source).unwrap(), 1);
    assert!(store.get(&old_id).unwrap().is_some());
    assert!(store.page("secondneedle", None, 50).unwrap().is_empty());
}

#[test]
fn appended_ambiguous_tail_reconciles_only_changed_chunk_and_retries() {
    let temp = Temp::new();
    let source = temp.0.join("chunks.md");
    let first = format!(
        "stablechunkneedle{}\n",
        " ".repeat(256 * 1024 - 3 - "stablechunkneedle\n".len())
    );
    fs::write(&source, format!("{first}tailneedle\n")).unwrap();
    let store = HistoryStore::open(temp.db()).unwrap();
    assert_eq!(store.import_legacy(&source).unwrap(), 2);
    let stable = store.page("stablechunkneedle", None, 50).unwrap()[0]
        .id
        .clone();
    let stale_tail = store.page("tailneedle", None, 50).unwrap()[0].id.clone();
    fs::write(&source, format!("{first}tailneedle\nappendedneedle\n")).unwrap();
    for _ in 0..2 {
        assert_eq!(store.import_legacy(&source).unwrap(), 2);
        assert_eq!(store.page("tailneedle", None, 50).unwrap().len(), 1);
        assert_eq!(store.page("appendedneedle", None, 50).unwrap().len(), 1);
        assert!(store.get(&stable).unwrap().is_some());
        assert!(store.get(&stale_tail).unwrap().is_none());
    }
}

#[test]
fn interrupted_new_version_keeps_previous_search_and_ownership() {
    let temp = Temp::new();
    let source = temp.0.join("interrupted-growth.md");
    let first = unmarked("firstneedle");
    fs::write(&source, &first).unwrap();
    let store = HistoryStore::open(temp.db()).unwrap();
    store.import_legacy(&source).unwrap();
    let old = store.page("firstneedle", None, 50).unwrap()[0].clone();
    fs::write(&source, format!("{first}{}", unmarked("secondneedle"))).unwrap();
    store.connection.execute_batch("CREATE TRIGGER interrupt_completion BEFORE INSERT ON history_imports BEGIN SELECT RAISE(ABORT,'interrupted'); END;").unwrap();
    assert!(store.import_legacy(&source).is_err());
    assert_eq!(store.page("firstneedle", None, 50).unwrap(), vec![old]);
    assert!(store.page("secondneedle", None, 50).unwrap().is_empty());
    let versions: i64 = store
        .connection
        .query_row("SELECT count(*) FROM history_imports", [], |r| r.get(0))
        .unwrap();
    assert_eq!(versions, 1);
    store
        .connection
        .execute_batch("DROP TRIGGER interrupt_completion")
        .unwrap();
    drop(store);
    let store = HistoryStore::open(temp.db()).unwrap();
    assert_eq!(store.import_legacy(&source).unwrap(), 1);
    assert_eq!(store.page("firstneedle", None, 50).unwrap().len(), 1);
}

#[test]
fn reconciliation_protects_native_saves_other_sources_and_repeated_dictations() {
    let temp = Temp::new();
    let source = temp.0.join("first.md");
    let other = temp.0.join("second.md");
    let text = unmarked("sharedneedle");
    fs::write(&source, &text).unwrap();
    fs::write(&other, &text).unwrap();
    let store = HistoryStore::open(temp.db()).unwrap();
    store.import_legacy(&source).unwrap();
    store.import_legacy(&other).unwrap();
    let id = store.page("sharedneedle", None, 50).unwrap()[0].id.clone();
    fs::write(&source, "replacementneedle\n").unwrap();
    store.import_legacy(&source).unwrap();
    assert!(store.get(&id).unwrap().is_some());
    // A native save of an existing imported ID must claim durable ownership.
    let native = store.get(&id).unwrap().unwrap();
    store.save(&native).unwrap();
    fs::write(&other, "replacementneedle\n").unwrap();
    store.import_legacy(&other).unwrap();
    assert_eq!(store.get(&id).unwrap(), Some(native));
    let mut repeated = record("native-one");
    repeated.text = "samegenuinedictation".into();
    store.save(&repeated).unwrap();
    repeated.id = "native-two".into();
    store.save(&repeated).unwrap();
    assert_eq!(
        store.page("samegenuinedictation", None, 50).unwrap().len(),
        2
    );
}

#[test]
fn schema_one_upgrade_is_sqlite_only_and_explicit_import_versions_completion() {
    use sha2::{Digest, Sha256};
    let temp = Temp::new();
    let source = temp.0.join("upgrade.md");
    let text = unmarked("upgradeneedle");
    fs::write(&source, &text).unwrap();
    let store = HistoryStore::open(temp.db()).unwrap();
    store.save(&record("existing-native")).unwrap();
    let hash: String = Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    store
        .connection
        .execute(
            "INSERT INTO history_imports VALUES (?1,?2,0)",
            rusqlite::params![
                format!("searchable-archive-v2:{hash}"),
                i64::try_from(text.len()).unwrap()
            ],
        )
        .unwrap();
    store.connection.execute_batch("DROP TABLE history_sources; DROP TABLE history_import_records; DROP TABLE history_native_records; PRAGMA user_version=1;").unwrap();
    drop(store);
    let store = HistoryStore::open(temp.db()).unwrap();
    assert!(store.page("upgradeneedle", None, 50).unwrap().is_empty());
    assert!(store.get("existing-native").unwrap().is_some());
    assert_eq!(store.import_legacy(&source).unwrap(), 1);
    fs::write(&source, format!("{text}{}", unmarked("moreupgradeneedle"))).unwrap();
    assert_eq!(store.import_legacy(&source).unwrap(), 1);
    assert_eq!(store.page("upgradeneedle", None, 50).unwrap().len(), 1);
    assert!(store.get("existing-native").unwrap().is_some());
}

#[test]
fn schema_upgrade_protects_preexisting_native_id_referenced_by_import() {
    let temp = Temp::new();
    let source = temp.0.join("native-reference.md");
    let native = record("native-before-upgrade");
    fs::write(&source, transcript_entry(&native).unwrap()).unwrap();
    let store = HistoryStore::open(temp.db()).unwrap();
    store.save(&native).unwrap();
    store.connection.execute_batch("DROP TABLE history_sources; DROP TABLE history_import_records; DROP TABLE history_native_records; PRAGMA user_version=1;").unwrap();
    drop(store);
    let store = HistoryStore::open(temp.db()).unwrap();
    assert_eq!(store.import_legacy(&source).unwrap(), 1);
    fs::write(&source, "").unwrap();
    assert_eq!(store.import_legacy(&source).unwrap(), 0);
    assert_eq!(store.get(&native.id).unwrap(), Some(native));
}
