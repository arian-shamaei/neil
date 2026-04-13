use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::panel::Panel;
use crate::state::NeilState;

pub struct HeaderPanel;

impl Panel for HeaderPanel {
    fn id(&self) -> &str { "header" }
    fn title(&self) -> &str { "" }
    fn priority(&self) -> u8 { 3 }

    fn render(&self, area: Rect, buf: &mut Buffer, state: &NeilState) {
        let time = state.now.format("%H:%M:%S").to_string();
        let date = state.now.format("%Y-%m-%d %a").to_string();

        let name = Span::styled(
            " NEIL ",
            Style::default().fg(Color::Black).bg(Color::Cyan),
        );
        let sep = Span::raw(" | ");
        let dt = Span::styled(
            format!("{} {}", date, time),
            Style::default().fg(Color::DarkGray),
        );
        let beat_info = Span::styled(
            format!(" | beats: {}/50", state.heartbeat.beats_today),
            Style::default().fg(if state.heartbeat.beats_today > 40 {
                Color::Red
            } else {
                Color::DarkGray
            }),
        );
        let queue = if state.system.queue_count > 0 {
            Span::styled(
                format!(" | queue: {}", state.system.queue_count),
                Style::default().fg(Color::Yellow),
            )
        } else {
            Span::styled(" | queue: 0", Style::default().fg(Color::DarkGray))
        };

        let line = Line::from(vec![name, sep, dt, beat_info, queue]);
        line.render(area, buf);
    }
}
