//! CLI argument definitions using clap

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// minutes - Meeting recording, transcription, and AI-powered insights
#[derive(Parser, Debug)]
#[command(name = "minutes")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start recording a new meeting
    Start {
        /// Optional title for the recording
        #[arg(short, long)]
        title: Option<String>,
    },

    /// Stop the current recording
    Stop,

    /// Show current recording status
    Status,

    /// List recorded meetings
    List {
        /// Maximum number of recordings to show
        #[arg(short, long, default_value = "10")]
        limit: usize,

        /// Search term to filter recordings
        #[arg(short, long)]
        search: Option<String>,
    },

    /// View a specific recording's transcript
    View {
        /// Recording ID or partial ID
        id: String,
    },

    /// Search through all transcripts
    Search {
        /// Search query (supports full-text search)
        query: String,
    },

    /// Export a recording to a file
    Export {
        /// Recording ID
        id: String,

        /// Output format (txt, json, srt)
        #[arg(short, long, default_value = "txt")]
        format: String,

        /// Output file path (defaults to stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Daemon management commands
    #[command(subcommand)]
    Daemon(DaemonCommand),

    /// Launch the interactive TUI
    Tui,

    /// Configuration management
    #[command(subcommand)]
    Config(ConfigCommand),
}

#[derive(Subcommand, Debug)]
pub enum DaemonCommand {
    /// Start the background daemon
    Start {
        /// Run in foreground (don't daemonize)
        #[arg(short, long)]
        foreground: bool,
    },

    /// Stop the running daemon
    Stop,

    /// Restart the daemon
    Restart,

    /// Check daemon status
    Status,
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommand {
    /// Show current configuration
    Show,

    /// Show configuration file path
    Path,

    /// Initialize default configuration
    Init {
        /// Force overwrite existing config
        #[arg(short, long)]
        force: bool,
    },

    /// Set a configuration value
    Set {
        /// Configuration key (e.g., whisper.model)
        key: String,

        /// Value to set
        value: String,
    },
}
