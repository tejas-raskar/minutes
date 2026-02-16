//! Help popup widget

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::tui::AppScreen;

/// Help popup that shows keyboard shortcuts
pub struct HelpPopup;

impl HelpPopup {
    pub fn draw(frame: &mut Frame, area: Rect, screen: AppScreen) {
        // Calculate popup area (centered, 60% width, 70% height)
        let popup_width = (area.width as f32 * 0.6) as u16;
        let popup_height = (area.height as f32 * 0.7) as u16;
        let popup_x = (area.width - popup_width) / 2;
        let popup_y = (area.height - popup_height) / 2;

        let popup_area = Rect {
            x: popup_x,
            y: popup_y,
            width: popup_width,
            height: popup_height,
        };

        // Clear the area behind the popup
        frame.render_widget(Clear, popup_area);

        let help_text = match screen {
            AppScreen::Dashboard => vec![
                Line::from(Span::styled(
                    "Dashboard Shortcuts",
                    Style::default().fg(Color::Cyan).bold(),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled("r", Style::default().fg(Color::Yellow)),
                    Span::raw("       Start/stop recording"),
                ]),
                Line::from(vec![
                    Span::styled("l", Style::default().fg(Color::Yellow)),
                    Span::raw("       List recordings"),
                ]),
                Line::from(vec![
                    Span::styled("Tab", Style::default().fg(Color::Yellow)),
                    Span::raw("     Switch to browser"),
                ]),
                Line::from(vec![
                    Span::styled("?", Style::default().fg(Color::Yellow)),
                    Span::raw("       Show this help"),
                ]),
                Line::from(vec![
                    Span::styled("q", Style::default().fg(Color::Yellow)),
                    Span::raw("       Quit application"),
                ]),
            ],
            AppScreen::Browser => vec![
                Line::from(Span::styled(
                    "Browser Shortcuts",
                    Style::default().fg(Color::Cyan).bold(),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled("↑/k", Style::default().fg(Color::Yellow)),
                    Span::raw("     Move up"),
                ]),
                Line::from(vec![
                    Span::styled("↓/j", Style::default().fg(Color::Yellow)),
                    Span::raw("     Move down"),
                ]),
                Line::from(vec![
                    Span::styled("Enter", Style::default().fg(Color::Yellow)),
                    Span::raw("   View transcript"),
                ]),
                Line::from(vec![
                    Span::styled("/", Style::default().fg(Color::Yellow)),
                    Span::raw("       Search recordings"),
                ]),
                Line::from(vec![
                    Span::styled("d", Style::default().fg(Color::Yellow)),
                    Span::raw("       Go to dashboard"),
                ]),
                Line::from(vec![
                    Span::styled("Esc", Style::default().fg(Color::Yellow)),
                    Span::raw("     Go back"),
                ]),
            ],
            AppScreen::Viewer => vec![
                Line::from(Span::styled(
                    "Viewer Shortcuts",
                    Style::default().fg(Color::Cyan).bold(),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled("↑/k", Style::default().fg(Color::Yellow)),
                    Span::raw("     Scroll up"),
                ]),
                Line::from(vec![
                    Span::styled("↓/j", Style::default().fg(Color::Yellow)),
                    Span::raw("     Scroll down"),
                ]),
                Line::from(vec![
                    Span::styled("PgUp", Style::default().fg(Color::Yellow)),
                    Span::raw("    Page up"),
                ]),
                Line::from(vec![
                    Span::styled("PgDn", Style::default().fg(Color::Yellow)),
                    Span::raw("    Page down"),
                ]),
                Line::from(vec![
                    Span::styled("g", Style::default().fg(Color::Yellow)),
                    Span::raw("       Go to top"),
                ]),
                Line::from(vec![
                    Span::styled("G", Style::default().fg(Color::Yellow)),
                    Span::raw("       Go to bottom"),
                ]),
                Line::from(vec![
                    Span::styled("Esc", Style::default().fg(Color::Yellow)),
                    Span::raw("     Go back"),
                ]),
            ],
        };

        let mut full_text = help_text;
        full_text.push(Line::from(""));
        full_text.push(Line::from(Span::styled(
            "Press any key to close",
            Style::default().fg(Color::DarkGray),
        )));

        let help = Paragraph::new(full_text).wrap(Wrap { trim: true }).block(
            Block::default()
                .title(" Help ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .style(Style::default().bg(Color::Black)),
        );

        frame.render_widget(help, popup_area);
    }
}
