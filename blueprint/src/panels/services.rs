// Minimal stub — the prior implementation was removed but main.rs still
// references this path. Returns an empty panel so the binary compiles.
// TODO: restore or retire this panel.

use crate::state::NeilState;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

pub fn render(_state: &NeilState) -> Vec<Line<'static>> {
    vec![
        Line::from(Span::styled(
            "Services panel stub — implementation removed",
            Style::default().fg(Color::DarkGray),
        )),
    ]
}
