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
    /// Temporary microphone capture path used when dual capture is active
    mic_path: Option<PathBuf>,
}

const SYSTEM_TARGET_FALLBACK: &str = "@DEFAULT_AUDIO_SINK.monitor";
const MICROPHONE_TARGET_FALLBACK: &str = "@DEFAULT_AUDIO_SOURCE@";
const SYSTEM_ALIAS: &str = "@DEFAULT_AUDIO_SINK@";
const MICROPHONE_ALIAS: &str = "@DEFAULT_AUDIO_SOURCE@";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TargetKind {
    System,
    Microphone,
}

impl TargetKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            TargetKind::System => "system",
            TargetKind::Microphone => "microphone",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TargetResolutionMethod {
    WpctlInspect,
    WpctlStatus,
    FallbackAlias,
}

impl TargetResolutionMethod {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            TargetResolutionMethod::WpctlInspect => "wpctl-inspect",
            TargetResolutionMethod::WpctlStatus => "wpctl-status",
            TargetResolutionMethod::FallbackAlias => "fallback-alias",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedCaptureTarget {
    pub(crate) kind: TargetKind,
    pub(crate) target: String,
    pub(crate) method: TargetResolutionMethod,
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
            let system_target = targets[0].as_str();
            let mic_target = targets[1].as_str();
            let mic_path = output_path.with_extension("mic.wav");

            let system_process =
                spawn_pw_record(system_target, self.sample_rate, self.channels, output_path)?;

            let mic_process = match spawn_pw_record(
                mic_target,
                self.sample_rate,
                self.channels,
                &mic_path,
            ) {
                Ok(process) => process,
                Err(e) => {
                    self.system_process = Some(system_process);
                    self.mic_process = None;
                    self.mic_path = None;
                    tracing::warn!(
                        "PipeWire: microphone capture unavailable, continuing with system audio only: {}",
                        e
                    );
                    return Ok(());
                }
            };

            self.system_process = Some(system_process);
            self.mic_process = Some(mic_process);

            self.mic_path = Some(mic_path);

            tracing::info!(
                "PipeWire: Recording system monitor + microphone via dual pw-record (system_target={}, mic_target={})",
                system_target,
                mic_target
            );
        } else if self.capture_system {
            let system_target = targets[0].as_str();
            self.system_process = Some(spawn_pw_record(
                system_target,
                self.sample_rate,
                self.channels,
                output_path,
            )?);
            tracing::info!(
                "PipeWire: Recording system monitor via pw-record (system_target={})",
                system_target
            );
        } else {
            let mic_target = targets[0].as_str();
            self.mic_process = Some(spawn_pw_record(
                mic_target,
                self.sample_rate,
                self.channels,
                output_path,
            )?);
            tracing::info!(
                "PipeWire: Recording microphone via pw-record (mic_target={})",
                mic_target
            );
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

        if let (Some(output_path), Some(mic_path)) =
            (self.output_path.as_ref(), self.mic_path.take())
        {
            if let Err(e) = maybe_mix_microphone_track(output_path, &mic_path, self.mic_boost) {
                tracing::warn!(
                    "PipeWire: failed to mix microphone track, keeping system-only capture: {}",
                    e
                );
            }

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

fn spawn_pw_record(
    target: &str,
    sample_rate: u32,
    channels: u16,
    output_path: &Path,
) -> Result<Child> {
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

fn mix_wav_files(
    system_path: &Path,
    mic_path: &Path,
    output_path: &Path,
    mic_boost: f32,
) -> Result<()> {
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

fn maybe_mix_microphone_track(output_path: &Path, mic_path: &Path, mic_boost: f32) -> Result<()> {
    if !mic_path.exists() {
        return Ok(());
    }

    // WAV file with header only (no samples) means mic was effectively silent/unavailable.
    let mic_size = std::fs::metadata(mic_path)?.len();
    if mic_size <= 44 {
        return Ok(());
    }

    mix_wav_files(output_path, mic_path, output_path, mic_boost)
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
        (hound::SampleFormat::Float, 32) => reader
            .into_samples::<f32>()
            .filter_map(|s| s.ok())
            .collect(),
        _ => anyhow::bail!(
            "Unsupported WAV format in {}: {:?} {}-bit",
            path.display(),
            spec.sample_format,
            spec.bits_per_sample
        ),
    };

    Ok((spec.sample_rate, spec.channels, samples))
}

fn capture_targets(capture_system: bool, capture_microphone: bool) -> Vec<String> {
    resolve_capture_targets(capture_system, capture_microphone)
        .into_iter()
        .map(|target| target.target)
        .collect()
}

pub(crate) fn resolve_capture_targets(
    capture_system: bool,
    capture_microphone: bool,
) -> Vec<ResolvedCaptureTarget> {
    capture_targets_with_resolver(capture_system, capture_microphone, resolve_target)
}

fn capture_targets_with_resolver<F>(
    capture_system: bool,
    capture_microphone: bool,
    resolver: F,
) -> Vec<ResolvedCaptureTarget>
where
    F: Fn(TargetKind) -> ResolvedCaptureTarget,
{
    let mut targets = Vec::new();
    if capture_system {
        targets.push(resolver(TargetKind::System));
    }
    if capture_microphone {
        targets.push(resolver(TargetKind::Microphone));
    }
    targets
}

fn resolve_target(kind: TargetKind) -> ResolvedCaptureTarget {
    let alias = match kind {
        TargetKind::System => SYSTEM_ALIAS,
        TargetKind::Microphone => MICROPHONE_ALIAS,
    };

    if let Some(id) = resolve_wpctl_node_id(alias) {
        return ResolvedCaptureTarget {
            kind,
            target: id,
            method: TargetResolutionMethod::WpctlInspect,
        };
    }

    if let Some(id) = resolve_wpctl_default_node_id(kind) {
        return ResolvedCaptureTarget {
            kind,
            target: id,
            method: TargetResolutionMethod::WpctlStatus,
        };
    }

    tracing::debug!(
        "PipeWire: failed to resolve {} target id, using alias fallback",
        kind.label()
    );

    ResolvedCaptureTarget {
        kind,
        target: match kind {
            TargetKind::System => SYSTEM_TARGET_FALLBACK.to_string(),
            TargetKind::Microphone => MICROPHONE_TARGET_FALLBACK.to_string(),
        },
        method: TargetResolutionMethod::FallbackAlias,
    }
}

fn resolve_wpctl_node_id(alias: &str) -> Option<String> {
    let output = Command::new("wpctl")
        .args(["inspect", alias])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    parse_wpctl_node_id(&String::from_utf8_lossy(&output.stdout))
}

fn resolve_wpctl_default_node_id(kind: TargetKind) -> Option<String> {
    let output = Command::new("wpctl")
        .args(["status", "-n"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    parse_wpctl_status_default_node_id(&String::from_utf8_lossy(&output.stdout), kind)
}

fn parse_wpctl_node_id(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let trimmed = line.trim();
        let id_part = trimmed.strip_prefix("id ")?.split(',').next()?.trim();
        if id_part.chars().all(|c| c.is_ascii_digit()) {
            Some(id_part.to_string())
        } else {
            None
        }
    })
}

fn parse_wpctl_status_default_node_id(output: &str, kind: TargetKind) -> Option<String> {
    let section_label = match kind {
        TargetKind::System => "Sinks:",
        TargetKind::Microphone => "Sources:",
    };

    let mut in_section = false;
    let mut nodes: Vec<(String, String, bool)> = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.ends_with(section_label) {
            in_section = true;
            continue;
        }

        if !in_section {
            continue;
        }

        if trimmed.ends_with("Filters:") || trimmed.ends_with("Streams:") {
            break;
        }

        let Some((id, name, is_default)) = parse_wpctl_status_node_line(trimmed) else {
            continue;
        };

        nodes.push((id, name, is_default));
    }

    if let Some((id, _, _)) = nodes.iter().find(|(_, _, is_default)| *is_default) {
        return Some(id.clone());
    }

    if let Some(configured_name) = parse_wpctl_configured_default_name(output, kind) {
        if let Some((id, _, _)) = nodes.iter().find(|(_, name, _)| name == &configured_name) {
            return Some(id.clone());
        }
    }

    nodes.first().map(|(id, _, _)| id.clone())
}

fn parse_wpctl_status_node_line(line: &str) -> Option<(String, String, bool)> {
    let is_default = line.contains('*');
    let mut start = None;

    for (idx, ch) in line.char_indices() {
        if ch.is_ascii_digit() {
            start = Some(idx);
            break;
        }
    }

    let start = start?;
    let mut end = start;
    for (idx, ch) in line[start..].char_indices() {
        if !ch.is_ascii_digit() {
            break;
        }
        end = start + idx + ch.len_utf8();
    }

    let id = line[start..end].to_string();
    let after_id = line[end..].trim_start();
    let after_dot = after_id.strip_prefix('.')?.trim_start();
    let name = after_dot.split_whitespace().next().map(str::to_string)?;

    Some((id, name, is_default))
}

fn parse_wpctl_configured_default_name(output: &str, kind: TargetKind) -> Option<String> {
    let key = match kind {
        TargetKind::System => "Audio/Sink",
        TargetKind::Microphone => "Audio/Source",
    };

    output.lines().find_map(|line| {
        let trimmed = line.trim();
        let (_, after_key) = trimmed.split_once(key)?;
        let name = after_key.split_whitespace().next()?;
        Some(name.to_string())
    })
}

impl Drop for PipeWireCapture {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hound::{SampleFormat, WavWriter};
    use tempfile::tempdir;

    #[test]
    fn selects_monitor_and_microphone_targets_when_both_enabled() {
        let targets = capture_targets_with_resolver(true, true, |kind| match kind {
            TargetKind::System => ResolvedCaptureTarget {
                kind,
                target: "61".to_string(),
                method: TargetResolutionMethod::WpctlInspect,
            },
            TargetKind::Microphone => ResolvedCaptureTarget {
                kind,
                target: "62".to_string(),
                method: TargetResolutionMethod::WpctlInspect,
            },
        });
        assert_eq!(targets[0].target, "61");
        assert_eq!(targets[1].target, "62");
    }

    #[test]
    fn falls_back_to_alias_targets_when_resolution_fails() {
        let targets = capture_targets_with_resolver(true, true, |kind| ResolvedCaptureTarget {
            kind,
            target: match kind {
                TargetKind::System => SYSTEM_TARGET_FALLBACK.to_string(),
                TargetKind::Microphone => MICROPHONE_TARGET_FALLBACK.to_string(),
            },
            method: TargetResolutionMethod::FallbackAlias,
        });
        assert_eq!(targets[0].target, "@DEFAULT_AUDIO_SINK.monitor");
        assert_eq!(targets[1].target, "@DEFAULT_AUDIO_SOURCE@");
        assert_eq!(targets[0].method, TargetResolutionMethod::FallbackAlias);
    }

    #[test]
    fn parses_wpctl_node_id() {
        let output = r#"
id 61, type PipeWire:Interface:Node
    node.name = "alsa_output.pci-0000_65_00.6.analog-stereo"
"#;
        assert_eq!(parse_wpctl_node_id(output), Some("61".to_string()));
    }

    #[test]
    fn parses_default_sink_id_from_wpctl_status_output() {
        let status = r#"
Audio
 ├─ Sinks:
 │  *   61. alsa_output.pci-0000_65_00.6.analog-stereo [vol: 0.44]
 │      72. bluez_output.14:06:A7:95:AC:6C [vol: 0.34]
 │
 ├─ Sources:
 │  *   62. alsa_input.pci-0000_65_00.6.analog-stereo [vol: 0.39 MUTED]
"#;

        assert_eq!(
            parse_wpctl_status_default_node_id(status, TargetKind::System),
            Some("61".to_string())
        );
    }

    #[test]
    fn parses_default_source_id_from_wpctl_status_output() {
        let status = r#"
Audio
 ├─ Sinks:
 │  *   61. alsa_output.pci-0000_65_00.6.analog-stereo [vol: 0.44]
 │
 ├─ Sources:
 │      55. monitor-source
 │  *   62. alsa_input.pci-0000_65_00.6.analog-stereo [vol: 0.39 MUTED]
"#;

        assert_eq!(
            parse_wpctl_status_default_node_id(status, TargetKind::Microphone),
            Some("62".to_string())
        );
    }

    #[test]
    fn uses_configured_sink_name_when_status_has_no_default_marker() {
        let status = r#"
Audio
 ├─ Sinks:
 │      10. alsa_output.pci-0000_65_00.6.analog-stereo
 │      20. bluez_output.14:06:A7:95:AC:6C
 │
 ├─ Sources:
 │      30. alsa_input.pci-0000_65_00.6.analog-stereo

Settings
 └─ Default Configured Devices:
         0. Audio/Sink    bluez_output.14:06:A7:95:AC:6C
         1. Audio/Source  alsa_input.pci-0000_65_00.6.analog-stereo
"#;

        assert_eq!(
            parse_wpctl_status_default_node_id(status, TargetKind::System),
            Some("20".to_string())
        );
    }

    #[test]
    fn uses_configured_source_name_when_status_has_no_default_marker() {
        let status = r#"
Audio
 ├─ Sinks:
 │      10. alsa_output.pci-0000_65_00.6.analog-stereo
 │      20. bluez_output.14:06:A7:95:AC:6C
 │
 ├─ Sources:
 │      30. alsa_input.pci-0000_65_00.6.analog-stereo
 │      40. filter_input.echo-cancel

Settings
 └─ Default Configured Devices:
         0. Audio/Sink    bluez_output.14:06:A7:95:AC:6C
         1. Audio/Source  filter_input.echo-cancel
"#;

        assert_eq!(
            parse_wpctl_status_default_node_id(status, TargetKind::Microphone),
            Some("40".to_string())
        );
    }

    #[test]
    fn fails_when_no_capture_sources_enabled() {
        assert!(capture_targets(false, false).is_empty());
    }

    #[test]
    fn keeps_system_capture_when_mic_track_missing() {
        let dir = tempdir().unwrap();
        let system_path = dir.path().join("system.wav");
        let missing_mic_path = dir.path().join("missing.wav");

        write_test_wav(&system_path, &[1000, -1000, 500, -500]);
        maybe_mix_microphone_track(&system_path, &missing_mic_path, 1.2).unwrap();

        let (_, _, samples) = read_wav_as_f32(&system_path).unwrap();
        assert_eq!(samples.len(), 4);
    }

    #[test]
    fn keeps_system_capture_when_mic_track_is_empty() {
        let dir = tempdir().unwrap();
        let system_path = dir.path().join("system.wav");
        let mic_path = dir.path().join("mic.wav");

        write_test_wav(&system_path, &[1000, -1000, 500, -500]);
        write_test_wav(&mic_path, &[]);

        maybe_mix_microphone_track(&system_path, &mic_path, 1.2).unwrap();

        let (_, _, samples) = read_wav_as_f32(&system_path).unwrap();
        assert_eq!(samples.len(), 4);
    }

    fn write_test_wav(path: &Path, samples: &[i16]) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };

        let mut writer = WavWriter::create(path, spec).unwrap();
        for sample in samples {
            writer.write_sample(*sample).unwrap();
        }
        writer.finalize().unwrap();
    }
}
