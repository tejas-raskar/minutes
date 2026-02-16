//! Viewer screen - display transcript for a recording

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};

use crate::config::Settings;
use crate::storage::{Recording, TranscriptSegment};

/// Viewer screen state
pub struct ViewerScreen {
    recording: Option<Recording>,
    segments: Vec<TranscriptSegment>,
    scroll_offset: usize,
    content_height: usize,
}

impl Default for ViewerScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewerScreen {
    pub fn new() -> Self {
        Self {
            recording: None,
            segments: Vec::new(),
            scroll_offset: 0,
            content_height: 0,
        }
    }

    pub fn set_recording(&mut self, recording: Recording, segments: Vec<TranscriptSegment>) {
        self.recording = Some(recording);
        self.segments = segments;
        self.scroll_offset = 0;
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect, settings: &Settings) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4), // Header
                Constraint::Min(5),    // Transcript
                Constraint::Length(3), // Help
            ])
            .split(area);

        // Header
        let header_text = if let Some(ref recording) = self.recording {
            let duration = recording
                .duration_secs
                .map(|d| format!("{}:{:02}", d / 60, d % 60))
                .unwrap_or_else(|| "??:??".to_string());

            vec![
                Line::from(vec![Span::styled(
                    &recording.title,
                    Style::default().fg(Color::White).bold(),
                )]),
                Line::from(vec![
                    Span::styled(
                        recording.created_at.format("%Y-%m-%d %H:%M").to_string(),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::raw(" • "),
                    Span::styled(duration, Style::default().fg(Color::Cyan)),
                    Span::raw(" • "),
                    Span::styled(
                        format!("{} segments", self.segments.len()),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
            ]
        } else {
            vec![Line::from("No recording selected")]
        };

        let header = Paragraph::new(header_text).block(
            Block::default()
                .title(" Recording ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        );
        frame.render_widget(header, chunks[0]);

        // Transcript
        let show_timestamps = settings.tui.show_timestamps;
        let transcript_lines: Vec<Line> = self
            .segments
            .iter()
            .map(|segment| {
                if show_timestamps {
                    let timestamp = format_timestamp(segment.start_time);
                    Line::from(vec![
                        Span::styled(
                            format!("[{}] ", timestamp),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::raw(&segment.text),
                    ])
                } else {
                    Line::from(segment.text.as_str())
                }
            })
            .collect();

        self.content_height = transcript_lines.len();

        let transcript_area = chunks[1];
        let visible_height = transcript_area.height.saturating_sub(2) as usize; // Account for borders

        let transcript = Paragraph::new(transcript_lines)
            .wrap(Wrap { trim: false })
            .scroll((self.scroll_offset as u16, 0))
            .block(
                Block::default()
                    .title(" Transcript ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Blue)),
            );
        frame.render_widget(transcript, transcript_area);

        // Scrollbar
        if self.content_height > visible_height {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));

            let mut scrollbar_state = ScrollbarState::new(self.content_height)
                .position(self.scroll_offset)
                .viewport_content_length(visible_height);

            frame.render_stateful_widget(
                scrollbar,
                transcript_area.inner(Margin {
                    horizontal: 0,
                    vertical: 1,
                }),
                &mut scrollbar_state,
            );
        }

        // Help bar
        let help = Paragraph::new(Line::from(vec![
            Span::styled(" ↑/↓ ", Style::default().fg(Color::Black).bg(Color::Cyan)),
            Span::raw(" Scroll  "),
            Span::styled(
                " PgUp/PgDn ",
                Style::default().fg(Color::Black).bg(Color::Cyan),
            ),
            Span::raw(" Page  "),
            Span::styled(" g/G ", Style::default().fg(Color::Black).bg(Color::Cyan)),
            Span::raw(" Top/Bottom  "),
            Span::styled(" Esc ", Style::default().fg(Color::Black).bg(Color::Cyan)),
            Span::raw(" Back"),
        ]))
        .alignment(Alignment::Center);
        frame.render_widget(help, chunks[2]);
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        if self.scroll_offset < self.content_height.saturating_sub(1) {
            self.scroll_offset += 1;
        }
    }

    pub fn page_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(10);
    }

    pub fn page_down(&mut self) {
        self.scroll_offset = (self.scroll_offset + 10).min(self.content_height.saturating_sub(1));
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.content_height.saturating_sub(1);
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
