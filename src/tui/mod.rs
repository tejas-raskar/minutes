//! TUI module for minutes
//!
//! Interactive terminal user interface using ratatui.

mod app;
pub mod screens;
pub mod widgets;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io;
use std::time::Duration;

use crate::config::Settings;
pub use app::{App, AppScreen};

/// Run the TUI application
pub async fn run(settings: &Settings) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new(settings.clone())?;

    // Run main loop
    let result = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

/// Main application loop
async fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    loop {
        // Draw UI
        terminal.draw(|f| app.draw(f))?;

        // Handle events with timeout for async updates
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            if app.should_quit() {
                                return Ok(());
                            }
                            app.handle_back();
                        }
                        KeyCode::Char('?') => {
                            app.toggle_help();
                        }
                        _ => {
                            app.handle_key(key.code).await?;
                        }
                    }
                }
            }
        }

        // Update app state (check daemon status, etc.)
        app.update().await?;
    }
}
