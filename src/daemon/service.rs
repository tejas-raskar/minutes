//! Main daemon service implementation

use anyhow::Result;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::audio::AudioCapture;
use crate::config::Settings;
use crate::daemon::ipc::{DaemonRequest, DaemonResponse};
use crate::daemon::server::{CommandReceiver, IpcServer};
use crate::daemon::state::{ActiveRecording, DaemonState, SharedState, TranscriptionState, new_shared_state};
use crate::storage::{Database, Recording, RecordingState};
use crate::transcription::TranscriptionPipeline;

/// Run the daemon service
pub async fn run(settings: &Settings) -> Result<()> {
    info!("Starting minutes daemon");

    // Ensure directories exist
    settings.ensure_dirs()?;

    // Write PID file
    let pid = std::process::id();
    std::fs::write(settings.pid_path(), pid.to_string())?;

    // Initialize shared state
    let state = new_shared_state();

    // Create command channel
    let (cmd_tx, cmd_rx) = mpsc::channel::<(DaemonRequest, mpsc::Sender<DaemonResponse>)>(32);

    // Start IPC server
    let mut server = IpcServer::new(settings.socket_path());
    server.start().await?;

    // Spawn server task
    let server_handle = tokio::spawn(async move {
        if let Err(e) = server.run(cmd_tx).await {
            error!("IPC server error: {}", e);
        }
    });

    // Spawn transcription worker
    let transcription_state = state.clone();
    let transcription_settings = settings.clone();
    let transcription_handle = tokio::spawn(async move {
        transcription_worker(transcription_settings, transcription_state).await;
    });

    // Run command handler
    let handler_result = command_handler(settings.clone(), state, cmd_rx).await;

    // Cleanup
    info!("Shutting down daemon");

    // Remove PID file
    let _ = std::fs::remove_file(settings.pid_path());

    // Abort spawned tasks
    server_handle.abort();
    transcription_handle.abort();

    handler_result
}

/// Handle incoming commands
async fn command_handler(
    settings: Settings,
    state: SharedState,
    mut cmd_rx: CommandReceiver,
) -> Result<()> {
    let mut audio_capture: Option<AudioCapture> = None;

    while let Some((request, resp_tx)) = cmd_rx.recv().await {
        let response = match request {
            DaemonRequest::StartRecording { title } => {
                handle_start_recording(&settings, &state, &mut audio_capture, title).await
            }
            DaemonRequest::StopRecording => {
                handle_stop_recording(&settings, &state, &mut audio_capture).await
            }
            DaemonRequest::GetStatus => {
                let state = state.read().await;
                DaemonResponse::Status(state.to_status())
            }
            DaemonRequest::Ping => DaemonResponse::Pong,
            DaemonRequest::Shutdown => {
                let _ = resp_tx.send(DaemonResponse::Ok).await;
                break;
            }
            DaemonRequest::Transcribe { recording_id } => {
                handle_transcribe_request(&settings, &recording_id).await
            }
        };

        let _ = resp_tx.send(response).await;
    }

    Ok(())
}

/// Handle start recording request
async fn handle_start_recording(
    settings: &Settings,
    state: &SharedState,
    audio_capture: &mut Option<AudioCapture>,
    title: String,
) -> DaemonResponse {
    let mut state_guard = state.write().await;

    // Check if already recording
    if matches!(*state_guard, DaemonState::Recording(_)) {
        return DaemonResponse::Error {
            message: "Already recording".to_string(),
        };
    }

    // Create new recording
    let recording = Recording::new(title);
    let audio_filename = format!("{}.wav", recording.id);
    let audio_path = settings.audio_dir().join(&audio_filename);

    // Initialize audio capture
    match AudioCapture::new(settings, &audio_path) {
        Ok(mut capture) => {
            if let Err(e) = capture.start() {
                return DaemonResponse::Error {
                    message: format!("Failed to start audio capture: {}", e),
                };
            }
            *audio_capture = Some(capture);
        }
        Err(e) => {
            return DaemonResponse::Error {
                message: format!("Failed to initialize audio: {}", e),
            };
        }
    }

    // Save to database
    let db = match Database::open(settings) {
        Ok(db) => db,
        Err(e) => {
            return DaemonResponse::Error {
                message: format!("Database error: {}", e),
            };
        }
    };

    let mut db_recording = recording.clone();
    db_recording.audio_path = Some(audio_path.to_string_lossy().to_string());

    if let Err(e) = db.insert_recording(&db_recording) {
        return DaemonResponse::Error {
            message: format!("Failed to save recording: {}", e),
        };
    }

    let id = recording.id.clone();

    // Update state
    *state_guard = DaemonState::Recording(ActiveRecording {
        recording,
        audio_path,
        started_at: Instant::now(),
        audio_level: 0.0,
    });

    info!("Recording started: {}", id);
    DaemonResponse::RecordingStarted { id }
}

/// Handle stop recording request
async fn handle_stop_recording(
    settings: &Settings,
    state: &SharedState,
    audio_capture: &mut Option<AudioCapture>,
) -> DaemonResponse {
    let mut state_guard = state.write().await;

    let active = match &*state_guard {
        DaemonState::Recording(active) => active,
        _ => {
            return DaemonResponse::Error {
                message: "Not recording".to_string(),
            };
        }
    };

    let id = active.recording.id.clone();
    let duration_secs = active.started_at.elapsed().as_secs();

    // Stop audio capture
    if let Some(ref mut capture) = audio_capture {
        if let Err(e) = capture.stop() {
            warn!("Error stopping audio capture: {}", e);
        }
    }
    *audio_capture = None;

    // Update database
    let db = match Database::open(settings) {
        Ok(db) => db,
        Err(e) => {
            return DaemonResponse::Error {
                message: format!("Database error: {}", e),
            };
        }
    };

    if let Ok(Some(mut recording)) = db.get_recording(&id) {
        recording.duration_secs = Some(duration_secs);
        recording.state = RecordingState::Pending;
        if let Err(e) = db.update_recording(&recording) {
            warn!("Failed to update recording: {}", e);
        }
    }

    // Update state to idle
    *state_guard = DaemonState::Idle;

    info!("Recording stopped: {} ({}s)", id, duration_secs);
    DaemonResponse::RecordingStopped { id, duration_secs }
}

/// Handle transcription request
async fn handle_transcribe_request(settings: &Settings, recording_id: &str) -> DaemonResponse {
    let db = match Database::open(settings) {
        Ok(db) => db,
        Err(e) => {
            return DaemonResponse::Error {
                message: format!("Database error: {}", e),
            };
        }
    };

    match db.find_recording_by_prefix(recording_id) {
        Ok(Some(mut recording)) => {
            recording.state = RecordingState::Pending;
            if let Err(e) = db.update_recording(&recording) {
                return DaemonResponse::Error {
                    message: format!("Failed to queue transcription: {}", e),
                };
            }
            DaemonResponse::Ok
        }
        Ok(None) => DaemonResponse::Error {
            message: "Recording not found".to_string(),
        },
        Err(e) => DaemonResponse::Error {
            message: format!("Database error: {}", e),
        },
    }
}

/// Background worker that processes pending transcriptions
async fn transcription_worker(settings: Settings, state: SharedState) {
    let check_interval = std::time::Duration::from_secs(5);

    loop {
        tokio::time::sleep(check_interval).await;

        // Skip if currently recording or transcribing
        {
            let state_guard = state.read().await;
            if !matches!(*state_guard, DaemonState::Idle) {
                continue;
            }
        }

        // Check for pending recordings
        let db = match Database::open(&settings) {
            Ok(db) => db,
            Err(e) => {
                error!("Database error in transcription worker: {}", e);
                continue;
            }
        };

        let pending = match db.get_pending_recordings() {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to get pending recordings: {}", e);
                continue;
            }
        };

        for recording in pending {
            // Update state
            {
                let mut state_guard = state.write().await;
                *state_guard = DaemonState::Transcribing(TranscriptionState {
                    recording_id: recording.id.clone(),
                    progress: 0.0,
                });
            }

            info!("Starting transcription for: {}", recording.id);

            // Run transcription
            let result = run_transcription(&settings, &recording, &state).await;

            // Update state back to idle
            {
                let mut state_guard = state.write().await;
                *state_guard = DaemonState::Idle;
            }

            match result {
                Ok(_) => {
                    info!("Transcription completed: {}", recording.id);
                }
                Err(e) => {
                    error!("Transcription failed for {}: {}", recording.id, e);
                    // Mark as failed
                    if let Err(e) = db.update_recording_state(&recording.id, RecordingState::Failed) {
                        error!("Failed to update recording state: {}", e);
                    }
                }
            }
        }
    }
}

/// Run transcription for a recording
async fn run_transcription(
    settings: &Settings,
    recording: &Recording,
    state: &SharedState,
) -> Result<()> {
    let db = Database::open(settings)?;

    // Mark as transcribing
    db.update_recording_state(&recording.id, RecordingState::Transcribing)?;

    // Get audio path
    let audio_path = recording
        .audio_path
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No audio path"))?;

    // Run transcription
    let pipeline = TranscriptionPipeline::new(settings)?;

    let progress_state = state.clone();
    let recording_id = recording.id.clone();

    let segments = pipeline
        .transcribe(
            audio_path,
            &recording.id,
            Box::new(move |progress| {
                let state = progress_state.clone();
                let _id = recording_id.clone();
                tokio::spawn(async move {
                    let mut state_guard = state.write().await;
                    if let DaemonState::Transcribing(ref mut ts) = *state_guard {
                        ts.progress = progress;
                    }
                });
            }),
        )
        .await?;

    // Save segments
    db.insert_segments(&segments)?;

    // Mark as completed
    db.update_recording_state(&recording.id, RecordingState::Completed)?;

    Ok(())
}
