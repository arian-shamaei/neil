use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Modifier};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use std::fs;

use crate::panel::Panel;
use crate::state::NeilState;

/// Mood derived from system state
#[derive(Debug, Clone, Copy, PartialEq)]
enum Mood {
    Happy,      // all good, low beats
    Working,    // actively processing
    Curious,    // idle, exploring
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
        if !s.failures.iter().any(|f| f.resolution == "pending") {
            // no failures -- check other conditions
        } else {
            return Mood::Alert;
        }
        if s.heartbeat.beats_today > 35 {
            return Mood::Tired;
        }
        if s.system.queue_count > 0 {
            return Mood::Working;
        }
        // Idle -- alternate between happy and curious based on tick
        if s.tick % 4 == 0 {
            Mood::Curious
        } else {
            Mood::Happy
        }
    }

    fn label(&self) -> &str {
        match self {
            Mood::Happy => "happy",
            Mood::Working => "working",
            Mood::Curious => "curious",
            Mood::Tired => "tired",
            Mood::Alert => "alert!",
            Mood::Sleeping => "sleeping",
        }
    }

    fn color(&self) -> Color {
        match self {
            Mood::Happy => Color::Green,
            Mood::Working => Color::Cyan,
            Mood::Curious => Color::Magenta,
            Mood::Tired => Color::Yellow,
            Mood::Alert => Color::Red,
            Mood::Sleeping => Color::Blue,
        }
    }

    fn art_file(&self) -> &str {
        match self {
            Mood::Happy => "happy.txt",
            Mood::Working => "working.txt",
            Mood::Curious => "curious.txt",
            Mood::Tired => "working.txt",
            Mood::Alert => "stressed.txt",
            Mood::Sleeping => "sleeping.txt",
        }
    }

    fn emoji(&self) -> &str {
        match self {
            Mood::Happy => ":)",
            Mood::Working => "o.o",
            Mood::Curious => ":?",
            Mood::Tired => "~.~",
            Mood::Alert => "O_O",
            Mood::Sleeping => "z.z",
        }
    }
}

/// Read seal art from art/ file, with inline fallback
fn load_art(neil_home: &std::path::PathBuf, mood: &Mood) -> Vec<String> {
    let art_path = neil_home.join("blueprint/art").join(mood.art_file());
    if let Ok(content) = fs::read_to_string(&art_path) {
        content.lines().map(|l| l.to_string()).collect()
    } else {
        // Fallback: simple ASCII seal
        vec![
            "      _____      ".into(),
            "    /       \\    ".into(),
            format!("   |  {}  |   ", mood.emoji()),
            "    \\ .---. /    ".into(),
            "     '-----'     ".into(),
            "    /|     |\\    ".into(),
            "~~~~~~~~~~~~~~~~~~".into(),
        ]
    }
}

/// Build a beat budget bar: [========--] 10/50
fn beat_bar(beats: usize, max: usize, width: usize) -> String {
    let filled = if max > 0 { (beats * width) / max } else { 0 };
    let filled = filled.min(width);
    let empty = width.saturating_sub(filled);
    format!(
        "[{}{}] {}/{}",
        "=".repeat(filled),
        "-".repeat(empty),
        beats,
        max,
    )
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

        // Seal art from art/ files
        let art = load_art(&state.neil_home, &mood);
        for a in &art {
            lines.push(Line::from(Span::styled(
                format!("  {}", a),
                Style::default().fg(mc),
            )));
        }

        lines.push(Line::from(""));

        // Mood line
        lines.push(Line::from(vec![
            Span::styled("  mood: ", Style::default().fg(Color::DarkGray)),
            Span::styled(mood.label(), Style::default().fg(mc).add_modifier(Modifier::BOLD)),
        ]));

        // Consciousness section
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  -- consciousness --",
            Style::default().fg(Color::DarkGray),
        )));

        // Beat count (with budget bar only if a cap is set)
        match state.max_daily_beats {
            Some(cap) => {
                let bar = beat_bar(state.heartbeat.beats_today, cap, 14);
                lines.push(Line::from(vec![
                    Span::styled("  beats: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(bar, Style::default().fg(
                        if state.heartbeat.beats_today > cap * 4 / 5 { Color::Red }
                        else if state.heartbeat.beats_today > cap / 2 { Color::Yellow }
                        else { Color::Green }
                    )),
                ]));
            }
            None => {
                lines.push(Line::from(vec![
                    Span::styled("  beats: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{} today", state.heartbeat.beats_today),
                        Style::default().fg(Color::Green),
                    ),
                ]));
            }
        }

        // Memory
        lines.push(Line::from(vec![
            Span::styled("  notes: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} across {} wings", state.palace.total_notes, state.palace.wings.len()),
                Style::default().fg(Color::White),
            ),
        ]));

        // Wing breakdown (compact)
        if !state.palace.wings.is_empty() {
            let wing_summary: String = state.palace.wings.iter()
                .take(3)
                .map(|w| format!("{}({})", w.name, w.count))
                .collect::<Vec<_>>()
                .join(" ");
            lines.push(Line::from(vec![
                Span::styled("  wings: ", Style::default().fg(Color::DarkGray)),
                Span::styled(wing_summary, Style::default().fg(Color::DarkGray)),
            ]));
        }

        // Last thought (last heartbeat summary, truncated)
        if let Some(last) = state.heartbeat.entries.last() {
            if !last.summary.is_empty() {
                let thought = if last.summary.len() > 40 {
                    format!("{}...", &last.summary[..40])
                } else {
                    last.summary.clone()
                };
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled("  last thought: ", Style::default().fg(Color::DarkGray)),
                ]));
                lines.push(Line::from(Span::styled(
                    format!("  \"{}\"", thought),
                    Style::default().fg(Color::White).add_modifier(Modifier::ITALIC),
                )));
            }
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
        format!("🦭 {} | {}b | {}n", mood.label(), state.heartbeat.beats_today, state.palace.total_notes)
    }
}
