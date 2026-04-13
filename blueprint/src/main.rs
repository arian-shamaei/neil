mod panel;
mod state;
mod panels;
mod stream;

use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style, Modifier};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};
use ratatui::Terminal;

use state::NeilState;
use stream::{StreamEntry, EntryKind};

fn main() -> anyhow::Result<()> {
    let neil_home = env::var("NEIL_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(home).join(".neil")
        });

    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let queue_dir = neil_home.join("tools/autoPrompter/queue");
    let history_dir = neil_home.join("tools/autoPrompter/history");

    // State
    let mut stream: Vec<StreamEntry> = Vec::new();
    let mut input = String::new();
    let mut scroll: u16 = 0;
    let mut show_sidebar = true;
    let mut tick: u64 = 0;
    let mut last_history_count: usize = 0;

    // Load existing conversation from recent history
    stream.push(StreamEntry::new(
        EntryKind::System,
        "Neil is online. Type a message and press Enter.".into(),
    ));

    // Load last few results as conversation history
    if let Ok(mut entries) = fs::read_dir(&history_dir) {
        let mut result_files: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".result.md"))
            .collect();
        result_files.sort_by_key(|e| e.file_name());
        let recent = if result_files.len() > 5 {
            &result_files[result_files.len()-5..]
        } else {
            &result_files[..]
        };

        for entry in recent {
            if let Ok(content) = fs::read_to_string(entry.path()) {
                // Extract prompt and output
                let prompt = extract_between(&content, "## Prompt\n```\n", "\n```");
                let output = extract_between(&content, "## Output\n```\n", "\n```");

                if let Some(p) = prompt {
                    if !p.starts_with("# Heartbeat") && !p.starts_with("# Wake Up") {
                        stream.push(StreamEntry::new(EntryKind::Human, p));
                    }
                }
                if let Some(o) = output {
                    if !o.is_empty() {
                        stream.push(StreamEntry::new(EntryKind::Neil, o));
                    }
                }
            }
        }
        last_history_count = result_files.len();
    }

    let tick_rate = Duration::from_millis(500);
    let mut last_tick = Instant::now();

    loop {
        let state = NeilState::load(&neil_home);

        // Check for new results
        if let Ok(entries) = fs::read_dir(&history_dir) {
            let result_files: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().ends_with(".result.md"))
                .collect();
            if result_files.len() > last_history_count {
                // New result! Load it
                let mut sorted: Vec<_> = result_files.iter().collect();
                sorted.sort_by_key(|e| e.file_name());
                if let Some(latest) = sorted.last() {
                    if let Ok(content) = fs::read_to_string(latest.path()) {
                        let output = extract_between(&content, "## Output\n```\n", "\n```");
                        if let Some(o) = output {
                            if !o.is_empty() {
                                stream.push(StreamEntry::new(EntryKind::Neil, o));
                                // Auto-scroll to bottom
                                scroll = 0;
                            }
                        }
                    }
                }
                last_history_count = result_files.len();
            }
        }

        // Render
        terminal.draw(|frame| {
            let size = frame.area();

            if show_sidebar && size.width > 60 {
                // Layout: stream | sidebar
                let h_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Min(40),
                        Constraint::Length(28),
                    ])
                    .split(size);

                render_stream(frame, h_chunks[0], &stream, &input, scroll);
                render_sidebar(frame, h_chunks[1], &state);
            } else {
                render_stream(frame, size, &stream, &input, scroll);
            }
        })?;

        // Handle input
        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Enter => {
                            if !input.is_empty() {
                                let msg = input.clone();
                                stream.push(StreamEntry::new(EntryKind::Human, msg.clone()));
                                input.clear();
                                scroll = 0;

                                // Write to queue
                                let ts = chrono::Local::now().format("%Y%m%dT%H%M%S");
                                let path = queue_dir.join(format!("{}_chat.md", ts));
                                let _ = fs::write(&path, &msg);

                                stream.push(StreamEntry::new(
                                    EntryKind::System,
                                    "thinking...".into(),
                                ));
                            }
                        }
                        KeyCode::Char(c) => {
                            if key.modifiers.contains(KeyModifiers::CONTROL) {
                                match c {
                                    'c' | 'q' => break,
                                    's' => show_sidebar = !show_sidebar,
                                    _ => {}
                                }
                            } else {
                                input.push(c);
                            }
                        }
                        KeyCode::Backspace => { input.pop(); }
                        KeyCode::Up => { scroll = scroll.saturating_add(1); }
                        KeyCode::Down => { scroll = scroll.saturating_sub(1); }
                        KeyCode::Esc => {
                            if input.is_empty() {
                                break;
                            } else {
                                input.clear();
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            tick += 1;
            last_tick = Instant::now();
        }
    }

    terminal::disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

fn render_stream(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    stream: &[StreamEntry],
    input: &str,
    scroll: u16,
) {
    // Split: conversation area | input bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(3),
        ])
        .split(area);

    // Conversation
    let conv_area = chunks[0];
    let mut lines: Vec<Line> = Vec::new();

    for entry in stream.iter() {
        let (prefix, color) = match entry.kind {
            EntryKind::Neil => ("neil", Color::Cyan),
            EntryKind::Human => ("you", Color::Green),
            EntryKind::System => ("sys", Color::DarkGray),
        };

        lines.push(Line::from(vec![
            Span::styled(
                format!(" {} ", prefix),
                Style::default().fg(Color::Black).bg(color),
            ),
            Span::styled(
                format!(" {}", entry.time.format("%H:%M")),
                Style::default().fg(Color::DarkGray),
            ),
        ]));

        // Wrap content lines
        for text_line in entry.content.lines() {
            // Detect code blocks, diagrams, etc.
            let style = if text_line.starts_with("  ") || text_line.starts_with("```")
                || text_line.starts_with("    ") || text_line.contains("───")
                || text_line.contains("│") || text_line.contains("┌")
            {
                Style::default().fg(Color::Yellow)
            } else if text_line.starts_with("MEMORY:") || text_line.starts_with("CALL:")
                || text_line.starts_with("NOTIFY:") || text_line.starts_with("HEARTBEAT:")
            {
                Style::default().fg(Color::Magenta)
            } else {
                Style::default().fg(Color::White)
            };

            lines.push(Line::from(Span::styled(
                format!("  {}", text_line),
                style,
            )));
        }
        lines.push(Line::from(""));
    }

    // Apply scroll from bottom
    let total = lines.len() as u16;
    let visible = conv_area.height;
    let max_scroll = total.saturating_sub(visible);
    let offset = max_scroll.saturating_sub(scroll);

    let conversation = Paragraph::new(lines)
        .scroll((offset, 0))
        .block(Block::default().borders(Borders::NONE));
    frame.render_widget(conversation, conv_area);

    // Input bar
    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" > ");

    let input_text = Paragraph::new(Line::from(vec![
        Span::styled(input, Style::default().fg(Color::White)),
        Span::styled("_", Style::default().fg(Color::Cyan).add_modifier(Modifier::SLOW_BLINK)),
    ]))
    .block(input_block);

    frame.render_widget(input_text, chunks[1]);
}

fn render_sidebar(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    state: &NeilState,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),   // status
            Constraint::Length(8),   // memory
            Constraint::Min(4),     // intentions
        ])
        .split(area);

    // Status
    let status_lines = vec![
        Line::from(Span::styled(" NEIL ", Style::default().fg(Color::Black).bg(Color::Cyan))),
        Line::from(Span::styled(
            format!(" beats: {}/50", state.heartbeat.beats_today),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            format!(" queue: {}", state.system.queue_count),
            Style::default().fg(if state.system.queue_count > 0 { Color::Yellow } else { Color::DarkGray }),
        )),
        Line::from(Span::styled(
            format!(" notes: {}", state.palace.total_notes),
            Style::default().fg(Color::Cyan),
        )),
    ];
    let status = Paragraph::new(status_lines)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(status, chunks[0]);

    // Memory
    let mut mem_lines = vec![
        Line::from(Span::styled(
            format!(" {} notes", state.palace.total_notes),
            Style::default().fg(Color::Cyan),
        )),
    ];
    for wing in state.palace.wings.iter().take(4) {
        mem_lines.push(Line::from(Span::styled(
            format!("  {}: {}", wing.name, wing.count),
            Style::default().fg(Color::DarkGray),
        )));
    }
    let memory = Paragraph::new(mem_lines)
        .block(Block::default().borders(Borders::ALL).title(" memory ")
            .border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(memory, chunks[1]);

    // Intentions
    let pending: Vec<_> = state.intentions.iter()
        .filter(|i| i.status == "pending")
        .collect();
    let mut intent_lines = Vec::new();
    if pending.is_empty() {
        intent_lines.push(Line::from(Span::styled(
            " (none)", Style::default().fg(Color::DarkGray),
        )));
    } else {
        for i in pending.iter().take(5) {
            let color = match i.priority.as_str() {
                "high" => Color::Red,
                "medium" => Color::Yellow,
                _ => Color::Green,
            };
            intent_lines.push(Line::from(vec![
                Span::styled(format!(" [{}] ", i.priority.chars().next().unwrap_or('?')),
                    Style::default().fg(color)),
                Span::styled(
                    i.description.chars().take(20).collect::<String>(),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    }
    let intentions = Paragraph::new(intent_lines)
        .block(Block::default().borders(Borders::ALL).title(" intents ")
            .border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(intentions, chunks[2]);
}

fn extract_between(content: &str, start: &str, end: &str) -> Option<String> {
    let s = content.find(start)? + start.len();
    let e = content[s..].find(end)? + s;
    Some(content[s..e].trim().to_string())
}
