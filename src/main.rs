//! minutes - Meeting recording, transcription, and AI-powered insights
//!
//! Entry point for the minutes CLI application.

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use minutes::cli::{Cli, Commands};
use minutes::config::Settings;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_writer(std::io::stderr),
        )
        .init();

    // Load configuration
    let settings = Settings::load()?;

    // Parse CLI arguments
    let cli = Cli::parse();

    // Execute command
    match cli.command {
        Commands::Start { title } => {
            minutes::cli::commands::start_recording(&settings, title).await?;
        }
        Commands::Stop => {
            minutes::cli::commands::stop_recording(&settings).await?;
        }
        Commands::Status => {
            minutes::cli::commands::show_status(&settings).await?;
        }
        Commands::List { limit, search } => {
            minutes::cli::commands::list_recordings(&settings, limit, search).await?;
        }
        Commands::View { id } => {
            minutes::cli::commands::view_recording(&settings, &id).await?;
        }
        Commands::Search { query } => {
            minutes::cli::commands::search_transcripts(&settings, &query).await?;
        }
        Commands::Doctor { json } => {
            minutes::cli::commands::run_doctor(&settings, json).await?;
        }
        Commands::Summarize { id } => {
            minutes::cli::commands::summarize_recording(&settings, &id).await?;
        }
        Commands::Export { id, format, output } => {
            minutes::cli::commands::export_recording(&settings, &id, &format, output).await?;
        }
        Commands::Daemon(daemon_cmd) => {
            minutes::cli::commands::daemon_command(&settings, daemon_cmd).await?;
        }
        Commands::Tui => {
            minutes::tui::run(&settings).await?;
        }
        Commands::Config(config_cmd) => {
            minutes::cli::commands::config_command(&settings, config_cmd)?;
        }
    }

    Ok(())
}
