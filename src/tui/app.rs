//! Main TUI application state and logic

use anyhow::Result;
use crossterm::event::KeyCode;
use ratatui::prelude::*;
use std::time::{Duration, Instant};

use crate::config::Settings;
use crate::daemon::client::DaemonClient;
use crate::daemon::ipc::{DaemonRequest, RecordingStatus};
use crate::storage::Database;
use crate::tui::screens::{BrowserScreen, DashboardScreen, ViewerScreen};
use crate::tui::widgets::HelpPopup;

/// Current screen
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppScreen {
    Dashboard,
    Browser,
    Viewer,
}

/// Main application state
pub struct App {
    settings: Settings,
    current_screen: AppScreen,
    previous_screen: Option<AppScreen>,
    show_help: bool,

    // Screen states
    dashboard: DashboardScreen,
    browser: BrowserScreen,
    viewer: ViewerScreen,

    // Daemon state
    daemon_status: RecordingStatus,
    last_status_update: Instant,
}

impl App {
    /// Create a new app instance
    pub fn new(settings: Settings) -> Result<Self> {
        let db = Database::open(&settings)?;
        let recordings = db.list_recordings(100)?;

        Ok(Self {
            settings,
            current_screen: AppScreen::Dashboard,
            previous_screen: None,
            show_help: false,
            dashboard: DashboardScreen::new(),
            browser: BrowserScreen::new(recordings),
            viewer: ViewerScreen::new(),
            daemon_status: RecordingStatus::Idle,
            last_status_update: Instant::now(),
        })
    }

    /// Draw the current screen
    pub fn draw(&mut self, frame: &mut Frame) {
        let area = frame.size();

        match self.current_screen {
            AppScreen::Dashboard => {
                self.dashboard.draw(frame, area, &self.daemon_status);
            }
            AppScreen::Browser => {
                self.browser.draw(frame, area);
            }
            AppScreen::Viewer => {
                self.viewer.draw(frame, area, &self.settings);
            }
        }

        // Draw help popup if active
        if self.show_help {
            HelpPopup::draw(frame, area, self.current_screen);
        }
    }

    /// Handle key input
    pub async fn handle_key(&mut self, key: KeyCode) -> Result<()> {
        if self.show_help {
            self.show_help = false;
            return Ok(());
        }

        match self.current_screen {
            AppScreen::Dashboard => {
                self.handle_dashboard_key(key).await?;
            }
            AppScreen::Browser => {
                self.handle_browser_key(key).await?;
            }
            AppScreen::Viewer => {
                self.handle_viewer_key(key)?;
            }
        }

        Ok(())
    }

    /// Handle dashboard key input
    async fn handle_dashboard_key(&mut self, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::Char('r') | KeyCode::Enter => {
                // Toggle recording
                self.toggle_recording().await?;
            }
            KeyCode::Char('l') | KeyCode::Tab => {
                self.switch_screen(AppScreen::Browser);
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle browser key input
    async fn handle_browser_key(&mut self, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::Up | KeyCode::Char('k') => {
                self.browser.previous();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.browser.next();
            }
            KeyCode::Enter => {
                if let Some(recording_id) = self.browser.selected().map(|r| r.id.clone()) {
                    self.open_recording(&recording_id)?;
                }
            }
            KeyCode::Char('/') => {
                self.browser.start_search();
            }
            KeyCode::Char('d') => {
                self.switch_screen(AppScreen::Dashboard);
            }
            _ => {
                self.browser.handle_key(key);
            }
        }
        Ok(())
    }

    /// Handle viewer key input
    fn handle_viewer_key(&mut self, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::Up | KeyCode::Char('k') => {
                self.viewer.scroll_up();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.viewer.scroll_down();
            }
            KeyCode::PageUp => {
                self.viewer.page_up();
            }
            KeyCode::PageDown => {
                self.viewer.page_down();
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.viewer.scroll_to_top();
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.viewer.scroll_to_bottom();
            }
            _ => {}
        }
        Ok(())
    }

    /// Toggle recording on/off
    async fn toggle_recording(&mut self) -> Result<()> {
        match DaemonClient::connect(&self.settings).await {
            Ok(mut client) => {
                let request = match &self.daemon_status {
                    RecordingStatus::Idle => DaemonRequest::StartRecording {
                        title: format!(
                            "Meeting {}",
                            chrono::Local::now().format("%Y-%m-%d %H:%M")
                        ),
                    },
                    RecordingStatus::Recording { .. } => DaemonRequest::StopRecording,
                    _ => return Ok(()),
                };

                let _ = client.send(request).await;
            }
            Err(_) => {
                // Daemon not running - could show error in UI
            }
        }
        Ok(())
    }

    /// Open a recording in the viewer
    fn open_recording(&mut self, recording_id: &str) -> Result<()> {
        let db = Database::open(&self.settings)?;

        if let Some(recording) = db.get_recording(recording_id)? {
            let segments = db.get_transcript_segments(recording_id)?;
            self.viewer.set_recording(recording, segments);
            self.switch_screen(AppScreen::Viewer);
        }

        Ok(())
    }

    /// Switch to a different screen
    fn switch_screen(&mut self, screen: AppScreen) {
        self.previous_screen = Some(self.current_screen);
        self.current_screen = screen;
    }

    /// Handle back navigation
    pub fn handle_back(&mut self) {
        if let Some(prev) = self.previous_screen.take() {
            self.current_screen = prev;
        } else if self.current_screen != AppScreen::Dashboard {
            self.current_screen = AppScreen::Dashboard;
        }
    }

    /// Check if app should quit
    pub fn should_quit(&self) -> bool {
        self.current_screen == AppScreen::Dashboard && !self.show_help
    }

    /// Toggle help popup
    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    /// Update app state
    pub async fn update(&mut self) -> Result<()> {
        // Update daemon status periodically
        if self.last_status_update.elapsed() > Duration::from_secs(1) {
            self.update_daemon_status().await;
            self.last_status_update = Instant::now();
        }

        Ok(())
    }

    /// Update daemon status
    async fn update_daemon_status(&mut self) {
        if let Ok(mut client) = DaemonClient::connect(&self.settings).await {
            if let Ok(response) = client.send(DaemonRequest::GetStatus).await {
                if let crate::daemon::ipc::DaemonResponse::Status(status) = response {
                    self.daemon_status = status;
                }
            }
        }
    }

    /// Refresh recordings list
    pub fn refresh_recordings(&mut self) -> Result<()> {
        let db = Database::open(&self.settings)?;
        let recordings = db.list_recordings(100)?;
        self.browser = BrowserScreen::new(recordings);
        Ok(())
    }
}
