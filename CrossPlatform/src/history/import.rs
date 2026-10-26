//! Conservative streaming migration. Originals are never modified. Every byte is
//! retained in bounded fragments, including valid entries, for lossless review.
use super::store::{HistoryRecord, HistoryStore, error};
use base64::prelude::{BASE64_STANDARD, Engine as _};
use rusqlite::{OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::Path,
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
const CHUNK: usize = 256 * 1024;
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
impl HistoryStore {
    /// Returns the number of records/chunks, including previously imported ones.
    /// Ambiguous UTF-8 becomes explicitly labeled archive text with unknown date.
    /// Every original byte also remains available in legacy_fragments.
    pub fn import_legacy(&self, path: impl AsRef<Path>) -> Result<usize, String> {
        let path = path.as_ref().canonicalize().map_err(error)?;
        let source_key = format!(
            "legacy-path:{}",
            path.to_str()
                .ok_or("legacy source path is not valid UTF-8")?
        );
        let mut file = File::open(&path).map_err(error)?;
        // Content versions are shared across paths; active ownership is per source.
        let mut digest = Sha256::new();
        let mut buffer = vec![0; CHUNK];
        loop {
            let n = file.read(&mut buffer).map_err(error)?;
            if n == 0 {
                break;
            }
            digest.update(&buffer[..n]);
        }
        let content_hash = hex(&digest.finalize());
        // Version completion identity so archives imported by the earlier
        // preservation-only importer are indexed on their next explicit import.
        let source = format!("searchable-archive-v3:{content_hash}");
        let transaction = self.connection.unchecked_transaction().map_err(error)?;
        let complete: Option<i64> = self
            .connection
            .query_row(
                "SELECT entry_count FROM history_imports WHERE source_key=?1",
                [&source],
                |row| row.get(0),
            )
            .optional()
            .map_err(error)?;
        let missing: bool = self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM history_import_records r LEFT JOIN transcripts t ON t.id=r.transcript_id WHERE r.version_key=?1 AND t.id IS NULL)", [&source], |r| r.get(0)
        ).map_err(error)?;
        if let Some(count) = complete.filter(|_| !missing) {
            self.activate_version(&source_key, &source)?;
            transaction.commit().map_err(error)?;
            return usize::try_from(count).map_err(error);
        }
        file.seek(SeekFrom::Start(0)).map_err(error)?;
        let mut reader = BufReader::new(file);
        let mut offset = 0i64;
        let mut block_offset = 0i64;
        let mut block = Vec::new();
        let mut count = 0usize;
        let mut oversized = false;
        let mut second_digest = Sha256::new();
        let mut at_line_start = true;
        loop {
            let line = read_bounded_line(&mut reader)?;
            let n = line.len();
            if n == 0 {
                break;
            }
            second_digest.update(&line);
            let line_start = at_line_start;
            at_line_start = line.ends_with(b"\n");
            let boundary = line_start
                && !oversized
                && (line.starts_with(b"<!-- koett-transcript:")
                    || (line.starts_with(b"## ") && block.is_empty()));
            let marker_boundary = line_start && line.starts_with(b"<!-- koett-transcript:");
            // Unmarked Markdown cannot distinguish a dictated timestamp heading from
            // an actual record boundary. Keep it as one block and reject ambiguity.
            if marker_boundary && !block.is_empty() {
                count += self.import_block(&source, block_offset, &block, oversized)?;
                block.clear();
                block_offset = offset;
                oversized = false;
            } else if boundary && block.iter().all(u8::is_ascii_whitespace) {
                self.preserve(&source, block_offset, &block, "legacy bytes")?;
                block.clear();
                block_offset = offset;
            }
            if block.len() + n > CHUNK {
                count += self.import_block(&source, block_offset, &block, true)?;
                block.clear();
                block_offset = offset;
                oversized = true;
            }
            block.extend_from_slice(&line);
            offset += i64::try_from(n).map_err(error)?;
        }
        count += self.import_block(&source, block_offset, &block, oversized)?;
        if hex(&second_digest.finalize()) != content_hash {
            return Err(
                "legacy source changed during import; retry to import its current contents".into(),
            );
        }
        self.connection.execute("INSERT INTO history_imports(source_key,byte_count,entry_count) VALUES (?1,?2,?3) ON CONFLICT(source_key) DO UPDATE SET byte_count=excluded.byte_count,entry_count=excluded.entry_count", params![source,offset,i64::try_from(count).map_err(error)?]).map_err(error)?;
        self.activate_version(&source_key, &source)?;
        transaction.commit().map_err(error)?;
        Ok(count)
    }
    fn activate_version(&self, source: &str, version: &str) -> Result<(), String> {
        let previous: Option<String> = self
            .connection
            .query_row(
                "SELECT version_key FROM history_sources WHERE source_key=?1",
                [source],
                |r| r.get(0),
            )
            .optional()
            .map_err(error)?;
        self.connection.execute("INSERT INTO history_sources(source_key,version_key) VALUES (?1,?2) ON CONFLICT(source_key) DO UPDATE SET version_key=excluded.version_key", params![source,version]).map_err(error)?;
        if let Some(previous) = previous {
            self.connection.execute("DELETE FROM transcripts WHERE id IN (SELECT transcript_id FROM history_import_records WHERE version_key=?1) AND NOT EXISTS (SELECT 1 FROM history_import_records r JOIN history_sources s ON s.version_key=r.version_key WHERE r.transcript_id=transcripts.id) AND NOT EXISTS (SELECT 1 FROM history_native_records n WHERE n.transcript_id=transcripts.id)", [previous]).map_err(error)?;
        }
        Ok(())
    }
    fn import_record(&self, version: &str, record: &HistoryRecord) -> Result<(), String> {
        self.insert_record(record)?;
        self.connection.execute("INSERT INTO history_import_records(version_key,transcript_id) VALUES (?1,?2) ON CONFLICT DO NOTHING", params![version,record.id]).map_err(error)?;
        Ok(())
    }
    fn preserve(
        &self,
        source: &str,
        offset: i64,
        bytes: &[u8],
        reason: &str,
    ) -> Result<(), String> {
        if bytes.is_empty() {
            return Ok(());
        }
        self.connection.execute("INSERT INTO legacy_fragments(source_key,byte_offset,content,reason) VALUES (?1,?2,?3,?4) ON CONFLICT DO NOTHING", params![source,offset,bytes,reason]).map_err(error)?;
        Ok(())
    }
    fn import_block(
        &self,
        source: &str,
        offset: i64,
        bytes: &[u8],
        oversized: bool,
    ) -> Result<usize, String> {
        let Ok(value) = std::str::from_utf8(bytes) else {
            self.preserve(
                source,
                offset,
                bytes,
                "invalid UTF-8; review original bytes",
            )?;
            return Ok(0);
        };
        let record = if !oversized {
            parse(value, offset)
        } else {
            None
        };
        let Some(record) = record else {
            self.preserve(
                source,
                offset,
                bytes,
                "unrecognized or oversized UTF-8; indexed as Imported archive",
            )?;
            if value.trim().is_empty() {
                return Ok(0);
            }
            self.import_record(
                source,
                &HistoryRecord {
                    id: format!("archive-{}-{offset}", hex(&Sha256::digest(bytes))),
                    created_at_ms: 0,
                    model: "Imported archive".into(),
                    text: value.to_owned(),
                    raw_text: None,
                },
            )?;
            return Ok(1);
        };
        self.preserve(
            source,
            offset,
            bytes,
            "original bytes of recognized transcript",
        )?;
        self.import_record(source, &record)?;
        Ok(1)
    }
}
/// Reserve three bytes to complete a four-byte UTF-8 scalar at a read boundary.
/// Never decode lossily: malformed bytes remain raw fragments for review.
fn read_bounded_line(reader: &mut impl BufRead) -> Result<Vec<u8>, String> {
    let mut line = Vec::new();
    reader
        .take(u64::try_from(CHUNK - 3).map_err(error)?)
        .read_until(b'\n', &mut line)
        .map_err(error)?;
    for _ in 0..3 {
        let incomplete = std::str::from_utf8(&line).is_err_and(|e| e.error_len().is_none());
        if !incomplete {
            break;
        }
        let mut byte = [0];
        if reader.read(&mut byte).map_err(error)? == 0 {
            break;
        }
        line.push(byte[0]);
    }
    Ok(line)
}

fn parse(value: &str, offset: i64) -> Option<HistoryRecord> {
    let value = value.trim_matches('\n');
    let value = value
        .strip_prefix("# Koett Transcripts\n\nSaved locally by Koett. Audio is not stored.\n")
        .unwrap_or(value)
        .trim_start_matches('\n');
    let (marker, value) = if value.starts_with("<!-- koett-transcript:") {
        let (marker, rest) = value.split_once('\n')?;
        (Some(marker), rest)
    } else {
        (None, value)
    };
    let (heading, rest) = value.split_once("\n\nModel: ")?;
    let date = OffsetDateTime::parse(heading.strip_prefix("## ")?, &Rfc3339).ok()?;
    let (model, body) = rest.split_once('\n')?;
    let body = body.strip_prefix('\n')?.trim_matches('\n');
    if body.lines().any(|line| {
        line.starts_with("<!-- koett-transcript:")
            || (marker.is_none()
                && line.starts_with("## ")
                && OffsetDateTime::parse(&line[3..], &Rfc3339).is_ok())
    }) {
        return None;
    }
    let (text, raw_text) = if let Some(raw) = body.strip_prefix("Raw transcript:\n\n") {
        if raw.matches("\n\nCleaned transcript:\n\n").count() != 1 {
            return None;
        }
        let (raw, cleaned) = raw.split_once("\n\nCleaned transcript:\n\n")?;
        (
            if cleaned == "(empty)" { "" } else { cleaned },
            Some(raw.to_owned()),
        )
    } else {
        (body, None)
    };
    if text.trim().is_empty() && raw_text.as_deref().unwrap_or("").trim().is_empty() {
        return None;
    }
    // Marker identity survives append/retry. Unmarked entries include offset to
    // retain legitimate repeated dictations, without inventing model/time metadata.
    let id = if let Some(marker) = marker {
        let encoded = marker
            .strip_prefix("<!-- koett-transcript:")?
            .strip_suffix(" -->")?;
        let id = String::from_utf8(BASE64_STANDARD.decode(encoded).ok()?).ok()?;
        if id.is_empty() {
            return None;
        }
        id
    } else {
        format!(
            "legacy-{}",
            hex(&Sha256::digest(format!("{offset}:{value}").as_bytes()))
        )
    };
    Some(HistoryRecord {
        id,
        created_at_ms: i64::try_from(date.unix_timestamp_nanos() / 1_000_000).ok()?,
        model: model.to_owned(),
        text: text.to_owned(),
        raw_text,
    })
}
