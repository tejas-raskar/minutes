//! Data models for storage

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// State of a recording
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecordingState {
    /// Recording is in progress
    Recording,
    /// Recording stopped, waiting for transcription
    Pending,
    /// Transcription in progress
    Transcribing,
    /// Transcription complete
    Completed,
    /// Transcription failed
    Failed,
}

impl RecordingState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Recording => "recording",
            Self::Pending => "pending",
            Self::Transcribing => "transcribing",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "recording" => Some(Self::Recording),
            "pending" => Some(Self::Pending),
            "transcribing" => Some(Self::Transcribing),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

/// A meeting recording
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recording {
    /// Unique identifier (UUID)
    pub id: String,

    /// User-provided or auto-generated title
    pub title: String,

    /// Path to the audio file
    pub audio_path: Option<String>,

    /// Duration in seconds
    pub duration_secs: Option<u64>,

    /// Current state
    pub state: RecordingState,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,

    /// Optional notes or summary
    pub notes: Option<String>,

    /// Tags for categorization
    pub tags: Vec<String>,
}

impl Recording {
    /// Create a new recording with the given title
    pub fn new(title: String) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            audio_path: None,
            duration_secs: None,
            state: RecordingState::Recording,
            created_at: now,
            updated_at: now,
            notes: None,
            tags: Vec::new(),
        }
    }
}

/// A segment of transcribed text
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    /// Unique identifier
    pub id: i64,

    /// Recording this segment belongs to
    pub recording_id: String,

    /// Start time in seconds from beginning of recording
    pub start_time: f64,

    /// End time in seconds
    pub end_time: f64,

    /// Transcribed text
    pub text: String,

    /// Speaker identifier (for diarization, post-MVP)
    pub speaker: Option<String>,

    /// Confidence score (0.0 - 1.0)
    pub confidence: Option<f64>,
}

impl TranscriptSegment {
    /// Create a new transcript segment
    pub fn new(
        recording_id: String,
        start_time: f64,
        end_time: f64,
        text: String,
    ) -> Self {
        Self {
            id: 0, // Will be set by database
            recording_id,
            start_time,
            end_time,
            text,
            speaker: None,
            confidence: None,
        }
    }
}

/// Search result with context
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SearchResult {
    pub recording: Recording,
    pub segment: TranscriptSegment,
    pub rank: f64,
}
