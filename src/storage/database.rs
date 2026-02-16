//! SQLite database management with FTS5 support

use anyhow::{Context, Result};
use chrono::{TimeZone, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

use crate::config::Settings;
use crate::storage::models::{Recording, RecordingState, TranscriptSegment};

/// Database wrapper for minutes
pub struct Database {
    conn: Connection,
}

const CURRENT_SCHEMA_VERSION: i64 = 1;

impl Database {
    /// Open or create the database
    pub fn open(settings: &Settings) -> Result<Self> {
        let db_path = settings.database_path();

        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        Self::open_path(&db_path)
    }

    /// Open database at a specific path (useful for testing)
    pub fn open_path(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open database: {}", path.display()))?;

        let db = Self { conn };
        db.initialize()?;

        Ok(db)
    }

    /// Open an in-memory database (for testing)
    #[cfg(test)]
    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.initialize()?;
        Ok(db)
    }

    /// Initialize database schema
    fn initialize(&self) -> Result<()> {
        // Enable foreign keys
        self.conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        let current_version = self.schema_version()?;
        if current_version > CURRENT_SCHEMA_VERSION {
            anyhow::bail!(
                "Database schema version {} is newer than supported version {}",
                current_version,
                CURRENT_SCHEMA_VERSION
            );
        }

        if current_version < 1 {
            self.migrate_to_v1()?;
            self.set_schema_version(1)?;
        }

        Ok(())
    }

    /// Current schema version tracked in PRAGMA user_version.
    pub fn schema_version(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("PRAGMA user_version;", [], |row| row.get(0))?)
    }

    fn set_schema_version(&self, version: i64) -> Result<()> {
        self.conn
            .execute(&format!("PRAGMA user_version = {}", version), [])?;
        Ok(())
    }

    fn migrate_to_v1(&self) -> Result<()> {
        // Create recordings table
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS recordings (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                audio_path TEXT,
                duration_secs INTEGER,
                state TEXT NOT NULL DEFAULT 'recording',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                notes TEXT,
                tags TEXT DEFAULT '[]'
            );

            CREATE INDEX IF NOT EXISTS idx_recordings_created_at
                ON recordings(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_recordings_state
                ON recordings(state);
            "#,
        )?;

        // Create transcript segments table
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS transcript_segments (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                recording_id TEXT NOT NULL,
                start_time REAL NOT NULL,
                end_time REAL NOT NULL,
                text TEXT NOT NULL,
                speaker TEXT,
                confidence REAL,
                FOREIGN KEY (recording_id) REFERENCES recordings(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_segments_recording_id
                ON transcript_segments(recording_id);
            CREATE INDEX IF NOT EXISTS idx_segments_start_time
                ON transcript_segments(recording_id, start_time);
            "#,
        )?;

        // Create FTS5 virtual table for full-text search
        self.conn.execute_batch(
            r#"
            CREATE VIRTUAL TABLE IF NOT EXISTS transcript_fts USING fts5(
                recording_id,
                text,
                content='transcript_segments',
                content_rowid='id',
                tokenize='porter unicode61'
            );

            -- Triggers to keep FTS index in sync
            CREATE TRIGGER IF NOT EXISTS transcript_ai AFTER INSERT ON transcript_segments BEGIN
                INSERT INTO transcript_fts(rowid, recording_id, text)
                VALUES (new.id, new.recording_id, new.text);
            END;

            CREATE TRIGGER IF NOT EXISTS transcript_ad AFTER DELETE ON transcript_segments BEGIN
                INSERT INTO transcript_fts(transcript_fts, rowid, recording_id, text)
                VALUES ('delete', old.id, old.recording_id, old.text);
            END;

            CREATE TRIGGER IF NOT EXISTS transcript_au AFTER UPDATE ON transcript_segments BEGIN
                INSERT INTO transcript_fts(transcript_fts, rowid, recording_id, text)
                VALUES ('delete', old.id, old.recording_id, old.text);
                INSERT INTO transcript_fts(rowid, recording_id, text)
                VALUES (new.id, new.recording_id, new.text);
            END;
            "#,
        )?;

        Ok(())
    }

    /// Insert a new recording
    pub fn insert_recording(&self, recording: &Recording) -> Result<()> {
        let tags_json = serde_json::to_string(&recording.tags)?;

        self.conn.execute(
            r#"
            INSERT INTO recordings (id, title, audio_path, duration_secs, state, created_at, updated_at, notes, tags)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            params![
                recording.id,
                recording.title,
                recording.audio_path,
                recording.duration_secs,
                recording.state.as_str(),
                recording.created_at.timestamp(),
                recording.updated_at.timestamp(),
                recording.notes,
                tags_json,
            ],
        )?;

        Ok(())
    }

    /// Update a recording
    pub fn update_recording(&self, recording: &Recording) -> Result<()> {
        let tags_json = serde_json::to_string(&recording.tags)?;

        self.conn.execute(
            r#"
            UPDATE recordings
            SET title = ?2, audio_path = ?3, duration_secs = ?4, state = ?5,
                updated_at = ?6, notes = ?7, tags = ?8
            WHERE id = ?1
            "#,
            params![
                recording.id,
                recording.title,
                recording.audio_path,
                recording.duration_secs,
                recording.state.as_str(),
                Utc::now().timestamp(),
                recording.notes,
                tags_json,
            ],
        )?;

        Ok(())
    }

    /// Get a recording by ID
    pub fn get_recording(&self, id: &str) -> Result<Option<Recording>> {
        let result = self.conn.query_row(
            "SELECT id, title, audio_path, duration_secs, state, created_at, updated_at, notes, tags FROM recordings WHERE id = ?1",
            params![id],
            |row| Ok(Self::row_to_recording(row)),
        ).optional()?;

        match result {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    /// Find a recording by ID prefix
    pub fn find_recording_by_prefix(&self, prefix: &str) -> Result<Option<Recording>> {
        let pattern = format!("{}%", prefix);

        let result = self.conn.query_row(
            "SELECT id, title, audio_path, duration_secs, state, created_at, updated_at, notes, tags FROM recordings WHERE id LIKE ?1 LIMIT 1",
            params![pattern],
            |row| Ok(Self::row_to_recording(row)),
        ).optional()?;

        match result {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    /// List recordings ordered by creation date
    pub fn list_recordings(&self, limit: usize) -> Result<Vec<Recording>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, audio_path, duration_secs, state, created_at, updated_at, notes, tags
             FROM recordings
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;

        let recordings = stmt
            .query_map(params![limit], |row| Ok(Self::row_to_recording(row)))?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .collect::<Result<Vec<_>>>()?;

        Ok(recordings)
    }

    /// Search recordings by title
    pub fn search_recordings(&self, query: &str, limit: usize) -> Result<Vec<Recording>> {
        let pattern = format!("%{}%", query);

        let mut stmt = self.conn.prepare(
            "SELECT id, title, audio_path, duration_secs, state, created_at, updated_at, notes, tags
             FROM recordings
             WHERE title LIKE ?1
             ORDER BY created_at DESC
             LIMIT ?2",
        )?;

        let recordings = stmt
            .query_map(params![pattern, limit], |row| {
                Ok(Self::row_to_recording(row))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .collect::<Result<Vec<_>>>()?;

        Ok(recordings)
    }

    /// Delete a recording and its segments
    pub fn delete_recording(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM recordings WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Insert a transcript segment
    pub fn insert_segment(&self, segment: &TranscriptSegment) -> Result<i64> {
        self.conn.execute(
            r#"
            INSERT INTO transcript_segments (recording_id, start_time, end_time, text, speaker, confidence)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                segment.recording_id,
                segment.start_time,
                segment.end_time,
                segment.text,
                segment.speaker,
                segment.confidence,
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Insert multiple segments in a transaction
    pub fn insert_segments(&self, segments: &[TranscriptSegment]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;

        for segment in segments {
            tx.execute(
                r#"
                INSERT INTO transcript_segments (recording_id, start_time, end_time, text, speaker, confidence)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                "#,
                params![
                    segment.recording_id,
                    segment.start_time,
                    segment.end_time,
                    segment.text,
                    segment.speaker,
                    segment.confidence,
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    /// Get transcript segments for a recording
    pub fn get_transcript_segments(&self, recording_id: &str) -> Result<Vec<TranscriptSegment>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, recording_id, start_time, end_time, text, speaker, confidence
             FROM transcript_segments
             WHERE recording_id = ?1
             ORDER BY start_time",
        )?;

        let segments = stmt
            .query_map(params![recording_id], |row| {
                Ok(TranscriptSegment {
                    id: row.get(0)?,
                    recording_id: row.get(1)?,
                    start_time: row.get(2)?,
                    end_time: row.get(3)?,
                    text: row.get(4)?,
                    speaker: row.get(5)?,
                    confidence: row.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(segments)
    }

    /// Full-text search across transcripts
    pub fn search_transcripts(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<(Recording, TranscriptSegment)>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT
                r.id, r.title, r.audio_path, r.duration_secs, r.state, r.created_at, r.updated_at, r.notes, r.tags,
                s.id, s.recording_id, s.start_time, s.end_time, s.text, s.speaker, s.confidence
            FROM transcript_fts f
            JOIN transcript_segments s ON f.rowid = s.id
            JOIN recordings r ON s.recording_id = r.id
            WHERE transcript_fts MATCH ?1
            ORDER BY rank
            LIMIT ?2
            "#,
        )?;

        let results = stmt
            .query_map(params![query, limit], |row| {
                let recording = Self::row_to_recording_offset(row, 0)?;
                let segment = TranscriptSegment {
                    id: row.get(9)?,
                    recording_id: row.get(10)?,
                    start_time: row.get(11)?,
                    end_time: row.get(12)?,
                    text: row.get(13)?,
                    speaker: row.get(14)?,
                    confidence: row.get(15)?,
                };
                Ok((recording, segment))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(results)
    }

    /// Get recordings with pending transcription
    pub fn get_pending_recordings(&self) -> Result<Vec<Recording>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, audio_path, duration_secs, state, created_at, updated_at, notes, tags
             FROM recordings
             WHERE state = 'pending'
             ORDER BY created_at ASC",
        )?;

        let recordings = stmt
            .query_map([], |row| Ok(Self::row_to_recording(row)))?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .collect::<Result<Vec<_>>>()?;

        Ok(recordings)
    }

    /// Update recording state
    pub fn update_recording_state(&self, id: &str, state: RecordingState) -> Result<()> {
        self.conn.execute(
            "UPDATE recordings SET state = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, state.as_str(), Utc::now().timestamp()],
        )?;
        Ok(())
    }

    // Helper to convert a row to a Recording
    fn row_to_recording(row: &rusqlite::Row) -> Result<Recording> {
        Ok(Self::row_to_recording_offset(row, 0)?)
    }

    fn row_to_recording_offset(row: &rusqlite::Row, offset: usize) -> rusqlite::Result<Recording> {
        let state_str: String = row.get(offset + 4)?;
        let created_timestamp: i64 = row.get(offset + 5)?;
        let updated_timestamp: i64 = row.get(offset + 6)?;
        let tags_json: String = row.get(offset + 8)?;

        Ok(Recording {
            id: row.get(offset)?,
            title: row.get(offset + 1)?,
            audio_path: row.get(offset + 2)?,
            duration_secs: row.get(offset + 3)?,
            state: state_str.parse().unwrap_or(RecordingState::Pending),
            created_at: Utc.timestamp_opt(created_timestamp, 0).unwrap(),
            updated_at: Utc.timestamp_opt(updated_timestamp, 0).unwrap(),
            notes: row.get(offset + 7)?,
            tags: serde_json::from_str(&tags_json).unwrap_or_default(),
        })
    }

    /// Get database statistics
    pub fn get_stats(&self) -> Result<DatabaseStats> {
        let total_recordings: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM recordings", [], |row| row.get(0))?;

        let total_segments: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM transcript_segments", [], |row| {
                    row.get(0)
                })?;

        let total_duration: Option<i64> = self
            .conn
            .query_row(
                "SELECT SUM(duration_secs) FROM recordings WHERE duration_secs IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .optional()?
            .flatten();

        Ok(DatabaseStats {
            total_recordings: total_recordings as usize,
            total_segments: total_segments as usize,
            total_duration_secs: total_duration.unwrap_or(0) as u64,
        })
    }
}

/// Database statistics
#[derive(Debug, Clone)]
pub struct DatabaseStats {
    pub total_recordings: usize,
    pub total_segments: usize,
    pub total_duration_secs: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::tempdir;

    #[test]
    fn test_create_database() {
        let db = Database::open_memory().unwrap();
        let stats = db.get_stats().unwrap();
        assert_eq!(stats.total_recordings, 0);
    }

    #[test]
    fn test_insert_and_get_recording() {
        let db = Database::open_memory().unwrap();

        let recording = Recording::new("Test Meeting".to_string());
        db.insert_recording(&recording).unwrap();

        let retrieved = db.get_recording(&recording.id).unwrap().unwrap();
        assert_eq!(retrieved.title, "Test Meeting");
    }

    #[test]
    fn test_insert_and_search_segments() {
        let db = Database::open_memory().unwrap();

        let recording = Recording::new("Test Meeting".to_string());
        db.insert_recording(&recording).unwrap();

        let segment = TranscriptSegment::new(
            recording.id.clone(),
            0.0,
            5.0,
            "Hello world this is a test".to_string(),
        );
        db.insert_segment(&segment).unwrap();

        let results = db.search_transcripts("hello", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].1.text.contains("Hello"));
    }

    #[test]
    fn test_new_database_sets_schema_version() {
        let db = Database::open_memory().unwrap();
        assert_eq!(db.schema_version().unwrap(), 1);
    }

    #[test]
    fn test_opening_legacy_database_runs_migration() {
        let tmp = tempdir().unwrap();
        let db_path = tmp.path().join("legacy.db");

        // Simulate a legacy schema without PRAGMA user_version migration tracking.
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE recordings (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                audio_path TEXT,
                duration_secs INTEGER,
                state TEXT NOT NULL DEFAULT 'recording',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                notes TEXT,
                tags TEXT DEFAULT '[]'
            );

            CREATE TABLE transcript_segments (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                recording_id TEXT NOT NULL,
                start_time REAL NOT NULL,
                end_time REAL NOT NULL,
                text TEXT NOT NULL,
                speaker TEXT,
                confidence REAL
            );
            "#,
        )
        .unwrap();
        drop(conn);

        let db = Database::open_path(&db_path).unwrap();
        assert_eq!(db.schema_version().unwrap(), 1);

        let recording = Recording::new("Legacy migration".to_string());
        db.insert_recording(&recording).unwrap();
        let segment = TranscriptSegment::new(
            recording.id.clone(),
            0.0,
            1.0,
            "migration searchable text".to_string(),
        );
        db.insert_segment(&segment).unwrap();

        let results = db.search_transcripts("searchable", 10).unwrap();
        assert_eq!(results.len(), 1);
    }
}
