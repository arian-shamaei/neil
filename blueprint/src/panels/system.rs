use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};

use crate::panel::Panel;
use crate::state::NeilState;

pub struct SystemPanel;

impl Panel for SystemPanel {
    fn id(&self) -> &str { "system" }
    fn title(&self) -> &str { " system " }
    fn priority(&self) -> u8 { 2 }

    fn render(&self, area: Rect, buf: &mut Buffer, state: &NeilState) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(self.title())
            .border_style(Style::default().fg(Color::DarkGray));
        let inner = block.inner(area);
        block.render(area, buf);

        let ap_status = if state.system.autoprompt_active { "active" } else { "DOWN" };
        let ap_color = if state.system.autoprompt_active { Color::Green } else { Color::Red };
        let q_color = if state.system.queue_count > 0 { Color::Yellow } else { Color::DarkGray };

        let lines: Vec<(String, String, Color)> = vec![
            ("essence".into(), format!("{} files", state.essence_files.len()), Color::Cyan),
            ("services".into(), format!("{} registered", state.services.len()), Color::Cyan),
            ("autoprompt".into(), ap_status.into(), ap_color),
            ("queue".into(), format!("{} pending", state.system.queue_count), q_color),
        ];

        for (i, (label, value, color)) in lines.iter().enumerate() {
            if i >= inner.height as usize { break; }
            let line = Line::from(vec![
                Span::styled(
                    format!("  {:<12}", label),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    value.clone(),
                    Style::default().fg(*color),
                ),
            ]);
            line.render(Rect::new(inner.x, inner.y + i as u16, inner.width, 1), buf);
        }
    }
}
