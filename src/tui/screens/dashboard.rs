//! Dashboard screen - main landing page with recording status

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::daemon::ipc::RecordingStatus;

/// Dashboard screen state
pub struct DashboardScreen {
    // Add any dashboard-specific state here
}

impl Default for DashboardScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl DashboardScreen {
    pub fn new() -> Self {
        Self {}
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, status: &RecordingStatus) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Length(7), // Status
                Constraint::Min(5),    // Info
                Constraint::Length(3), // Help
            ])
            .split(area);

        // Title
        let title = Paragraph::new("minutes")
            .style(Style::default().fg(Color::Cyan).bold())
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::BOTTOM));
        frame.render_widget(title, chunks[0]);

        // Recording status
        let (status_text, status_style) = match status {
            RecordingStatus::Idle => (
                vec![
                    Line::from(vec![
                        Span::raw("Status: "),
                        Span::styled("Not Recording", Style::default().fg(Color::Gray)),
                    ]),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Press [r] to start recording",
                        Style::default().fg(Color::DarkGray),
                    )),
                ],
                Style::default(),
            ),
            RecordingStatus::Recording {
                title,
                duration_secs,
                audio_level,
                ..
            } => {
                let minutes = duration_secs / 60;
                let seconds = duration_secs % 60;
                let level_bar = create_level_bar(*audio_level);

                (
                    vec![
                        Line::from(vec![
                            Span::raw("Status: "),
                            Span::styled("● Recording", Style::default().fg(Color::Red).bold()),
                        ]),
                        Line::from(vec![
                            Span::raw("Title: "),
                            Span::styled(title, Style::default().fg(Color::White)),
                        ]),
                        Line::from(vec![
                            Span::raw("Duration: "),
                            Span::styled(
                                format!("{:02}:{:02}", minutes, seconds),
                                Style::default().fg(Color::Yellow),
                            ),
                        ]),
                        Line::from(vec![
                            Span::raw("Audio: "),
                            Span::styled(level_bar, Style::default().fg(Color::Green)),
                        ]),
                        Line::from(""),
                        Line::from(Span::styled(
                            "Press [r] to stop recording",
                            Style::default().fg(Color::DarkGray),
                        )),
                    ],
                    Style::default(),
                )
            }
            RecordingStatus::Transcribing { id, progress } => (
                vec![
                    Line::from(vec![
                        Span::raw("Status: "),
                        Span::styled("Transcribing...", Style::default().fg(Color::Yellow)),
                    ]),
                    Line::from(vec![
                        Span::raw("Recording: "),
                        Span::styled(&id[..8], Style::default().fg(Color::White)),
                    ]),
                    Line::from(vec![
                        Span::raw("Progress: "),
                        Span::styled(
                            format!("{:.0}%", progress * 100.0),
                            Style::default().fg(Color::Cyan),
                        ),
                    ]),
                ],
                Style::default(),
            ),
        };

        let status_widget = Paragraph::new(status_text).style(status_style).block(
            Block::default()
                .title(" Recording Status ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        );
        frame.render_widget(status_widget, chunks[1]);

        // Info section
        let info_text = vec![
            Line::from(Span::styled(
                "Welcome to minutes",
                Style::default().fg(Color::White).bold(),
            )),
            Line::from(""),
            Line::from("A lightweight meeting recording and transcription tool."),
            Line::from(""),
            Line::from(vec![
                Span::raw("• Press "),
                Span::styled("[r]", Style::default().fg(Color::Cyan)),
                Span::raw(" to start/stop recording"),
            ]),
            Line::from(vec![
                Span::raw("• Press "),
                Span::styled("[l]", Style::default().fg(Color::Cyan)),
                Span::raw(" to browse recordings"),
            ]),
            Line::from(vec![
                Span::raw("• Press "),
                Span::styled("[?]", Style::default().fg(Color::Cyan)),
                Span::raw(" for help"),
            ]),
        ];

        let info_widget = Paragraph::new(info_text).wrap(Wrap { trim: true }).block(
            Block::default()
                .title(" Info ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
        frame.render_widget(info_widget, chunks[2]);

        // Help bar
        let help = Paragraph::new(Line::from(vec![
            Span::styled(" [r] ", Style::default().fg(Color::Black).bg(Color::Cyan)),
            Span::raw(" Record  "),
            Span::styled(" [l] ", Style::default().fg(Color::Black).bg(Color::Cyan)),
            Span::raw(" List  "),
            Span::styled(" [?] ", Style::default().fg(Color::Black).bg(Color::Cyan)),
            Span::raw(" Help  "),
            Span::styled(" [q] ", Style::default().fg(Color::Black).bg(Color::Cyan)),
            Span::raw(" Quit"),
        ]))
        .alignment(Alignment::Center);
        frame.render_widget(help, chunks[3]);
    }
}

fn create_level_bar(level: f32) -> String {
    let filled = (level * 20.0) as usize;
    let empty = 20 - filled.min(20);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}
