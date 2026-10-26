use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::paths;

static NEXT_FAILED_TRANSCRIPT: AtomicU64 = AtomicU64::new(0);

pub fn append_transcript(model: &str, text: &str) -> Result<(), String> {
    append_transcript_to(
        &paths::history_file()?,
        OffsetDateTime::now_utc(),
        model,
        text,
    )
}

pub fn append_transcript_resilient(model: &str, text: &str) -> Result<Option<PathBuf>, String> {
    let primary = paths::history_file()?;
    append_transcript_with_fallback_to(
        &primary,
        &paths::failed_transcripts_directory()?,
        OffsetDateTime::now_utc(),
        model,
        text,
    )
}

fn append_transcript_with_fallback_to(
    primary: &Path,
    fallback_directory: &Path,
    timestamp: OffsetDateTime,
    model: &str,
    text: &str,
) -> Result<Option<PathBuf>, String> {
    match append_transcript_to(primary, timestamp, model, text) {
        Ok(()) => Ok(None),
        Err(primary_error) => {
            let fallback = save_failed_transcript_to(fallback_directory, timestamp, model, text)
                .map_err(|fallback_error| {
                    format!(
                        "the main transcript history failed: {primary_error}; the fallback also failed: {fallback_error}"
                    )
                })?;
            Ok(Some(fallback))
        }
    }
}

fn save_failed_transcript_to(
    directory: &Path,
    timestamp: OffsetDateTime,
    model: &str,
    text: &str,
) -> Result<PathBuf, String> {
    fs::create_dir_all(directory)
        .map_err(|error| format!("could not create {}: {error}", directory.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("could not protect {}: {error}", directory.display()))?;
    }
    let entry = transcript_entry(timestamp, model, text)?;
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

    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    let entry = transcript_entry(timestamp, model, text)?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("could not open {}: {error}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("could not protect {}: {error}", path.display()))?;
    }
    file.write_all(entry.as_bytes())
        .map_err(|error| format!("could not write {}: {error}", path.display()))
}

fn transcript_entry(timestamp: OffsetDateTime, model: &str, text: &str) -> Result<String, String> {
    let timestamp = timestamp
        .format(&Rfc3339)
        .map_err(|error| format!("could not format transcript time: {error}"))?;
    Ok(format!(
        "## {timestamp}\n\nModel: {model}\n\n{}\n\n",
        text.trim()
    ))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use time::macros::datetime;

    use super::{append_transcript_to, append_transcript_with_fallback_to};

    static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

    fn temporary_file() -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("koett-history-{nonce}-{sequence}.md"))
    }

    #[test]
    fn history_appends_without_replacing_earlier_text() {
        let path = temporary_file();
        let timestamp = datetime!(2026-08-24 12:30 UTC);

        append_transcript_to(&path, timestamp, "Parakeet 110M", "First.").unwrap();
        append_transcript_to(&path, timestamp, "Parakeet 110M", "Second.").unwrap();

        let text = fs::read_to_string(&path).unwrap();
        assert_eq!(text.matches("## 2026-08-24T12:30:00Z").count(), 2);
        assert!(text.contains("First."));
        assert!(text.contains("Second."));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn empty_transcript_creates_no_file() {
        let path = temporary_file();

        append_transcript_to(
            &path,
            datetime!(2026-08-24 12:30 UTC),
            "Parakeet 110M",
            "  ",
        )
        .unwrap();

        assert!(!path.exists());
    }

    #[test]
    fn failed_primary_history_uses_the_fallback() {
        let primary = temporary_file();
        let fallback_directory = temporary_file();
        fs::create_dir(&primary).unwrap();

        let used = append_transcript_with_fallback_to(
            &primary,
            &fallback_directory,
            datetime!(2026-08-24 12:30 UTC),
            "Parakeet 110M",
            "Saved safely.",
        )
        .unwrap();

        let fallback = used.unwrap();
        assert_eq!(fallback.parent(), Some(fallback_directory.as_path()));
        assert!(
            fs::read_to_string(&fallback)
                .unwrap()
                .contains("Saved safely.")
        );
        fs::remove_dir(primary).unwrap();
        fs::remove_dir_all(fallback_directory).unwrap();
    }
}
