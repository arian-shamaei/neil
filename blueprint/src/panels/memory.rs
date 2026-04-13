use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};

use crate::panel::Panel;
use crate::state::NeilState;

pub struct MemoryPanel;

impl Panel for MemoryPanel {
    fn id(&self) -> &str { "memory" }
    fn title(&self) -> &str { " memory palace " }
    fn priority(&self) -> u8 { 3 }

    fn render(&self, area: Rect, buf: &mut Buffer, state: &NeilState) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(self.title())
            .border_style(Style::default().fg(Color::DarkGray));
        let inner = block.inner(area);
        block.render(area, buf);

        let p = &state.palace;
        let summary = Line::from(vec![
            Span::styled(
                format!("{} notes", p.total_notes),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                format!(" | {} classified | {} unclassified",
                    p.classified, p.unclassified),
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        summary.render(Rect::new(inner.x, inner.y, inner.width, 1), buf);

        for (i, wing) in p.wings.iter().enumerate() {
            if i + 1 >= inner.height as usize { break; }

            let rooms: String = wing.rooms.iter()
                .map(|(r, c)| format!("{}({})", r, c))
                .collect::<Vec<_>>()
                .join(", ");

            let line = Line::from(vec![
                Span::styled(
                    format!("  wing/{}", wing.name),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(
                    format!(": {} -> {}", wing.count, rooms),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            line.render(
                Rect::new(inner.x, inner.y + 1 + i as u16, inner.width, 1),
                buf,
            );
        }
    }
}
