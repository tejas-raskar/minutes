//! Daemon module for minutes
//!
//! Handles background recording service and IPC communication.

pub mod client;
pub mod ipc;
pub mod server;
pub mod service;
pub mod state;

use anyhow::Result;
use std::process::Command;
use std::time::{Duration, Instant};

use crate::config::Settings;

/// Start the daemon as a background process
pub fn start_daemon(settings: &Settings) -> Result<()> {
    let pid_path = settings.pid_path();
    let socket_path = settings.socket_path();

    // Check if already running
    if pid_path.exists() {
        if let Ok(pid_str) = std::fs::read_to_string(&pid_path) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                // Check if process is still alive
                if std::path::Path::new(&format!("/proc/{}", pid)).exists() {
                    anyhow::bail!("Daemon is already running (PID: {})", pid);
                }
            }
        }
        // Stale PID file, remove it
        std::fs::remove_file(&pid_path)?;
    }

    // Remove stale socket file before starting a fresh daemon instance.
    if socket_path.exists() {
        let _ = std::fs::remove_file(&socket_path);
    }

    // Start daemon process
    let exe = std::env::current_exe()?;
    let mut child = Command::new(exe)
        .args(["daemon", "start", "--foreground"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;

    // Wait for daemon readiness so callers don't get a false positive start.
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait()? {
            anyhow::bail!(
                "Daemon failed to start (exit: {}). Run `minutes daemon start --foreground` for details.",
                status
            );
        }

        if pid_path.exists() && socket_path.exists() {
            return Ok(());
        }

        std::thread::sleep(Duration::from_millis(50));
    }

    anyhow::bail!("Daemon start timed out. Run `minutes daemon start --foreground` for details.")
}

/// Run the daemon in the foreground
pub async fn run_foreground(settings: &Settings) -> Result<()> {
    service::run(settings).await
}
