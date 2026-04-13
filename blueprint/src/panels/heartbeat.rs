use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};

use crate::panel::Panel;
use crate::state::NeilState;

pub struct HeartbeatPanel;

impl Panel for HeartbeatPanel {
    fn id(&self) -> &str { "heartbeat" }
    fn title(&self) -> &str { " heartbeat " }
    fn priority(&self) -> u8 { 3 }

    fn render(&self, area: Rect, buf: &mut Buffer, state: &NeilState) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(self.title())
            .border_style(Style::default().fg(Color::DarkGray));
        let inner = block.inner(area);
        block.render(area, buf);

        let entries = &state.heartbeat.entries;
        let max_lines = inner.height as usize;
        let start = if entries.len() > max_lines {
            entries.len() - max_lines
        } else { 0 };

        for (i, entry) in entries[start..].iter().enumerate() {
            if i >= max_lines { break; }

            let status_color = match entry.status.as_str() {
                "ok" => Color::Green,
                "acted" => Color::Cyan,
                "error" => Color::Red,
                _ => Color::DarkGray,
            };

            let ts = if entry.timestamp.len() > 11 {
                &entry.timestamp[11..]
            } else {
                &entry.timestamp
            };

            let line = Line::from(vec![
                Span::styled(
                    format!("{} ", ts),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("[{}] ", entry.status),
                    Style::default().fg(status_color),
                ),
                Span::styled(
                    entry.summary.chars().take(inner.width as usize - 20).collect::<String>(),
                    Style::default().fg(Color::White),
                ),
            ]);
            line.render(Rect::new(inner.x, inner.y + i as u16, inner.width, 1), buf);
        }
    }
}
