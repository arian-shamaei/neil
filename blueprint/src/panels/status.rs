use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::panel::Panel;
use crate::state::NeilState;

pub struct StatusPanel;

impl Panel for StatusPanel {
    fn id(&self) -> &str { "status" }
    fn title(&self) -> &str { "" }
    fn priority(&self) -> u8 { 3 }

    fn render(&self, area: Rect, buf: &mut Buffer, state: &NeilState) {
        let pending_intents: usize = state.intentions.iter()
            .filter(|i| i.status == "pending")
            .count();
        let unresolved_fails: usize = state.failures.iter()
            .filter(|f| f.resolution == "pending")
            .count();

        let last = if state.heartbeat.last_beat.is_empty() {
            "never".to_string()
        } else {
            state.heartbeat.last_beat.clone()
        };

        let line = Line::from(vec![
            Span::styled(" last beat: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&last, Style::default().fg(Color::White)),
            Span::styled(
                format!(" | intents: {}", pending_intents),
                Style::default().fg(if pending_intents > 0 { Color::Yellow } else { Color::DarkGray }),
            ),
            Span::styled(
                format!(" | failures: {}", unresolved_fails),
                Style::default().fg(if unresolved_fails > 0 { Color::Red } else { Color::DarkGray }),
            ),
            Span::styled(
                format!(" | notes: {}", state.palace.total_notes),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(" | q to quit", Style::default().fg(Color::DarkGray)),
        ]);
        line.render(area, buf);
    }
}
