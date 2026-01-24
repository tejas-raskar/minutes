//! Browser screen - list and search recordings

use crossterm::event::KeyCode;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use crate::storage::Recording;

/// Browser screen state
pub struct BrowserScreen {
    recordings: Vec<Recording>,
    state: ListState,
    search_mode: bool,
    search_query: String,
    filtered_indices: Vec<usize>,
}

impl BrowserScreen {
    pub fn new(recordings: Vec<Recording>) -> Self {
        let mut state = ListState::default();
        if !recordings.is_empty() {
            state.select(Some(0));
        }

        let filtered_indices = (0..recordings.len()).collect();

        Self {
            recordings,
            state,
            search_mode: false,
            search_query: String::new(),
            filtered_indices,
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Search bar
                Constraint::Min(5),    // List
                Constraint::Length(3), // Help
            ])
            .split(area);

        // Search bar
        let search_style = if self.search_mode {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let search_text = if self.search_mode {
            format!("Search: {}█", self.search_query)
        } else if self.search_query.is_empty() {
            "Press [/] to search".to_string()
        } else {
            format!("Search: {}", self.search_query)
        };

        let search = Paragraph::new(search_text)
            .style(search_style)
            .block(Block::default().borders(Borders::ALL).title(" Search "));
        frame.render_widget(search, chunks[0]);

        // Recordings list
        let items: Vec<ListItem> = self
            .filtered_indices
            .iter()
            .map(|&i| {
                let recording = &self.recordings[i];
                let duration = recording
                    .duration_secs
                    .map(|d| format!("{}:{:02}", d / 60, d % 60))
                    .unwrap_or_else(|| "??:??".to_string());

                let date = recording.created_at.format("%Y-%m-%d %H:%M").to_string();

                let state_indicator = match recording.state {
                    crate::storage::RecordingState::Recording => "●",
                    crate::storage::RecordingState::Pending => "○",
                    crate::storage::RecordingState::Transcribing => "◐",
                    crate::storage::RecordingState::Completed => "✓",
                    crate::storage::RecordingState::Failed => "✗",
                };

                let state_color = match recording.state {
                    crate::storage::RecordingState::Recording => Color::Red,
                    crate::storage::RecordingState::Pending => Color::Yellow,
                    crate::storage::RecordingState::Transcribing => Color::Cyan,
                    crate::storage::RecordingState::Completed => Color::Green,
                    crate::storage::RecordingState::Failed => Color::Red,
                };

                ListItem::new(Line::from(vec![
                    Span::styled(state_indicator, Style::default().fg(state_color)),
                    Span::raw(" "),
                    Span::styled(
                        truncate(&recording.title, 30),
                        Style::default().fg(Color::White),
                    ),
                    Span::raw(" "),
                    Span::styled(date, Style::default().fg(Color::DarkGray)),
                    Span::raw(" "),
                    Span::styled(duration, Style::default().fg(Color::Cyan)),
                ]))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title(format!(" Recordings ({}) ", self.filtered_indices.len()))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Blue)),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        frame.render_stateful_widget(list, chunks[1], &mut self.state);

        // Help bar
        let help = Paragraph::new(Line::from(vec![
            Span::styled(" ↑/↓ ", Style::default().fg(Color::Black).bg(Color::Cyan)),
            Span::raw(" Navigate  "),
            Span::styled(" Enter ", Style::default().fg(Color::Black).bg(Color::Cyan)),
            Span::raw(" View  "),
            Span::styled(" / ", Style::default().fg(Color::Black).bg(Color::Cyan)),
            Span::raw(" Search  "),
            Span::styled(" d ", Style::default().fg(Color::Black).bg(Color::Cyan)),
            Span::raw(" Dashboard  "),
            Span::styled(" Esc ", Style::default().fg(Color::Black).bg(Color::Cyan)),
            Span::raw(" Back"),
        ]))
        .alignment(Alignment::Center);
        frame.render_widget(help, chunks[2]);
    }

    pub fn next(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }

        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.filtered_indices.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn previous(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }

        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.filtered_indices.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn selected(&self) -> Option<&Recording> {
        self.state
            .selected()
            .and_then(|i| self.filtered_indices.get(i))
            .map(|&i| &self.recordings[i])
    }

    pub fn start_search(&mut self) {
        self.search_mode = true;
    }

    pub fn handle_key(&mut self, key: KeyCode) {
        if !self.search_mode {
            return;
        }

        match key {
            KeyCode::Char(c) => {
                self.search_query.push(c);
                self.apply_filter();
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                self.apply_filter();
            }
            KeyCode::Enter | KeyCode::Esc => {
                self.search_mode = false;
            }
            _ => {}
        }
    }

    fn apply_filter(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_indices = (0..self.recordings.len()).collect();
        } else {
            let query = self.search_query.to_lowercase();
            self.filtered_indices = self
                .recordings
                .iter()
                .enumerate()
                .filter(|(_, r)| r.title.to_lowercase().contains(&query))
                .map(|(i, _)| i)
                .collect();
        }

        // Reset selection
        if !self.filtered_indices.is_empty() {
            self.state.select(Some(0));
        } else {
            self.state.select(None);
        }
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        format!("{:<width$}", s, width = max_len)
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
