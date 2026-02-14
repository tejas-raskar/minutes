//! PipeWire audio capture implementation
//!
//! Captures both system audio (monitor) and microphone, mixing them
//! into a single stream for meeting recording.
//!
//! Note: This is a simplified implementation that uses PipeWire's
//! pw-record utility under the hood for reliability.

use anyhow::{Context, Result};
use hound::{WavReader, WavSpec, WavWriter};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::config::Settings;

use super::{AudioCapture, AudioMixer};

/// PipeWire audio capture
///
/// Uses pw-record to capture audio from the default audio sink monitor.
/// This captures system audio (what you hear from speakers/headphones).
pub struct PipeWireCapture {
    /// Sample rate
    sample_rate: u32,
    /// Number of channels (always 1 - mono output)
    channels: u16,
    /// Whether to capture system output monitor
    capture_system: bool,
    /// Whether to capture microphone input
    capture_microphone: bool,
    /// Microphone boost applied during software mixing
    mic_boost: f32,
    /// Whether recording is active
    recording: Arc<AtomicBool>,
    /// pw-record process handle for system monitor capture
    system_process: Option<Child>,
    /// pw-record process handle for microphone capture
    mic_process: Option<Child>,
    /// Current output path
    output_path: Option<PathBuf>,
    /// Temporary system capture path used when dual capture is active
    system_path: Option<PathBuf>,
    /// Temporary microphone capture path used when dual capture is active
    mic_path: Option<PathBuf>,
}

impl PipeWireCapture {
    /// Create a new PipeWire capture instance
    pub fn new(settings: &Settings) -> Result<Self> {
        // Verify pw-record is available
        let status = Command::new("pw-record")
            .arg("--help")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        if status.is_err() {
            anyhow::bail!("pw-record not found. Please install pipewire-tools.");
        }

        Ok(Self {
            sample_rate: settings.audio.sample_rate,
            channels: 1, // Always mono for Whisper compatibility
            capture_system: settings.audio.capture_system,
            capture_microphone: settings.audio.capture_microphone,
            mic_boost: settings.audio.mic_boost,
            recording: Arc::new(AtomicBool::new(false)),
            system_process: None,
            mic_process: None,
            output_path: None,
            system_path: None,
            mic_path: None,
        })
    }

    /// Check if PipeWire is available on this system
    pub fn is_available() -> bool {
        Command::new("pw-record")
            .arg("--help")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok()
    }
}

impl AudioCapture for PipeWireCapture {
    fn start(&mut self, output_path: &Path) -> Result<()> {
        let targets = capture_targets(self.capture_system, self.capture_microphone);
        if targets.is_empty() {
            anyhow::bail!("No audio sources enabled. Enable system and/or microphone capture.");
        }

        // Ensure output directory exists
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        self.output_path = Some(output_path.to_path_buf());
        self.recording.store(true, Ordering::SeqCst);

        if targets.len() == 2 {
            let system_path = output_path.with_extension("system.wav");
            let mic_path = output_path.with_extension("mic.wav");

            let system_process = spawn_pw_record(
                "@DEFAULT_AUDIO_SINK.monitor",
                self.sample_rate,
                self.channels,
                &system_path,
            )?;

            let mic_process = match spawn_pw_record(
                "@DEFAULT_AUDIO_SOURCE@",
                self.sample_rate,
                self.channels,
                &mic_path,
            ) {
                Ok(process) => process,
                Err(e) => {
                    wait_for_process(system_process);
                    let _ = std::fs::remove_file(&system_path);
                    return Err(e);
                }
            };

            self.system_process = Some(system_process);
            self.mic_process = Some(mic_process);

            self.system_path = Some(system_path);
            self.mic_path = Some(mic_path);

            tracing::info!(
                "PipeWire: Recording system monitor + microphone via dual pw-record"
            );
        } else if targets[0] == "@DEFAULT_AUDIO_SINK.monitor" {
            self.system_process = Some(spawn_pw_record(
                targets[0],
                self.sample_rate,
                self.channels,
                output_path,
            )?);
            tracing::info!("PipeWire: Recording system monitor via pw-record");
        } else {
            self.mic_process = Some(spawn_pw_record(
                targets[0],
                self.sample_rate,
                self.channels,
                output_path,
            )?);
            tracing::info!("PipeWire: Recording microphone via pw-record");
        }

        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.recording.store(false, Ordering::SeqCst);

        if let Some(child) = self.system_process.take() {
            wait_for_process(child);
        }

        if let Some(child) = self.mic_process.take() {
            wait_for_process(child);
        }

        if let (Some(output_path), Some(system_path), Some(mic_path)) = (
            self.output_path.as_ref(),
            self.system_path.take(),
            self.mic_path.take(),
        ) {
            mix_wav_files(&system_path, &mic_path, output_path, self.mic_boost)?;
            let _ = std::fs::remove_file(&system_path);
            let _ = std::fs::remove_file(&mic_path);
        }

        tracing::info!("PipeWire: Recording stopped");
        Ok(())
    }

    fn is_recording(&self) -> bool {
        self.recording.load(Ordering::SeqCst)
    }

    fn backend_name(&self) -> &'static str {
        "pipewire"
    }
}

fn spawn_pw_record(target: &str, sample_rate: u32, channels: u16, output_path: &Path) -> Result<Child> {
    Command::new("pw-record")
        .args([
            "--target",
            target,
            "--rate",
            &sample_rate.to_string(),
            "--channels",
            &channels.to_string(),
            "--format",
            "s16",
            output_path.to_str().unwrap(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to start pw-record for target {}", target))
}

fn wait_for_process(mut child: Child) {
    #[cfg(unix)]
    unsafe {
        libc::kill(child.id() as i32, libc::SIGTERM);
    }

    match child.wait() {
        Ok(status) => {
            if !status.success() {
                tracing::warn!("pw-record exited with status: {}", status);
            }
        }
        Err(e) => {
            tracing::warn!("Failed to wait for pw-record: {}", e);
        }
    }
}

fn mix_wav_files(system_path: &Path, mic_path: &Path, output_path: &Path, mic_boost: f32) -> Result<()> {
    let (system_rate, system_channels, mut system_samples) = read_wav_as_f32(system_path)?;
    let (mic_rate, mic_channels, mut mic_samples) = read_wav_as_f32(mic_path)?;

    if system_channels > 1 {
        system_samples = AudioMixer::stereo_to_mono(&system_samples);
    }

    if mic_channels > 1 {
        mic_samples = AudioMixer::stereo_to_mono(&mic_samples);
    }

    let mixer = AudioMixer::new(system_rate, mic_boost);
    if mic_rate != system_rate {
        mic_samples = mixer.resample(&mic_samples, mic_rate);
    }

    let mixed = mixer.mix_to_i16(&system_samples, &mic_samples);

    let spec = WavSpec {
        channels: 1,
        sample_rate: system_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = WavWriter::create(output_path, spec)
        .with_context(|| format!("Failed to create mixed WAV file: {}", output_path.display()))?;

    for sample in mixed {
        writer.write_sample(sample)?;
    }
    writer.finalize()?;

    Ok(())
}

fn read_wav_as_f32(path: &Path) -> Result<(u32, u16, Vec<f32>)> {
    let reader = WavReader::open(path)
        .with_context(|| format!("Failed to open WAV file: {}", path.display()))?;
    let spec = reader.spec();

    let samples = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Int, 16) => reader
            .into_samples::<i16>()
            .filter_map(|s| s.ok())
            .map(|s| s as f32 / 32768.0)
            .collect(),
        (hound::SampleFormat::Int, 32) => reader
            .into_samples::<i32>()
            .filter_map(|s| s.ok())
            .map(|s| s as f32 / 2147483648.0)
            .collect(),
        (hound::SampleFormat::Float, 32) => {
            reader.into_samples::<f32>().filter_map(|s| s.ok()).collect()
        }
        _ => anyhow::bail!(
            "Unsupported WAV format in {}: {:?} {}-bit",
            path.display(),
            spec.sample_format,
            spec.bits_per_sample
        ),
    };

    Ok((spec.sample_rate, spec.channels, samples))
}

fn capture_targets(capture_system: bool, capture_microphone: bool) -> Vec<&'static str> {
    let mut targets = Vec::new();
    if capture_system {
        targets.push("@DEFAULT_AUDIO_SINK.monitor");
    }
    if capture_microphone {
        targets.push("@DEFAULT_AUDIO_SOURCE@");
    }
    targets
}

impl Drop for PipeWireCapture {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_monitor_and_microphone_targets_when_both_enabled() {
        let targets = capture_targets(true, true);
        assert_eq!(targets, vec!["@DEFAULT_AUDIO_SINK.monitor", "@DEFAULT_AUDIO_SOURCE@"]);
    }

    #[test]
    fn fails_when_no_capture_sources_enabled() {
        assert!(capture_targets(false, false).is_empty());
    }
}
