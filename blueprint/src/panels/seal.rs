use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Modifier};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::panel::Panel;
use crate::state::NeilState;

/// Mood derived from system state
#[derive(Debug, Clone, Copy, PartialEq)]
enum Mood {
    Happy,      // all good, low beats
    Working,    // actively processing
    Tired,      // high beat count
    Alert,      // failures exist
    Sleeping,   // quiet hours (23-07)
}

impl Mood {
    fn from_state(s: &NeilState) -> Self {
        let hour = s.now.format("%H").to_string().parse::<u8>().unwrap_or(12);
        if hour >= 23 || hour < 7 {
            return Mood::Sleeping;
        }
        if !s.failures.iter().filter(|f| f.resolution == "pending").collect::<Vec<_>>().is_empty() {
            return Mood::Alert;
        }
        if s.heartbeat.beats_today > 35 {
            return Mood::Tired;
        }
        if s.system.queue_count > 0 {
            return Mood::Working;
        }
        Mood::Happy
    }

    fn label(&self) -> &str {
        match self {
            Mood::Happy => "happy",
            Mood::Working => "working",
            Mood::Tired => "tired",
            Mood::Alert => "alert!",
            Mood::Sleeping => "sleeping",
        }
    }

    fn color(&self) -> Color {
        match self {
            Mood::Happy => Color::Green,
            Mood::Working => Color::Cyan,
            Mood::Tired => Color::Yellow,
            Mood::Alert => Color::Red,
            Mood::Sleeping => Color::Blue,
        }
    }

    fn eyes(&self) -> &str {
        match self {
            Mood::Happy => "^  ^",
            Mood::Working => "o  o",
            Mood::Tired => "-  -",
            Mood::Alert => "O  O",
            Mood::Sleeping => "-  -",
        }
    }

    fn mouth(&self) -> &str {
        match self {
            Mood::Happy => " w ",
            Mood::Working => " . ",
            Mood::Tired => " ~ ",
            Mood::Alert => " ! ",
            Mood::Sleeping => " z ",
        }
    }
}

/// Generate seal ASCII art lines based on mood
fn seal_art(mood: &Mood) -> Vec<&'static str> {
    match mood {
        Mood::Sleeping => vec![
            r"        _____      ",
            r"      /       \    ",
            r"     |  -   -  |   ",
            r"     |    z    |   ",
            r"      \ .---. /    ",
            r"       '-----'     ",
            r"      /|     |\    ",
            r"     / |     | \   ",
            r"  ~~~~~~~~~~~~~~~~~~",
            r"      z z z        ",
        ],
        Mood::Alert => vec![
            r"        _____      ",
            r"      /       \    ",
            r"     |  O   O  |   ",
            r"     |    !    |   ",
            r"      \ .---. /    ",
            r"       '-----'     ",
            r"      /|     |\    ",
            r"     / |     | \   ",
            r"  ~~~~~~~~~~~~~~~~~~",
            r"         !!        ",
        ],
        _ => vec![
            r"        _____      ",
            r"      /       \    ",
            &"",  // eyes placeholder
            &"",  // mouth placeholder
            r"      \ .---. /    ",
            r"       '-----'     ",
            r"      /|     |\    ",
            r"     / |     | \   ",
            r"  ~~~~~~~~~~~~~~~~~~",
            r"                   ",
        ],
    }
}

pub struct SealPanel;

impl Panel for SealPanel {
    fn id(&self) -> &str { "seal" }
    fn title(&self) -> &str { "Neil" }
    fn priority(&self) -> u8 { 3 }

    fn render(&self, area: Rect, buf: &mut Buffer, state: &NeilState) {
        let mood = Mood::from_state(state);
        let mc = mood.color();

        let mut lines: Vec<Line> = Vec::new();

        // Title
        lines.push(Line::from(Span::styled(
            "  🦭 NEIL THE SEAL",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        // Seal art (hardcoded for each mood to avoid lifetime issues)
        let art_lines: Vec<String> = match mood {
            Mood::Sleeping => vec![
                "        _____      ".into(),
                "      /       \\    ".into(),
                "     |  -   -  |   ".into(),
                "     |    z    |   ".into(),
                "      \\ .---. /    ".into(),
                "       '-----'     ".into(),
                "      /|     |\\    ".into(),
                "     / |     | \\   ".into(),
                "  ~~~~~~~~~~~~~~~~~~".into(),
                "      z z z        ".into(),
            ],
            Mood::Alert => vec![
                "        _____      ".into(),
                "      /       \\    ".into(),
                "     |  O   O  |   ".into(),
                "     |    !    |   ".into(),
                "      \\ .---. /    ".into(),
                "       '-----'     ".into(),
                "      /|     |\\    ".into(),
                "     / |     | \\   ".into(),
                "  ~~~~~~~~~~~~~~~~~~".into(),
                "         !!        ".into(),
            ],
            _ => vec![
                "        _____      ".into(),
                "      /       \\    ".into(),
                format!("     |  {}  |   ", mood.eyes()),
                format!("     |   {}   |   ", mood.mouth()),
                "      \\ .---. /    ".into(),
                "       '-----'     ".into(),
                "      /|     |\\    ".into(),
                "     / |     | \\   ".into(),
                "  ~~~~~~~~~~~~~~~~~~".into(),
                "                   ".into(),
            ],
        };

        for a in &art_lines {
            lines.push(Line::from(Span::styled(
                format!("  {}", a),
                Style::default().fg(mc),
            )));
        }

        lines.push(Line::from(""));

        // Mood
        lines.push(Line::from(vec![
            Span::styled("  mood: ", Style::default().fg(Color::DarkGray)),
            Span::styled(mood.label(), Style::default().fg(mc).add_modifier(Modifier::BOLD)),
        ]));

        // Consciousness - simple awareness indicator
        let consciousness = format!(
            "{}b/50 | {}n | {}w",
            state.heartbeat.beats_today,
            state.palace.total_notes,
            state.palace.wings.len(),
        );
        lines.push(Line::from(vec![
            Span::styled("  mind: ", Style::default().fg(Color::DarkGray)),
            Span::styled(consciousness, Style::default().fg(Color::White)),
        ]));

        // Uptime / last beat
        if !state.heartbeat.last_beat.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("  last: ", Style::default().fg(Color::DarkGray)),
                Span::styled(state.heartbeat.last_beat.clone(), Style::default().fg(Color::DarkGray)),
            ]));
        }

        // Pending work
        let pending: usize = state.intentions.iter().filter(|i| i.status == "pending").count();
        let unresolved: usize = state.failures.iter().filter(|f| f.resolution == "pending").count();
        if pending > 0 || unresolved > 0 {
            lines.push(Line::from(""));
            if pending > 0 {
                lines.push(Line::from(Span::styled(
                    format!("  {} pending intentions", pending),
                    Style::default().fg(Color::Yellow),
                )));
            }
            if unresolved > 0 {
                lines.push(Line::from(Span::styled(
                    format!("  {} unresolved failures", unresolved),
                    Style::default().fg(Color::Red),
                )));
            }
        }

        let paragraph = ratatui::widgets::Paragraph::new(lines);
        paragraph.render(area, buf);
    }

    fn compact(&self, state: &NeilState) -> String {
        let mood = Mood::from_state(state);
        format!("🦭 {}", mood.label())
    }
}
