//! Repository pattern wrapper for database operations
//!
//! Provides a higher-level interface for common database operations.

use anyhow::Result;

use crate::config::Settings;
use crate::storage::{Database, Recording, RecordingState, TranscriptSegment};

/// Repository for managing recordings and transcripts
pub struct Repository {
    db: Database,
}

impl Repository {
    /// Create a new repository
    pub fn new(settings: &Settings) -> Result<Self> {
        let db = Database::open(settings)?;
        Ok(Self { db })
    }

    /// Create a new recording
    pub fn create_recording(&self, title: String, audio_path: String) -> Result<Recording> {
        let mut recording = Recording::new(title);
        recording.audio_path = Some(audio_path);
        self.db.insert_recording(&recording)?;
        Ok(recording)
    }

    /// Mark a recording as completed with duration
    pub fn complete_recording(&self, id: &str, duration_secs: u64) -> Result<()> {
        if let Some(mut recording) = self.db.get_recording(id)? {
            recording.duration_secs = Some(duration_secs);
            recording.state = RecordingState::Pending;
            self.db.update_recording(&recording)?;
        }
        Ok(())
    }

    /// Start transcription for a recording
    pub fn start_transcription(&self, id: &str) -> Result<()> {
        self.db.update_recording_state(id, RecordingState::Transcribing)
    }

    /// Complete transcription for a recording
    pub fn complete_transcription(
        &self,
        id: &str,
        segments: &[TranscriptSegment],
    ) -> Result<()> {
        self.db.insert_segments(segments)?;
        self.db.update_recording_state(id, RecordingState::Completed)
    }

    /// Mark transcription as failed
    pub fn fail_transcription(&self, id: &str) -> Result<()> {
        self.db.update_recording_state(id, RecordingState::Failed)
    }

    /// Get a recording by ID
    pub fn get_recording(&self, id: &str) -> Result<Option<Recording>> {
        self.db.get_recording(id)
    }

    /// Find recording by ID prefix
    pub fn find_recording(&self, prefix: &str) -> Result<Option<Recording>> {
        self.db.find_recording_by_prefix(prefix)
    }

    /// List recent recordings
    pub fn list_recent(&self, limit: usize) -> Result<Vec<Recording>> {
        self.db.list_recordings(limit)
    }

    /// Get recordings pending transcription
    pub fn get_pending(&self) -> Result<Vec<Recording>> {
        self.db.get_pending_recordings()
    }

    /// Get transcript for a recording
    pub fn get_transcript(&self, recording_id: &str) -> Result<Vec<TranscriptSegment>> {
        self.db.get_transcript_segments(recording_id)
    }

    /// Search transcripts
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<(Recording, TranscriptSegment)>> {
        self.db.search_transcripts(query, limit)
    }

    /// Delete a recording
    pub fn delete(&self, id: &str) -> Result<()> {
        self.db.delete_recording(id)
    }
}
