//! Transcription module for minutes
//!
//! Handles speech-to-text using whisper-rs.

mod pipeline;
mod whisper;

pub use pipeline::TranscriptionPipeline;
pub use whisper::WhisperTranscriber;
