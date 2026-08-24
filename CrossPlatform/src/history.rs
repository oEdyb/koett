use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::paths;

pub fn append_transcript(model: &str, text: &str) -> Result<(), String> {
    append_transcript_to(
        &paths::history_file()?,
        OffsetDateTime::now_utc(),
        model,
        text,
    )
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
    let timestamp = timestamp
        .format(&Rfc3339)
        .map_err(|error| format!("could not format transcript time: {error}"))?;
    let entry = format!("## {timestamp}\n\nModel: {model}\n\n{}\n\n", text.trim());
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("could not open {}: {error}", path.display()))?;
    file.write_all(entry.as_bytes())
        .map_err(|error| format!("could not write {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use time::macros::datetime;

    use super::append_transcript_to;

    fn temporary_file() -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("koett-history-{nonce}.md"))
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
}
