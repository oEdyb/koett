use super::*;
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

#[derive(Default)]
struct Recovery {
    last: Option<String>,
    last_source: Option<&'static str>,
    pending: BTreeMap<String, HistoryRecord>,
}
static AVAILABLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static PENDING: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static STATE: OnceLock<Mutex<Recovery>> = OnceLock::new();
fn state() -> &'static Mutex<Recovery> {
    STATE.get_or_init(|| Mutex::new(Recovery::default()))
}

pub fn initialize_last() -> Result<(), String> {
    let path = paths::last_transcript()?;
    let text = match fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("Cannot restore Last Transcript: {e}")),
    };
    let mut state = state().lock().map_err(|e| e.to_string())?;
    if state.last.is_none() {
        state.last = text.filter(|text| !text.trim().is_empty());
        state.last_source = state.last.as_ref().map(|_| "restored");
    }
    AVAILABLE.store(state.last.is_some(), Ordering::Release);
    Ok(())
}

pub fn last() -> Option<String> {
    state().lock().ok()?.last.clone()
}
pub fn last_source() -> Option<&'static str> {
    state().lock().ok()?.last_source
}
pub(super) fn question_last(text: &str) -> Result<(), String> {
    if !text.trim().is_empty() {
        let mut state = state().lock().map_err(|e| e.to_string())?;
        state.last = Some(text.into());
        state.last_source = Some("ask");
        AVAILABLE.store(true, Ordering::Release);
    }
    Ok(())
}
pub fn last_available() -> bool {
    AVAILABLE.load(Ordering::Acquire)
}
pub fn pending_count() -> usize {
    PENDING.load(Ordering::Acquire)
}
pub fn unsaved() -> Result<String, String> {
    let state = state().lock().map_err(|e| e.to_string())?;
    let mut records: Vec<_> = state.pending.values().collect();
    records.sort_by_key(|r| (r.created_at_ms, &r.id));
    if records.is_empty() {
        return Err("No unsaved transcripts".into());
    }
    Ok(records
        .into_iter()
        .map(|r| {
            if r.text.is_empty() {
                r.raw_text.as_deref().unwrap_or("")
            } else {
                &r.text
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n"))
}

pub fn retry() -> Result<(), String> {
    let mut state = state().lock().map_err(|e| e.to_string())?;
    let primary = paths::history_database()?;
    let fallback = paths::failed_transcripts_directory()?;
    let result = retry_to(&mut state, &primary, &fallback);
    PENDING.store(state.pending.len(), Ordering::Release);
    result
}

fn retry_to(state: &mut Recovery, primary: &Path, fallback: &Path) -> Result<(), String> {
    let mut errors = Vec::new();
    state.pending.retain(|_, record| {
        match append_record_with_fallback_to(primary, fallback, record) {
            Ok(_) => false,
            Err(error) => {
                errors.push(error);
                true
            }
        }
    });
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} transcript(s) remain in memory · Retry Save or Copy Unsaved: {}",
            errors.len(),
            errors.join("; ")
        ))
    }
}

pub(super) fn persist(record: HistoryRecord, keep_last: bool) -> Result<Option<PathBuf>, String> {
    let _ = retry();
    let mut state = state().lock().map_err(|e| e.to_string())?;
    let locations = (|| {
        Ok::<_, String>((
            paths::history_database()?,
            paths::failed_transcripts_directory()?,
            paths::last_transcript()?,
        ))
    })();
    PENDING.store(
        state.pending.len() + usize::from(!state.pending.contains_key(&record.id)),
        Ordering::Release,
    );
    let result = match locations {
        Ok((primary, fallback, last)) => {
            persist_to(&mut state, record, keep_last, &primary, &fallback, &last)
        }
        Err(error) => {
            if keep_last && !record.text.trim().is_empty() {
                state.last = Some(record.text.clone());
                state.last_source = Some("unsaved");
            }
            state.pending.insert(record.id.clone(), record);
            Err(format!(
                "Text in memory · Save failed · Copy Unsaved Transcripts or Retry Save: {error}"
            ))
        }
    };
    PENDING.store(state.pending.len(), Ordering::Release);
    AVAILABLE.store(state.last.is_some(), Ordering::Release);
    result
}

fn persist_to(
    state: &mut Recovery,
    record: HistoryRecord,
    keep_last: bool,
    primary: &Path,
    fallback: &Path,
    last: &Path,
) -> Result<Option<PathBuf>, String> {
    if let Some(existing) = state.pending.get(&record.id)
        && existing != &record
    {
        return Err("Conflicting pending transcript ID".into());
    }
    if keep_last && !record.text.trim().is_empty() {
        state.last = Some(record.text.clone());
        state.last_source = Some("dictation");
    }
    state.pending.insert(record.id.clone(), record.clone());
    let saved = append_record_with_fallback_to(primary, fallback, &record);
    if saved.is_ok() {
        state.pending.remove(&record.id);
    }
    let last_saved = if keep_last && !record.text.trim().is_empty() {
        save_last_to(last, &record.text)
    } else {
        Ok(())
    };
    match (saved, last_saved) {
        (Ok(path), Ok(())) => Ok(path),
        (Ok(_), Err(error)) => Err(format!("Transcript saved · Last action in memory: {error}")),
        (Err(error), _) => {
            if keep_last && !record.text.trim().is_empty() {
                state.last = Some(record.text);
                state.last_source = Some("unsaved");
            }
            Err(format!(
                "Text in memory · Save failed · Copy Unsaved Transcripts or Retry Save: {error}"
            ))
        }
    }
}

pub fn save_last_to(path: &Path, text: &str) -> Result<(), String> {
    let parent = path.parent().ok_or("Last Transcript has no parent")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temporary = parent.join(format!(
        ".last-{}-{}",
        std::process::id(),
        NEXT_FAILED_TRANSCRIPT.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary).map_err(|e| e.to_string())?;
        file.write_all(text.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        fs::rename(&temporary, path).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        fs::File::open(parent)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_media_and_blank_dictation_preserve_last() {
        let root = std::env::temp_dir().join(format!(
            "koett-last-preserved-{}-{}",
            std::process::id(),
            NEXT_FAILED_TRANSCRIPT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let blocker = root.join("blocker");
        fs::write(&blocker, "file").unwrap();
        let mut state = Recovery {
            last: Some("prior dictation".into()),
            last_source: Some("dictation"),
            ..Recovery::default()
        };
        for (text, keep_last) in [("media result", false), (" \n ", true)] {
            let record = new_record(OffsetDateTime::now_utc(), "model", text, Some("raw")).unwrap();
            assert!(
                persist_to(
                    &mut state,
                    record,
                    keep_last,
                    &blocker.join("db"),
                    &blocker.join("fallback"),
                    &root.join("Last.txt"),
                )
                .is_err()
            );
            assert_eq!(state.last.as_deref(), Some("prior dictation"));
            assert_eq!(state.last_source, Some("dictation"));
        }
        assert_eq!(state.pending.len(), 2);
        assert!(!root.join("Last.txt").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn two_failed_records_retry_exactly_once_with_raw_and_collision() {
        let root = std::env::temp_dir().join(format!(
            "koett-recovery-{}-{}",
            std::process::id(),
            NEXT_FAILED_TRANSCRIPT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let blocker = root.join("blocker");
        fs::write(&blocker, "file").unwrap();
        let primary = blocker.join("history");
        let fallback = blocker.join("fallback");
        let last = root.join("Last.txt");
        let mut state = Recovery::default();
        let first = new_record(
            OffsetDateTime::now_utc(),
            "model A",
            "  Å世界\n",
            Some("raw A"),
        )
        .unwrap();
        let second = new_record(OffsetDateTime::now_utc(), "model B", "", Some("raw B")).unwrap();
        for record in [&first, &second] {
            assert!(
                persist_to(&mut state, record.clone(), true, &primary, &fallback, &last).is_err()
            );
        }
        assert_eq!(state.pending.len(), 2);
        assert_eq!(state.last.as_deref(), Some(first.text.as_str()));
        assert!(retry_to(&mut state, &primary, &fallback).is_err());
        let mut collision = first.clone();
        collision.text = "wrong".into();
        assert!(persist_to(&mut state, collision, true, &primary, &fallback, &last).is_err());
        assert_eq!(state.pending[&first.id], first);
        let database = root.join("history.sqlite3");
        retry_to(&mut state, &database, &fallback).unwrap();
        retry_to(&mut state, &database, &fallback).unwrap();
        assert!(state.pending.is_empty());
        let store = HistoryStore::open(&database).unwrap();
        assert_eq!(store.get(&first.id).unwrap(), Some(first.clone()));
        assert_eq!(store.get(&second.id).unwrap(), Some(second));
        assert_eq!(store.page("", None, 50).unwrap().len(), 2);
        assert_eq!(fs::read_to_string(last).unwrap(), first.text);
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn last_is_exact_private_atomic_and_failure_preserves_old() {
        let root = std::env::temp_dir().join(format!(
            "koett-last-{}-{}",
            std::process::id(),
            NEXT_FAILED_TRANSCRIPT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("last");
        let text = "  Åäö 世界 🦀\n\n trailing ";
        save_last_to(&path, text).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), text);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o500)).unwrap();
            let failed = save_last_to(&path, "must not replace");
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
            assert!(failed.is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), text);
        }
        let unavailable = path.join("child");
        assert!(save_last_to(&unavailable, "replacement").is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), text);
        let mut state = Recovery::default();
        let record = new_record(OffsetDateTime::now_utc(), "model", "new", None).unwrap();
        let error = persist_to(
            &mut state,
            record,
            true,
            &root.join("db"),
            &root.join("fallback"),
            &unavailable,
        )
        .unwrap_err();
        assert!(error.starts_with("Transcript saved · Last action in memory"));
        assert!(state.pending.is_empty());
        assert_eq!(state.last.as_deref(), Some("new"));
        fs::remove_dir_all(root).unwrap();
    }
}
