use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::paths;

#[path = "history/import.rs"]
mod import;
#[path = "history/store.rs"]
mod store;
pub use store::{HistoryCursor, HistoryRecord, HistoryStore, HistorySummary};

#[cfg(test)]
#[path = "history/tests.rs"]
mod tests;

static NEXT_FAILED_TRANSCRIPT: AtomicU64 = AtomicU64::new(0);

pub fn append_transcript(model: &str, text: &str) -> Result<(), String> {
    append_transcript_to(
        &paths::history_database()?,
        OffsetDateTime::now_utc(),
        model,
        text,
    )
}

pub fn append_transcript_resilient(model: &str, text: &str) -> Result<Option<PathBuf>, String> {
    append_transcript_with_raw_resilient(model, text, None)
}

pub fn append_transcript_with_raw_resilient(
    model: &str,
    text: &str,
    raw_text: Option<&str>,
) -> Result<Option<PathBuf>, String> {
    if text.trim().is_empty() && raw_text.unwrap_or("").trim().is_empty() {
        return Ok(None);
    }
    append_record_with_fallback_to(
        &paths::history_database()?,
        &paths::failed_transcripts_directory()?,
        &new_record(OffsetDateTime::now_utc(), model, text, raw_text)?,
    )
}

#[cfg(test)]
fn append_transcript_with_fallback_to(
    primary: &Path,
    fallback_directory: &Path,
    timestamp: OffsetDateTime,
    model: &str,
    text: &str,
) -> Result<Option<PathBuf>, String> {
    append_record_with_fallback_to(
        primary,
        fallback_directory,
        &new_record(timestamp, model, text, None)?,
    )
}

fn append_record_with_fallback_to(
    primary: &Path,
    fallback_directory: &Path,
    record: &HistoryRecord,
) -> Result<Option<PathBuf>, String> {
    if record.text.trim().is_empty() && record.raw_text.as_deref().unwrap_or("").trim().is_empty() {
        return Ok(None);
    }
    match HistoryStore::open(primary).and_then(|store| store.save(record)) {
        Ok(()) => Ok(None),
        Err(primary_error) => {
            let fallback = save_failed_transcript_to(fallback_directory, record)
                .map_err(|fallback_error| format!("the main transcript history failed: {primary_error}; the fallback also failed: {fallback_error}"))?;
            Ok(Some(fallback))
        }
    }
}

pub fn save_media_recovery(model: &str, text: &str) -> Result<PathBuf, String> {
    save_failed_transcript_to(
        &paths::failed_transcripts_directory()?,
        &new_record(OffsetDateTime::now_utc(), model, text, None)?,
    )
}

fn save_failed_transcript_to(directory: &Path, record: &HistoryRecord) -> Result<PathBuf, String> {
    fs::create_dir_all(directory)
        .map_err(|error| format!("could not create {}: {error}", directory.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("could not protect {}: {error}", directory.display()))?;
    }
    let timestamp =
        OffsetDateTime::from_unix_timestamp_nanos(i128::from(record.created_at_ms) * 1_000_000)
            .map_err(|e| e.to_string())?;
    let entry = transcript_entry(record)?;
    for _ in 0..100 {
        let sequence = NEXT_FAILED_TRANSCRIPT.fetch_add(1, Ordering::Relaxed);
        let path = directory.join(format!(
            "Transcript-{}-{}-{sequence}.md",
            timestamp.unix_timestamp_nanos(),
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(mut file) => {
                file.write_all(entry.as_bytes())
                    .and_then(|_| file.sync_all())
                    .map_err(|error| format!("could not write {}: {error}", path.display()))?;
                #[cfg(unix)]
                fs::File::open(directory)
                    .and_then(|directory| directory.sync_all())
                    .map_err(|error| format!("could not sync {}: {error}", directory.display()))?;
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!("could not create {}: {error}", path.display()));
            }
        }
    }
    Err("could not find a unique failed-transcript filename".to_string())
}

pub fn append_transcript_to(
    path: &Path,
    timestamp: OffsetDateTime,
    model: &str,
    text: &str,
) -> Result<(), String> {
    if text.trim().is_empty() {
        return Ok(());
    }

    HistoryStore::open(path)?.save(&new_record(timestamp, model, text, None)?)
}

fn new_record(
    timestamp: OffsetDateTime,
    model: &str,
    text: &str,
    raw_text: Option<&str>,
) -> Result<HistoryRecord, String> {
    Ok(HistoryRecord {
        id: format!(
            "rust-{}-{}-{}",
            timestamp.unix_timestamp_nanos(),
            std::process::id(),
            NEXT_FAILED_TRANSCRIPT.fetch_add(1, Ordering::Relaxed)
        ),
        created_at_ms: i64::try_from(timestamp.unix_timestamp_nanos() / 1_000_000)
            .map_err(|e| e.to_string())?,
        model: model.to_owned(),
        text: text.to_owned(),
        raw_text: raw_text.map(str::to_owned),
    })
}

fn transcript_entry(record: &HistoryRecord) -> Result<String, String> {
    use base64::prelude::{BASE64_STANDARD, Engine as _};
    let timestamp =
        OffsetDateTime::from_unix_timestamp_nanos(i128::from(record.created_at_ms) * 1_000_000)
            .map_err(|e| e.to_string())?
            .format(&Rfc3339)
            .map_err(|e| e.to_string())?;
    let body = match &record.raw_text {
        Some(raw) => format!(
            "Raw transcript:\n\n{raw}\n\nCleaned transcript:\n\n{}",
            if record.text.is_empty() {
                "(empty)"
            } else {
                &record.text
            }
        ),
        None => record.text.clone(),
    };
    Ok(format!(
        "<!-- koett-transcript:{} -->\n## {timestamp}\n\nModel: {}\n\n{body}\n\n",
        BASE64_STANDARD.encode(record.id.as_bytes()),
        record.model
    ))
}
