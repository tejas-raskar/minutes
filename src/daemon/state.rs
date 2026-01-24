//! Recording state machine for the daemon

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

use crate::daemon::ipc::RecordingStatus;
use crate::storage::Recording;

/// Current state of the daemon
#[derive(Debug)]
pub enum DaemonState {
    /// Idle, waiting for commands
    Idle,

    /// Actively recording
    Recording(ActiveRecording),

    /// Transcribing a recording
    Transcribing(TranscriptionState),
}

/// State of an active recording
#[derive(Debug)]
pub struct ActiveRecording {
    /// Database recording
    pub recording: Recording,

    /// Path to the audio file being written
    pub audio_path: PathBuf,

    /// When recording started
    pub started_at: Instant,

    /// Current audio level (0.0 - 1.0)
    pub audio_level: f32,
}

/// State of an active transcription
#[derive(Debug)]
pub struct TranscriptionState {
    /// Recording ID
    pub recording_id: String,

    /// Progress (0.0 - 1.0)
    pub progress: f32,
}

impl DaemonState {
    /// Get the recording status for IPC
    pub fn to_status(&self) -> RecordingStatus {
        match self {
            DaemonState::Idle => RecordingStatus::Idle,
            DaemonState::Recording(active) => {
                let duration = active.started_at.elapsed().as_secs();
                RecordingStatus::Recording {
                    id: active.recording.id.clone(),
                    title: active.recording.title.clone(),
                    duration_secs: duration,
                    audio_level: active.audio_level,
                }
            }
            DaemonState::Transcribing(state) => RecordingStatus::Transcribing {
                id: state.recording_id.clone(),
                progress: state.progress,
            },
        }
    }
}

/// Thread-safe state container
pub type SharedState = Arc<RwLock<DaemonState>>;

/// Create a new shared state
pub fn new_shared_state() -> SharedState {
    Arc::new(RwLock::new(DaemonState::Idle))
}
