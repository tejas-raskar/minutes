//! Transcription pipeline orchestration

use anyhow::Result;
use std::path::Path;

use crate::config::Settings;
use crate::storage::TranscriptSegment;
use crate::transcription::whisper::{load_audio, WhisperTranscriber};

/// Progress callback type
pub type ProgressCallback = Box<dyn Fn(f32) + Send + Sync>;

/// Transcription pipeline for processing audio files
pub struct TranscriptionPipeline {
    transcriber: WhisperTranscriber,
    chunk_duration_secs: f32,
}

impl TranscriptionPipeline {
    /// Create a new transcription pipeline
    pub fn new(settings: &Settings) -> Result<Self> {
        let transcriber = WhisperTranscriber::new(settings)?;

        Ok(Self {
            transcriber,
            chunk_duration_secs: 30.0, // Process in 30-second chunks
        })
    }

    /// Transcribe an audio file
    pub async fn transcribe(
        &self,
        audio_path: &str,
        recording_id: &str,
        progress_callback: ProgressCallback,
    ) -> Result<Vec<TranscriptSegment>> {
        let path = Path::new(audio_path);

        // Load audio
        tracing::info!("Loading audio from: {}", audio_path);
        let samples = load_audio(path)?;

        let sample_rate = 16000; // Whisper expects 16kHz
        let chunk_samples = (self.chunk_duration_secs * sample_rate as f32) as usize;

        let mut all_segments = Vec::new();
        let mut offset_time = 0.0;

        // Process in chunks
        let chunks: Vec<_> = samples.chunks(chunk_samples).collect();
        let total_chunks = chunks.len();

        for (i, chunk) in chunks.iter().enumerate() {
            tracing::debug!("Processing chunk {}/{}", i + 1, total_chunks);

            // Report progress
            let progress = (i as f32 + 0.5) / total_chunks as f32;
            progress_callback(progress);

            // Transcribe chunk
            let mut segments = self.transcriber.transcribe(chunk, recording_id)?;

            // Adjust timestamps for chunk offset
            for segment in &mut segments {
                segment.start_time += offset_time;
                segment.end_time += offset_time;
            }

            all_segments.extend(segments);

            // Update offset for next chunk
            offset_time += chunk.len() as f64 / sample_rate as f64;
        }

        // Final progress update
        progress_callback(1.0);

        // Merge adjacent segments if they're continuous
        let merged_segments = merge_segments(all_segments);

        tracing::info!(
            "Transcription complete: {} segments",
            merged_segments.len()
        );

        Ok(merged_segments)
    }
}

/// Merge adjacent segments with small gaps
fn merge_segments(segments: Vec<TranscriptSegment>) -> Vec<TranscriptSegment> {
    if segments.is_empty() {
        return segments;
    }

    let mut iter = segments.into_iter();
    let mut merged = Vec::new();
    let mut current = iter.next().unwrap();

    for segment in iter {
        // If segments are close together (within 0.5s) and from same speaker, merge
        let gap = segment.start_time - current.end_time;

        if gap < 0.5 && current.speaker == segment.speaker {
            current.end_time = segment.end_time;
            current.text.push(' ');
            current.text.push_str(&segment.text);
        } else {
            merged.push(current);
            current = segment;
        }
    }

    merged.push(current);
    merged
}
