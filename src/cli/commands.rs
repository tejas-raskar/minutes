//! CLI command implementations

use anyhow::{Context, Result};
use chrono::Local;
use serde::Serialize;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::audio::AudioBackend;
use crate::cli::args::{ConfigCommand, DaemonCommand};
use crate::config::Settings;
use crate::daemon::client::DaemonClient;
use crate::daemon::ipc::{DaemonRequest, DaemonResponse, RecordingStatus};
use crate::llm::{build_provider, SummaryRequest};
use crate::storage::{Database, Recording};

/// Start a new recording
pub async fn start_recording(settings: &Settings, title: Option<String>) -> Result<()> {
    let mut client = DaemonClient::connect(settings).await?;

    let title =
        title.unwrap_or_else(|| format!("Meeting {}", Local::now().format("%Y-%m-%d %H:%M")));

    let response = client
        .send(DaemonRequest::StartRecording {
            title: title.clone(),
        })
        .await?;

    match response {
        DaemonResponse::RecordingStarted { id } => {
            println!("Recording started: {} ({})", title, &id[..8]);
        }
        DaemonResponse::Error { message } => {
            anyhow::bail!("Failed to start recording: {}", message);
        }
        _ => {
            anyhow::bail!("Unexpected response from daemon");
        }
    }

    Ok(())
}

/// Stop the current recording
pub async fn stop_recording(settings: &Settings) -> Result<()> {
    let mut client = DaemonClient::connect(settings).await?;

    let response = client.send(DaemonRequest::StopRecording).await?;

    match response {
        DaemonResponse::RecordingStopped { id, duration_secs } => {
            let minutes = duration_secs / 60;
            let seconds = duration_secs % 60;
            println!(
                "Recording stopped: {} (duration: {}:{:02})",
                &id[..8],
                minutes,
                seconds
            );
            println!("Transcription queued...");
        }
        DaemonResponse::Error { message } => {
            anyhow::bail!("Failed to stop recording: {}", message);
        }
        _ => {
            anyhow::bail!("Unexpected response from daemon");
        }
    }

    Ok(())
}

/// Show current recording status
pub async fn show_status(settings: &Settings) -> Result<()> {
    let mut client = match DaemonClient::connect(settings).await {
        Ok(c) => c,
        Err(_) => {
            println!("Daemon is not running");
            return Ok(());
        }
    };

    let response = client.send(DaemonRequest::GetStatus).await?;

    match response {
        DaemonResponse::Status(status) => match status {
            RecordingStatus::Idle => {
                println!("Status: Idle (not recording)");
            }
            RecordingStatus::Recording {
                id,
                title,
                duration_secs,
                ..
            } => {
                let minutes = duration_secs / 60;
                let seconds = duration_secs % 60;
                println!("Status: Recording");
                println!("  Title: {}", title);
                println!("  ID: {}", &id[..8]);
                println!("  Duration: {}:{:02}", minutes, seconds);
            }
            RecordingStatus::Transcribing { id, progress } => {
                println!("Status: Transcribing");
                println!("  ID: {}", &id[..8]);
                println!("  Progress: {:.0}%", progress * 100.0);
            }
        },
        DaemonResponse::Error { message } => {
            anyhow::bail!("Failed to get status: {}", message);
        }
        _ => {
            anyhow::bail!("Unexpected response from daemon");
        }
    }

    Ok(())
}

/// List recorded meetings
pub async fn list_recordings(
    settings: &Settings,
    limit: usize,
    search: Option<String>,
) -> Result<()> {
    let db = Database::open(settings)?;

    let recordings = if let Some(query) = search {
        db.search_recordings(&query, limit)?
    } else {
        db.list_recordings(limit)?
    };

    if recordings.is_empty() {
        println!("No recordings found");
        return Ok(());
    }

    println!(
        "{:<10} {:<30} {:<12} {:<10}",
        "ID", "Title", "Date", "Duration"
    );
    println!("{}", "-".repeat(65));

    for recording in recordings {
        let duration = format_duration(recording.duration_secs.unwrap_or(0));
        let date = recording.created_at.format("%Y-%m-%d");
        println!(
            "{:<10} {:<30} {:<12} {:<10}",
            &recording.id[..8],
            truncate(&recording.title, 28),
            date,
            duration
        );
    }

    Ok(())
}

/// View a specific recording's transcript
pub async fn view_recording(settings: &Settings, id: &str) -> Result<()> {
    let db = Database::open(settings)?;

    let recording = db
        .find_recording_by_prefix(id)?
        .context("Recording not found")?;

    println!("Title: {}", recording.title);
    println!("Date: {}", recording.created_at.format("%Y-%m-%d %H:%M"));
    if let Some(duration) = recording.duration_secs {
        println!("Duration: {}", format_duration(duration));
    }

    if let Some(summary) = recording.notes.as_deref() {
        println!();
        println!("Summary:");
        println!("{}", summary);
    }
    println!();

    let segments = db.get_transcript_segments(&recording.id)?;

    if segments.is_empty() {
        println!("(No transcript available yet)");
        return Ok(());
    }

    for segment in segments {
        let timestamp = format_timestamp(segment.start_time);
        println!("[{}] {}", timestamp, segment.text);
    }

    Ok(())
}

/// Generate and store an AI summary for a recording.
pub async fn summarize_recording(settings: &Settings, id: &str) -> Result<()> {
    let db = Database::open(settings)?;

    let mut recording = db
        .find_recording_by_prefix(id)?
        .context("Recording not found")?;

    let segments = db.get_transcript_segments(&recording.id)?;
    if segments.is_empty() {
        anyhow::bail!(
            "No transcript available for recording {}",
            &recording.id[..8]
        );
    }

    let transcript = build_summary_transcript(&segments);
    let provider = build_provider(settings)?;
    let summary = provider
        .summarize(SummaryRequest {
            title: &recording.title,
            transcript: &transcript,
        })
        .await?;

    recording.notes = Some(summary.clone());
    db.update_recording(&recording)?;

    println!("Summary saved for {}:", &recording.id[..8]);
    println!();
    println!("{}", summary);

    Ok(())
}

/// Search through all transcripts
pub async fn search_transcripts(settings: &Settings, query: &str) -> Result<()> {
    let db = Database::open(settings)?;

    let results = db.search_transcripts(query, 20)?;

    if results.is_empty() {
        println!("No results found for: {}", query);
        return Ok(());
    }

    println!("Found {} results for: {}", results.len(), query);
    println!();

    let mut current_recording_id = String::new();

    for (recording, segment) in results {
        if recording.id != current_recording_id {
            if !current_recording_id.is_empty() {
                println!();
            }
            println!(
                "== {} ({}) ==",
                recording.title,
                recording.created_at.format("%Y-%m-%d")
            );
            current_recording_id = recording.id.clone();
        }

        let timestamp = format_timestamp(segment.start_time);
        println!("  [{}] {}", timestamp, segment.text);
    }

    Ok(())
}

/// Export a recording to a file
pub async fn export_recording(
    settings: &Settings,
    id: &str,
    format: &str,
    output: Option<PathBuf>,
) -> Result<()> {
    let db = Database::open(settings)?;

    let recording = db
        .find_recording_by_prefix(id)?
        .context("Recording not found")?;

    let segments = db.get_transcript_segments(&recording.id)?;

    let content = match format {
        "txt" => export_as_txt(&recording, &segments),
        "json" => export_as_json(&recording, &segments)?,
        "srt" => export_as_srt(&segments),
        _ => anyhow::bail!("Unsupported format: {}. Supported: txt, json, srt", format),
    };

    if let Some(path) = output {
        std::fs::write(&path, content)?;
        println!("Exported to: {}", path.display());
    } else {
        print!("{}", content);
    }

    Ok(())
}

/// Handle daemon subcommands
pub async fn daemon_command(settings: &Settings, cmd: DaemonCommand) -> Result<()> {
    match cmd {
        DaemonCommand::Start { foreground } => {
            if foreground {
                crate::daemon::run_foreground(settings).await?;
            } else {
                crate::daemon::start_daemon(settings)?;
                println!("Daemon started");
            }
        }
        DaemonCommand::Stop => {
            let mut client = DaemonClient::connect(settings).await?;
            client.send(DaemonRequest::Shutdown).await?;
            println!("Daemon stopped");
        }
        DaemonCommand::Restart => {
            // Try to stop existing daemon
            if let Ok(mut client) = DaemonClient::connect(settings).await {
                let _ = client.send(DaemonRequest::Shutdown).await;
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
            crate::daemon::start_daemon(settings)?;
            println!("Daemon restarted");
        }
        DaemonCommand::Status => match DaemonClient::connect(settings).await {
            Ok(mut client) => {
                let response = client.send(DaemonRequest::Ping).await?;
                if matches!(response, DaemonResponse::Pong) {
                    println!("Daemon is running");
                }
            }
            Err(_) => {
                println!("Daemon is not running");
            }
        },
    }

    Ok(())
}

/// Handle config subcommands
pub fn config_command(settings: &Settings, cmd: ConfigCommand) -> Result<()> {
    match cmd {
        ConfigCommand::Show => {
            let toml = toml::to_string_pretty(settings)?;
            println!("{}", toml);
        }
        ConfigCommand::Path => {
            let path = Settings::config_path()?;
            println!("{}", path.display());
        }
        ConfigCommand::Init { force } => {
            let path = Settings::config_path()?;
            if path.exists() && !force {
                anyhow::bail!(
                    "Config file already exists at {}. Use --force to overwrite.",
                    path.display()
                );
            }
            Settings::write_default(&path)?;
            println!("Configuration initialized at: {}", path.display());
        }
        ConfigCommand::Set { key, value } => {
            // Simple key=value setting - would need more sophisticated implementation
            // for nested keys like "whisper.model"
            println!("Setting {}={}", key, value);
            println!("(Note: Manual config editing is recommended for now)");
        }
    }

    Ok(())
}

#[derive(Serialize)]
struct DoctorCapture {
    system: bool,
    microphone: bool,
}

#[derive(Serialize)]
struct DoctorCheck {
    name: &'static str,
    status: &'static str,
    detail: &'static str,
}

#[derive(Serialize)]
struct DoctorPipeWireTarget {
    kind: String,
    target: String,
    method: &'static str,
}

#[derive(Serialize)]
struct DoctorReport {
    backend: String,
    capture: DoctorCapture,
    checks: Vec<DoctorCheck>,
    pipewire_targets: Vec<DoctorPipeWireTarget>,
    notes: Vec<String>,
}

/// Run diagnostic checks to help troubleshoot local setup issues.
pub async fn run_doctor(settings: &Settings, json: bool) -> Result<()> {
    let report = collect_doctor_report(settings);

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    println!("minutes doctor");
    println!("backend: {}", report.backend);
    println!(
        "capture: system={} microphone={}",
        report.capture.system, report.capture.microphone
    );
    println!();

    for check in &report.checks {
        println!("{:<10} {:<8} {}", check.name, check.status, check.detail);
    }

    if !report.pipewire_targets.is_empty() {
        println!();
        println!("PipeWire target resolution:");
        for target in &report.pipewire_targets {
            println!(
                "  - {} target: {} ({})",
                target.kind, target.target, target.method
            );
        }
    }

    if !report.notes.is_empty() {
        println!();
        for note in &report.notes {
            println!("{}", note);
        }
    }

    Ok(())
}

fn collect_doctor_report(settings: &Settings) -> DoctorReport {
    let pw_record_ok = command_exists("pw-record");
    let wpctl_ok = command_exists("wpctl");

    let mut notes = Vec::new();
    let mut pipewire_targets = Vec::new();

    match settings.audio.backend {
        AudioBackend::Cpal => {
            notes.push(
                "info: cpal backend is microphone-only; system audio capture is unavailable."
                    .to_string(),
            );
        }
        AudioBackend::Auto | AudioBackend::PipeWire => {
            #[cfg(feature = "pipewire")]
            {
                if settings.audio.capture_system || settings.audio.capture_microphone {
                    let resolved = crate::audio::resolve_capture_targets(
                        settings.audio.capture_system,
                        settings.audio.capture_microphone,
                    );

                    for target in &resolved {
                        pipewire_targets.push(DoctorPipeWireTarget {
                            kind: target.kind.label().to_string(),
                            target: target.target.clone(),
                            method: target.method.as_str(),
                        });
                    }

                    if resolved.iter().any(|target| {
                        target.method == crate::audio::TargetResolutionMethod::FallbackAlias
                    }) {
                        notes.push("warning: at least one target used alias fallback; this can capture microphone instead of monitor on some setups.".to_string());
                        notes.push("hint: ensure PipeWire/WirePlumber are running and `wpctl inspect @DEFAULT_AUDIO_SINK@` works.".to_string());
                        notes.push("hint: run `wpctl status -n` and confirm a default sink/source is available.".to_string());
                    } else {
                        notes.push(
                            "ok: capture targets resolved to concrete PipeWire node ids."
                                .to_string(),
                        );
                    }
                }
            }

            #[cfg(not(feature = "pipewire"))]
            {
                notes.push("warning: this build has no PipeWire feature enabled.".to_string());
            }
        }
    }

    DoctorReport {
        backend: format!("{:?}", settings.audio.backend),
        capture: DoctorCapture {
            system: settings.audio.capture_system,
            microphone: settings.audio.capture_microphone,
        },
        checks: vec![
            DoctorCheck {
                name: "pw-record",
                status: if pw_record_ok { "ok" } else { "missing" },
                detail: "required for PipeWire capture",
            },
            DoctorCheck {
                name: "wpctl",
                status: if wpctl_ok { "ok" } else { "missing" },
                detail: "used for default sink/source resolution",
            },
        ],
        pipewire_targets,
        notes,
    }
}

// Helper functions

fn command_exists(bin: &str) -> bool {
    Command::new(bin)
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

fn format_duration(secs: u64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;

    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{}:{:02}", minutes, seconds)
    }
}

fn format_timestamp(secs: f64) -> String {
    let total_secs = secs as u64;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{:02}:{:02}", minutes, seconds)
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

use crate::storage::TranscriptSegment;

fn build_summary_transcript(segments: &[TranscriptSegment]) -> String {
    let mut transcript = String::new();
    for segment in segments {
        let timestamp = format_timestamp(segment.start_time);
        transcript.push('[');
        transcript.push_str(&timestamp);
        transcript.push_str("] ");
        transcript.push_str(&segment.text);
        transcript.push('\n');
    }
    transcript
}

fn export_as_txt(recording: &Recording, segments: &[TranscriptSegment]) -> String {
    let mut output = String::new();
    output.push_str(&format!("Title: {}\n", recording.title));
    output.push_str(&format!(
        "Date: {}\n",
        recording.created_at.format("%Y-%m-%d %H:%M")
    ));
    if let Some(duration) = recording.duration_secs {
        output.push_str(&format!("Duration: {}\n", format_duration(duration)));
    }
    output.push_str("\n---\n\n");

    for segment in segments {
        let timestamp = format_timestamp(segment.start_time);
        output.push_str(&format!("[{}] {}\n", timestamp, segment.text));
    }

    output
}

fn export_as_json(recording: &Recording, segments: &[TranscriptSegment]) -> Result<String> {
    #[derive(serde::Serialize)]
    struct ExportData<'a> {
        recording: &'a Recording,
        segments: &'a [TranscriptSegment],
    }

    let data = ExportData {
        recording,
        segments,
    };
    Ok(serde_json::to_string_pretty(&data)?)
}

fn export_as_srt(segments: &[TranscriptSegment]) -> String {
    let mut output = String::new();

    for (i, segment) in segments.iter().enumerate() {
        output.push_str(&format!("{}\n", i + 1));
        output.push_str(&format!(
            "{} --> {}\n",
            format_srt_timestamp(segment.start_time),
            format_srt_timestamp(segment.end_time)
        ));
        output.push_str(&format!("{}\n\n", segment.text));
    }

    output
}

fn format_srt_timestamp(secs: f64) -> String {
    let total_ms = (secs * 1000.0) as u64;
    let hours = total_ms / 3_600_000;
    let minutes = (total_ms % 3_600_000) / 60_000;
    let seconds = (total_ms % 60_000) / 1000;
    let ms = total_ms % 1000;

    format!("{:02}:{:02}:{:02},{:03}", hours, minutes, seconds, ms)
}
