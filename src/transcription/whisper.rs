//! Whisper transcription using whisper-rs

use anyhow::{Context, Result};
use std::path::Path;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::config::Settings;
use crate::storage::TranscriptSegment;

/// Whisper-based transcriber
pub struct WhisperTranscriber {
    ctx: WhisperContext,
    language: Option<String>,
    translate: bool,
}

impl WhisperTranscriber {
    /// Create a new transcriber with the specified model
    pub fn new(settings: &Settings) -> Result<Self> {
        let model_path = settings.model_path();

        if !model_path.exists() {
            anyhow::bail!(
                "Whisper model not found at {}. Please download the model first.\n\
                 Run: minutes model download {}",
                model_path.display(),
                settings.whisper.model
            );
        }

        let ctx = WhisperContext::new_with_params(
            model_path.to_str().unwrap(),
            WhisperContextParameters::default(),
        )
        .context("Failed to load Whisper model")?;

        let language = if settings.whisper.language.is_empty() {
            None
        } else {
            Some(settings.whisper.language.clone())
        };

        Ok(Self {
            ctx,
            language,
            translate: settings.whisper.translate,
        })
    }

    /// Transcribe audio samples
    pub fn transcribe(
        &self,
        samples: &[f32],
        recording_id: &str,
    ) -> Result<Vec<TranscriptSegment>> {
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        // Configure parameters
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_translate(self.translate);

        if let Some(ref lang) = self.language {
            params.set_language(Some(lang));
        }

        // Run inference
        let mut state = self.ctx.create_state().context("Failed to create Whisper state")?;
        state
            .full(params, samples)
            .context("Whisper inference failed")?;

        // Extract segments
        let num_segments = state.full_n_segments().context("Failed to get segment count")?;
        let mut segments = Vec::new();

        for i in 0..num_segments {
            let start_time = state
                .full_get_segment_t0(i)
                .context("Failed to get segment start time")? as f64
                / 100.0; // Convert from centiseconds

            let end_time = state
                .full_get_segment_t1(i)
                .context("Failed to get segment end time")? as f64
                / 100.0;

            let text = state
                .full_get_segment_text(i)
                .context("Failed to get segment text")?;

            // Skip empty or whitespace-only segments
            let text = text.trim().to_string();
            if text.is_empty() {
                continue;
            }

            segments.push(TranscriptSegment::new(
                recording_id.to_string(),
                start_time,
                end_time,
                text,
            ));
        }

        Ok(segments)
    }
}

/// Load audio from a WAV file and convert to f32 samples at 16kHz mono
pub fn load_audio(path: &Path) -> Result<Vec<f32>> {
    let reader = hound::WavReader::open(path)
        .with_context(|| format!("Failed to open audio file: {}", path.display()))?;

    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let channels = spec.channels as usize;

    tracing::debug!(
        "Loading audio: {} Hz, {} channels, {:?}",
        sample_rate,
        channels,
        spec.sample_format
    );

    // Read samples based on format
    let samples: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Int, 16) => {
            reader
                .into_samples::<i16>()
                .filter_map(|s| s.ok())
                .map(|s| s as f32 / 32768.0)
                .collect()
        }
        (hound::SampleFormat::Int, 32) => {
            reader
                .into_samples::<i32>()
                .filter_map(|s| s.ok())
                .map(|s| s as f32 / 2147483648.0)
                .collect()
        }
        (hound::SampleFormat::Float, 32) => {
            reader.into_samples::<f32>().filter_map(|s| s.ok()).collect()
        }
        _ => anyhow::bail!(
            "Unsupported audio format: {:?} {}bit",
            spec.sample_format,
            spec.bits_per_sample
        ),
    };

    // Convert to mono if stereo
    let samples = if channels > 1 {
        samples
            .chunks(channels)
            .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
            .collect()
    } else {
        samples
    };

    // Resample to 16kHz if needed
    let samples = if sample_rate != 16000 {
        resample(&samples, sample_rate, 16000)
    } else {
        samples
    };

    Ok(samples)
}

/// Simple linear resampling
fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    let ratio = from_rate as f64 / to_rate as f64;
    let new_len = (samples.len() as f64 / ratio) as usize;
    let mut result = Vec::with_capacity(new_len);

    for i in 0..new_len {
        let src_pos = i as f64 * ratio;
        let src_idx = src_pos as usize;
        let frac = src_pos - src_idx as f64;

        let sample = if src_idx + 1 < samples.len() {
            samples[src_idx] * (1.0 - frac as f32) + samples[src_idx + 1] * frac as f32
        } else if src_idx < samples.len() {
            samples[src_idx]
        } else {
            0.0
        };

        result.push(sample);
    }

    result
}
