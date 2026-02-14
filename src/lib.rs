//! minutes - A lightweight Linux CLI tool for meeting recording, transcription, and AI-powered insights
//!
//! "minutes" is a playful take on "minutes" (meeting notes)

pub mod audio;
pub mod cli;
pub mod config;
pub mod daemon;
pub mod llm;
pub mod storage;
pub mod transcription;
pub mod tui;

use thiserror::Error;

/// Main error type for minutes
#[derive(Error, Debug)]
pub enum MintuesError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Audio error: {0}")]
    Audio(String),

    #[error("Transcription error: {0}")]
    Transcription(String),

    #[error("IPC error: {0}")]
    Ipc(String),

    #[error("Daemon error: {0}")]
    Daemon(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, MintuesError>;

/// Application version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Application name
pub const APP_NAME: &str = "minutes";
