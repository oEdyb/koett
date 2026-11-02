use crate::paths;
use rusqlite::{Connection, OptionalExtension, params};
use std::{
    fs::{self, OpenOptions},
    path::Path,
    time::Duration,
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryRecord {
    pub id: String,
    pub created_at_ms: i64,
    pub model: String,
    pub text: String,
    pub raw_text: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistorySummary {
    pub id: String,
    pub created_at_ms: i64,
    pub model: String,
    pub preview: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryCursor {
    pub created_at_ms: i64,
    pub id: String,
}
pub struct HistoryStore {
    pub(super) connection: Connection,
}
pub(super) fn error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
impl HistoryStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty())
            && !parent.exists()
        {
            fs::create_dir_all(parent).map_err(error)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).map_err(error)?;
            }
        }
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(path).map_err(error)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(error)?;
        }
        drop(file);
        let connection = Connection::open(path).map_err(error)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(error)?;
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(error)?;
        if version > 2 {
            return Err(format!("unsupported history schema version {version}"));
        }
        connection.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; PRAGMA temp_store=MEMORY; PRAGMA cache_size=-4096;").map_err(error)?;
        if version < 2 {
            let transaction = connection.unchecked_transaction().map_err(error)?;
            transaction
                .execute_batch(include_str!(
                    "../../../Sources/Koett/Resources/History/schema.sql"
                ))
                .map_err(error)?;
            if version == 1 {
                // Earlier schemas did not track provenance. Existing rows may be
                // native recordings, so preserve them rather than infer ownership.
                transaction.execute("INSERT INTO history_native_records(transcript_id) SELECT id FROM transcripts WHERE true ON CONFLICT DO NOTHING", []).map_err(error)?;
            }
            transaction.commit().map_err(error)?;
        }
        Ok(Self { connection })
    }
    /// Opens SQLite only; never reads or hashes the legacy archive.
    pub fn open_default() -> Result<Self, String> {
        Self::open(paths::history_database()?)
    }
    /// Call once on a background startup worker, independently of page/get requests.
    pub fn import_default_legacy(&self) -> Result<usize, String> {
        let legacy = paths::history_file()?;
        match std::fs::metadata(&legacy) {
            Ok(_) => self.import_legacy(legacy),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
            Err(error) => Err(error.to_string()),
        }
    }
    pub fn save(&self, record: &HistoryRecord) -> Result<(), String> {
        if record.text.trim().is_empty()
            && record.raw_text.as_deref().unwrap_or("").trim().is_empty()
        {
            return Ok(());
        }
        if record.id.is_empty() {
            return Err("history ID cannot be empty".into());
        }
        let transaction = self.connection.unchecked_transaction().map_err(error)?;
        self.insert_record(record)?;
        transaction.execute("INSERT INTO history_native_records(transcript_id) VALUES (?1) ON CONFLICT DO NOTHING", [&record.id]).map_err(error)?;
        transaction.commit().map_err(error)
    }
    pub(super) fn insert_record(&self, record: &HistoryRecord) -> Result<(), String> {
        let inserted = self.connection.execute("INSERT INTO transcripts(id,created_at_ms,model,text,raw_text) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(id) DO NOTHING", params![record.id,record.created_at_ms,record.model,record.text,record.raw_text]).map_err(error)?;
        if inserted == 0 && self.get(&record.id)?.as_ref() != Some(record) {
            return Err(format!("conflicting history ID: {}", record.id));
        }
        Ok(())
    }
    pub fn get(&self, id: &str) -> Result<Option<HistoryRecord>, String> {
        self.connection
            .query_row(
                "SELECT id,created_at_ms,model,text,raw_text FROM transcripts WHERE id=?1",
                [id],
                |r| {
                    Ok(HistoryRecord {
                        id: r.get(0)?,
                        created_at_ms: r.get(1)?,
                        model: r.get(2)?,
                        text: r.get(3)?,
                        raw_text: r.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(error)
    }
    pub fn page(
        &self,
        query: &str,
        before: Option<&HistoryCursor>,
        limit: usize,
    ) -> Result<Vec<HistorySummary>, String> {
        let terms: Vec<_> = query
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .map(|s| format!("\"{s}\""))
            .collect();
        if !query.trim().is_empty() && terms.is_empty() {
            return Ok(Vec::new());
        }
        let search = terms.join(" AND ");
        let source = if search.is_empty() {
            "transcripts t"
        } else {
            "transcripts t JOIN transcripts_fts f ON f.rowid=t.rowid"
        };
        let search_clause = if search.is_empty() {
            "?1=''"
        } else {
            "transcripts_fts MATCH ?1"
        };
        let sql = format!(
            "SELECT t.id,t.created_at_ms,t.model,substr(t.text,1,240) FROM {source} WHERE {search_clause} AND (?2 IS NULL OR (t.created_at_ms,t.id)<(?2,?3)) ORDER BY t.created_at_ms DESC,t.id DESC LIMIT ?4"
        );
        let mut statement = self.connection.prepare(&sql).map_err(error)?;
        statement
            .query_map(
                params![
                    search,
                    before.map(|c| c.created_at_ms),
                    before.map(|c| &c.id),
                    i64::try_from(limit.min(50)).map_err(error)?
                ],
                |r| {
                    Ok(HistorySummary {
                        id: r.get(0)?,
                        created_at_ms: r.get(1)?,
                        model: r.get(2)?,
                        preview: r.get(3)?,
                    })
                },
            )
            .map_err(error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(error)
    }
}
