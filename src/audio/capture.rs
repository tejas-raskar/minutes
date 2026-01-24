//! Audio capture implementation using cpal

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use hound::{WavSpec, WavWriter};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::config::Settings;

/// Audio capture state
pub struct AudioCapture {
    /// WAV writer
    writer: Arc<Mutex<Option<WavWriter<std::io::BufWriter<std::fs::File>>>>>,

    /// Audio stream
    stream: Option<Stream>,

    /// Whether recording is active
    recording: Arc<AtomicBool>,

    /// Sample rate
    sample_rate: u32,

    /// Number of channels
    channels: u16,
}

impl AudioCapture {
    /// Create a new audio capture instance
    pub fn new(settings: &Settings, output_path: &Path) -> Result<Self> {
        let sample_rate = settings.audio.sample_rate;
        let channels = settings.audio.channels;

        // Ensure output directory exists
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Create WAV writer
        let spec = WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let writer = WavWriter::create(output_path, spec)
            .with_context(|| format!("Failed to create WAV file: {}", output_path.display()))?;

        Ok(Self {
            writer: Arc::new(Mutex::new(Some(writer))),
            stream: None,
            recording: Arc::new(AtomicBool::new(false)),
            sample_rate,
            channels,
        })
    }

    /// Start recording
    pub fn start(&mut self) -> Result<()> {
        let host = cpal::default_host();

        // Get default input device
        let device = host
            .default_input_device()
            .context("No input device available")?;

        tracing::info!("Using audio device: {}", device.name().unwrap_or_default());

        // Get supported config
        let supported_configs = device
            .supported_input_configs()
            .context("Failed to get supported configs")?;

        // Find a suitable config
        let config = find_suitable_config(supported_configs, self.sample_rate, self.channels)?;

        tracing::info!(
            "Audio config: {} Hz, {} channels, {:?}",
            config.sample_rate().0,
            config.channels(),
            config.sample_format()
        );

        let stream_config = StreamConfig {
            channels: config.channels(),
            sample_rate: config.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };

        // Update actual values
        self.sample_rate = config.sample_rate().0;
        self.channels = config.channels();

        // Set up recording state
        self.recording.store(true, Ordering::SeqCst);

        let writer = self.writer.clone();
        let recording = self.recording.clone();

        // Create stream based on sample format
        let stream = match config.sample_format() {
            SampleFormat::I8 => build_stream::<i8>(&device, &stream_config, writer, recording)?,
            SampleFormat::I16 => build_stream::<i16>(&device, &stream_config, writer, recording)?,
            SampleFormat::I32 => build_stream::<i32>(&device, &stream_config, writer, recording)?,
            SampleFormat::I64 => build_stream::<i64>(&device, &stream_config, writer, recording)?,
            SampleFormat::U8 => build_stream::<u8>(&device, &stream_config, writer, recording)?,
            SampleFormat::U16 => build_stream::<u16>(&device, &stream_config, writer, recording)?,
            SampleFormat::U32 => build_stream::<u32>(&device, &stream_config, writer, recording)?,
            SampleFormat::U64 => build_stream::<u64>(&device, &stream_config, writer, recording)?,
            SampleFormat::F32 => build_stream::<f32>(&device, &stream_config, writer, recording)?,
            SampleFormat::F64 => build_stream::<f64>(&device, &stream_config, writer, recording)?,
            format => anyhow::bail!("Unsupported sample format: {:?}", format),
        };

        stream.play().context("Failed to start audio stream")?;
        self.stream = Some(stream);

        tracing::info!("Audio recording started");
        Ok(())
    }

    /// Stop recording
    pub fn stop(&mut self) -> Result<()> {
        self.recording.store(false, Ordering::SeqCst);

        // Drop the stream to stop recording
        self.stream.take();

        // Finalize the WAV file
        if let Ok(mut guard) = self.writer.lock() {
            if let Some(writer) = guard.take() {
                writer.finalize().context("Failed to finalize WAV file")?;
            }
        }

        tracing::info!("Audio recording stopped");
        Ok(())
    }

    /// Check if recording is active
    pub fn is_recording(&self) -> bool {
        self.recording.load(Ordering::SeqCst)
    }
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Find a suitable audio configuration
fn find_suitable_config(
    configs: cpal::SupportedInputConfigs,
    target_sample_rate: u32,
    target_channels: u16,
) -> Result<cpal::SupportedStreamConfig> {
    let configs: Vec<_> = configs.collect();

    // Try to find exact match first
    for config in &configs {
        if config.channels() == target_channels
            && config.min_sample_rate().0 <= target_sample_rate
            && config.max_sample_rate().0 >= target_sample_rate
        {
            return Ok(config.clone().with_sample_rate(cpal::SampleRate(target_sample_rate)));
        }
    }

    // Fall back to any config that supports the sample rate
    for config in &configs {
        if config.min_sample_rate().0 <= target_sample_rate
            && config.max_sample_rate().0 >= target_sample_rate
        {
            return Ok(config.clone().with_sample_rate(cpal::SampleRate(target_sample_rate)));
        }
    }

    // Just use the first available config
    configs
        .into_iter()
        .next()
        .map(|c| c.with_max_sample_rate())
        .context("No supported audio configuration found")
}

/// Build an audio stream for a specific sample format
fn build_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    writer: Arc<Mutex<Option<WavWriter<std::io::BufWriter<std::fs::File>>>>>,
    recording: Arc<AtomicBool>,
) -> Result<Stream>
where
    T: cpal::Sample + cpal::SizedSample + 'static,
    i16: cpal::FromSample<T>,
{
    let err_fn = |err| tracing::error!("Audio stream error: {}", err);

    let stream = device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            if !recording.load(Ordering::SeqCst) {
                return;
            }

            if let Ok(mut guard) = writer.lock() {
                if let Some(ref mut writer) = *guard {
                    for &sample in data {
                        let sample_i16: i16 = cpal::Sample::from_sample(sample);
                        if writer.write_sample(sample_i16).is_err() {
                            break;
                        }
                    }
                }
            }
        },
        err_fn,
        None,
    )?;

    Ok(stream)
}
