use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};

use crate::panel::Panel;
use crate::state::NeilState;

pub struct IntentionsPanel;

impl Panel for IntentionsPanel {
    fn id(&self) -> &str { "intentions" }
    fn title(&self) -> &str { " intentions " }
    fn priority(&self) -> u8 { 2 }

    fn render(&self, area: Rect, buf: &mut Buffer, state: &NeilState) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(self.title())
            .border_style(Style::default().fg(Color::DarkGray));
        let inner = block.inner(area);
        block.render(area, buf);

        let pending: Vec<_> = state.intentions.iter()
            .filter(|i| i.status == "pending")
            .collect();

        if pending.is_empty() {
            let line = Line::from(Span::styled(
                "  (no pending intentions)",
                Style::default().fg(Color::DarkGray),
            ));
            line.render(Rect::new(inner.x, inner.y, inner.width, 1), buf);
            return;
        }

        for (i, intent) in pending.iter().enumerate() {
            if i >= inner.height as usize { break; }

            let prio_color = match intent.priority.as_str() {
                "high" => Color::Red,
                "medium" => Color::Yellow,
                "low" => Color::Green,
                _ => Color::DarkGray,
            };

            let due = if intent.due.is_empty() {
                String::new()
            } else {
                format!(" (due: {})", &intent.due[..16.min(intent.due.len())])
            };

            let tag = if intent.tag.is_empty() {
                String::new()
            } else {
                format!(" #{}", intent.tag)
            };

            let line = Line::from(vec![
                Span::styled(
                    format!("  [{}] ", intent.priority),
                    Style::default().fg(prio_color),
                ),
                Span::styled(
                    intent.description.chars().take(inner.width as usize - 25).collect::<String>(),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!("{}{}", due, tag),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            line.render(Rect::new(inner.x, inner.y + i as u16, inner.width, 1), buf);
        }
    }
}
