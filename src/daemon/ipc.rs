//! IPC protocol definitions for daemon communication

use serde::{Deserialize, Serialize};

/// Request sent from CLI/TUI to daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonRequest {
    /// Start a new recording
    StartRecording { title: String },

    /// Stop the current recording
    StopRecording,

    /// Get current status
    GetStatus,

    /// Ping to check if daemon is alive
    Ping,

    /// Shutdown the daemon
    Shutdown,

    /// Force transcription of a recording
    Transcribe { recording_id: String },
}

/// Response sent from daemon to CLI/TUI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonResponse {
    /// Recording started successfully
    RecordingStarted { id: String },

    /// Recording stopped successfully
    RecordingStopped { id: String, duration_secs: u64 },

    /// Current status
    Status(RecordingStatus),

    /// Pong response to ping
    Pong,

    /// Acknowledgment (for shutdown, etc.)
    Ok,

    /// Error response
    Error { message: String },
}

/// Current recording status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecordingStatus {
    /// No active recording
    Idle,

    /// Recording in progress
    Recording {
        id: String,
        title: String,
        duration_secs: u64,
        audio_level: f32,
    },

    /// Transcription in progress
    Transcribing { id: String, progress: f32 },
}

/// Serialize a request to bytes for IPC
pub fn serialize_request(request: &DaemonRequest) -> Vec<u8> {
    let json = serde_json::to_string(request).expect("Failed to serialize request");
    let len = json.len() as u32;
    let mut bytes = len.to_le_bytes().to_vec();
    bytes.extend(json.as_bytes());
    bytes
}

/// Serialize a response to bytes for IPC
pub fn serialize_response(response: &DaemonResponse) -> Vec<u8> {
    let json = serde_json::to_string(response).expect("Failed to serialize response");
    let len = json.len() as u32;
    let mut bytes = len.to_le_bytes().to_vec();
    bytes.extend(json.as_bytes());
    bytes
}

/// Deserialize a request from bytes
pub fn deserialize_request(data: &[u8]) -> Result<DaemonRequest, String> {
    serde_json::from_slice(data).map_err(|e| e.to_string())
}

/// Deserialize a response from bytes
pub fn deserialize_response(data: &[u8]) -> Result<DaemonResponse, String> {
    serde_json::from_slice(data).map_err(|e| e.to_string())
}
