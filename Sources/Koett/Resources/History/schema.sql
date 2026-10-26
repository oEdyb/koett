CREATE TABLE IF NOT EXISTS transcripts (
    id TEXT PRIMARY KEY NOT NULL,
    created_at_ms INTEGER NOT NULL,
    model TEXT NOT NULL,
    text TEXT NOT NULL,
    raw_text TEXT
);
CREATE INDEX IF NOT EXISTS transcripts_chronological
    ON transcripts(created_at_ms DESC, id DESC);
CREATE VIRTUAL TABLE IF NOT EXISTS transcripts_fts USING fts5(
    text, raw_text, content='transcripts', content_rowid='rowid',
    tokenize='unicode61 remove_diacritics 2'
);
CREATE TRIGGER IF NOT EXISTS transcripts_insert AFTER INSERT ON transcripts BEGIN
    INSERT INTO transcripts_fts(rowid, text, raw_text)
    VALUES (new.rowid, new.text, new.raw_text);
END;
CREATE TRIGGER IF NOT EXISTS transcripts_delete AFTER DELETE ON transcripts BEGIN
    INSERT INTO transcripts_fts(transcripts_fts, rowid, text, raw_text)
    VALUES ('delete', old.rowid, old.text, old.raw_text);
END;
CREATE TRIGGER IF NOT EXISTS transcripts_update AFTER UPDATE ON transcripts BEGIN
    INSERT INTO transcripts_fts(transcripts_fts, rowid, text, raw_text)
    VALUES ('delete', old.rowid, old.text, old.raw_text);
    INSERT INTO transcripts_fts(rowid, text, raw_text)
    VALUES (new.rowid, new.text, new.raw_text);
END;
CREATE TABLE IF NOT EXISTS history_imports (
    source_key TEXT PRIMARY KEY NOT NULL,
    byte_count INTEGER NOT NULL,
    entry_count INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS legacy_fragments (
    source_key TEXT NOT NULL,
    byte_offset INTEGER NOT NULL,
    content BLOB NOT NULL,
    reason TEXT NOT NULL,
    PRIMARY KEY(source_key, byte_offset)
);
CREATE TABLE IF NOT EXISTS history_sources (
    source_key TEXT PRIMARY KEY NOT NULL,
    version_key TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS history_import_records (
    version_key TEXT NOT NULL,
    transcript_id TEXT NOT NULL,
    PRIMARY KEY(version_key, transcript_id)
);
CREATE INDEX IF NOT EXISTS history_import_records_transcript
    ON history_import_records(transcript_id);
CREATE TABLE IF NOT EXISTS history_native_records (
    transcript_id TEXT PRIMARY KEY NOT NULL
);
PRAGMA user_version = 2;
