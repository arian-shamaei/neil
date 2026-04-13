mod panel;
mod state;
mod panels;
mod stream;

use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style, Modifier};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Clear};
use ratatui::Terminal;

use state::NeilState;
use stream::{StreamEntry, EntryKind, RichBlock};

#[derive(Debug, Clone, PartialEq)]
enum View {
    Chat,
    PanelSelector,
    Panel(usize), // index into PANELS
}

const PANEL_NAMES: &[(&str, &str)] = &[
    ("Memory", "Browse wings, rooms, and notes"),
    ("Heartbeat", "Timeline of heartbeat activity"),
    ("Intentions", "Task board with priorities"),
    ("System", "Architecture and service status"),
    ("Services", "Registered APIs and plugins"),
    ("Failures", "Unresolved errors and lessons"),
    ("Logs", "Raw history browser"),
];

fn main() -> anyhow::Result<()> {
    let neil_home = env::var("NEIL_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(home).join(".neil")
        });

    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, crossterm::event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let queue_dir = neil_home.join("tools/autoPrompter/queue");
    let history_dir = neil_home.join("tools/autoPrompter/history");

    let mut stream: Vec<StreamEntry> = Vec::new();
    let mut input = String::new();
    let mut scroll_offset: i32 = 0; // 0 = bottom, positive = scrolled up
    let mut auto_scroll = true;
    let mut view = View::Chat;
    let mut panel_selection: usize = 0;
    let mut show_sidebar = true;
    let mut tick: u64 = 0;
    let mut last_history_count: usize = 0;

    stream.push(StreamEntry::new(
        EntryKind::System,
        "Neil is online. Type a message and press Enter. Tab for panels.".into(),
    ));

    // Load recent history
    load_history(&history_dir, &mut stream, &mut last_history_count);

    let tick_rate = Duration::from_millis(500);
    let mut last_tick = Instant::now();

    loop {
        let state = NeilState::load(&neil_home);

        // Check for new results
        check_new_results(&history_dir, &mut stream, &mut last_history_count, &mut auto_scroll);

        if auto_scroll { scroll_offset = 0; }

        terminal.draw(|frame| {
            let size = frame.area();

            match &view {
                View::Chat => {
                    if show_sidebar && size.width > 60 {
                        let h = Layout::default()
                            .direction(Direction::Horizontal)
                            .constraints([Constraint::Min(40), Constraint::Length(28)])
                            .split(size);
                        render_stream(frame, h[0], &stream, &input, scroll_offset);
                        render_sidebar(frame, h[1], &state);
                    } else {
                        render_stream(frame, size, &stream, &input, scroll_offset);
                    }
                }
                View::PanelSelector => {
                    // Render chat dimmed behind
                    render_stream(frame, size, &stream, &input, scroll_offset);
                    // Overlay panel selector
                    render_panel_selector(frame, size, panel_selection);
                }
                View::Panel(idx) => {
                    render_panel_view(frame, size, *idx, &state);
                }
            }
        })?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    match &view {
                        View::Chat => {
                            match key.code {
                                KeyCode::Enter => {
                                    if !input.is_empty() {
                                        let msg = input.clone();
                                        stream.push(StreamEntry::new(EntryKind::Human, msg.clone()));
                                        input.clear();
                                        auto_scroll = true;
                                        scroll_offset = 0;

                                        let ts = chrono::Local::now().format("%Y%m%dT%H%M%S");
                                        let path = queue_dir.join(format!("{}_chat.md", ts));
                                        let _ = fs::write(&path, &msg);
                                        stream.push(StreamEntry::new(EntryKind::System, "thinking...".into()));
                                    }
                                }
                                KeyCode::Tab => { view = View::PanelSelector; }
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
                                KeyCode::Up => {
                                    scroll_offset += 3;
                                    auto_scroll = false;
                                }
                                KeyCode::Down => {
                                    scroll_offset = (scroll_offset - 3).max(0);
                                    if scroll_offset == 0 { auto_scroll = true; }
                                }
                                KeyCode::PageUp => {
                                    scroll_offset += 20;
                                    auto_scroll = false;
                                }
                                KeyCode::PageDown => {
                                    scroll_offset = (scroll_offset - 20).max(0);
                                    if scroll_offset == 0 { auto_scroll = true; }
                                }
                                KeyCode::Home => {
                                    scroll_offset = 9999;
                                    auto_scroll = false;
                                }
                                KeyCode::End => {
                                    scroll_offset = 0;
                                    auto_scroll = true;
                                }
                                KeyCode::Esc => {
                                    if input.is_empty() { break; }
                                    else { input.clear(); }
                                }
                                _ => {}
                            }
                        }
                        View::PanelSelector => {
                            match key.code {
                                KeyCode::Esc | KeyCode::Tab => { view = View::Chat; }
                                KeyCode::Up => {
                                    if panel_selection > 0 { panel_selection -= 1; }
                                }
                                KeyCode::Down => {
                                    if panel_selection < PANEL_NAMES.len() - 1 { panel_selection += 1; }
                                }
                                KeyCode::Enter => {
                                    view = View::Panel(panel_selection);
                                }
                                KeyCode::Char(c) if c.is_ascii_digit() => {
                                    let idx = (c as u8 - b'1') as usize;
                                    if idx < PANEL_NAMES.len() {
                                        view = View::Panel(idx);
                                    }
                                }
                                _ => {}
                            }
                        }
                        View::Panel(_) => {
                            match key.code {
                                KeyCode::Esc | KeyCode::Tab => { view = View::Chat; }
                                KeyCode::Char(c) if c.is_ascii_digit() => {
                                    let idx = (c as u8 - b'1') as usize;
                                    if idx < PANEL_NAMES.len() {
                                        view = View::Panel(idx);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Event::Mouse(mouse) => {
                    match mouse.kind {
                        MouseEventKind::ScrollUp => {
                            scroll_offset += 3;
                            auto_scroll = false;
                        }
                        MouseEventKind::ScrollDown => {
                            scroll_offset = (scroll_offset - 3).max(0);
                            if scroll_offset == 0 { auto_scroll = true; }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        if last_tick.elapsed() >= tick_rate {
            tick += 1;
            last_tick = Instant::now();
        }
    }

    terminal::disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    Ok(())
}

fn load_history(history_dir: &PathBuf, stream: &mut Vec<StreamEntry>, count: &mut usize) {
    if let Ok(entries) = fs::read_dir(history_dir) {
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
        *count = result_files.len();
    }
}

fn check_new_results(
    history_dir: &PathBuf,
    stream: &mut Vec<StreamEntry>,
    count: &mut usize,
    auto_scroll: &mut bool,
) {
    if let Ok(entries) = fs::read_dir(history_dir) {
        let result_files: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".result.md"))
            .collect();
        if result_files.len() > *count {
            let mut sorted: Vec<_> = result_files.iter().collect();
            sorted.sort_by_key(|e| e.file_name());
            if let Some(latest) = sorted.last() {
                if let Ok(content) = fs::read_to_string(latest.path()) {
                    let output = extract_between(&content, "## Output\n```\n", "\n```");
                    if let Some(o) = output {
                        if !o.is_empty() {
                            // Remove "thinking..." if present
                            if let Some(last) = stream.last() {
                                if matches!(last.kind, EntryKind::System) {
                                    if last.blocks.first().map(|b| matches!(b, RichBlock::Text(t) if t.contains("thinking"))).unwrap_or(false) {
                                        stream.pop();
                                    }
                                }
                            }
                            stream.push(StreamEntry::new(EntryKind::Neil, o));
                            *auto_scroll = true;
                        }
                    }
                }
            }
            *count = result_files.len();
        }
    }
}

fn render_stream(
    frame: &mut ratatui::Frame,
    area: Rect,
    stream: &[StreamEntry],
    input: &str,
    scroll_offset: i32,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(area);

    let conv_area = chunks[0];
    let mut lines: Vec<Line> = Vec::new();

    for entry in stream.iter() {
        let (prefix, color) = match entry.kind {
            EntryKind::Neil => ("neil", Color::Cyan),
            EntryKind::Human => ("you", Color::Green),
            EntryKind::System => ("sys", Color::DarkGray),
        };

        lines.push(Line::from(vec![
            Span::styled(format!(" {} ", prefix), Style::default().fg(Color::Black).bg(color)),
            Span::styled(format!(" {}", entry.time.format("%H:%M")), Style::default().fg(Color::DarkGray)),
        ]));

        for block in &entry.blocks {
            match block {
                RichBlock::Text(t) => {
                    for text_line in t.lines() {
                        let style = if text_line.starts_with("MEMORY:") || text_line.starts_with("CALL:")
                            || text_line.starts_with("NOTIFY:") || text_line.starts_with("HEARTBEAT:")
                            || text_line.starts_with("INTEND:") || text_line.starts_with("DONE:")
                            || text_line.starts_with("FAIL:")
                        {
                            Style::default().fg(Color::Magenta)
                        } else if text_line.starts_with("**") || text_line.starts_with("##") {
                            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::White)
                        };
                        lines.push(Line::from(Span::styled(format!("  {}", text_line), style)));
                    }
                }
                RichBlock::Code { lang, content } => {
                    lines.push(Line::from(Span::styled(
                        format!("  ┌─ {} ─────────────────────", lang),
                        Style::default().fg(Color::DarkGray),
                    )));
                    for code_line in content.lines() {
                        lines.push(Line::from(Span::styled(
                            format!("  │ {}", code_line),
                            Style::default().fg(Color::Yellow),
                        )));
                    }
                    lines.push(Line::from(Span::styled(
                        "  └────────────────────────────",
                        Style::default().fg(Color::DarkGray),
                    )));
                }
                RichBlock::Diagram(d) => {
                    lines.push(Line::from(Span::styled(
                        "  ┌─ diagram ──────────────────",
                        Style::default().fg(Color::Blue),
                    )));
                    for d_line in d.lines() {
                        lines.push(Line::from(Span::styled(
                            format!("  │ {}", d_line),
                            Style::default().fg(Color::Cyan),
                        )));
                    }
                    lines.push(Line::from(Span::styled(
                        "  └────────────────────────────",
                        Style::default().fg(Color::Blue),
                    )));
                }
                RichBlock::Table { headers, rows } => {
                    let header_str = headers.iter()
                        .map(|h| format!("{:<15}", h))
                        .collect::<String>();
                    lines.push(Line::from(Span::styled(
                        format!("  {}", header_str),
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    )));
                    lines.push(Line::from(Span::styled(
                        format!("  {}", "─".repeat(headers.len() * 15)),
                        Style::default().fg(Color::DarkGray),
                    )));
                    for row in rows {
                        let row_str = row.iter()
                            .map(|c| format!("{:<15}", c))
                            .collect::<String>();
                        lines.push(Line::from(Span::styled(
                            format!("  {}", row_str),
                            Style::default().fg(Color::White),
                        )));
                    }
                }
                RichBlock::Chart { title, labels, data } => {
                    if !title.is_empty() {
                        lines.push(Line::from(Span::styled(
                            format!("  {}", title),
                            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                        )));
                    }
                    let max = data.iter().cloned().fold(0.0_f64, f64::max);
                    for (i, val) in data.iter().enumerate() {
                        let label = labels.get(i).map(|s| s.as_str()).unwrap_or("?");
                        let bar_width = if max > 0.0 { (val / max * 20.0) as usize } else { 0 };
                        let bar_str = format!("{}{}", "█".repeat(bar_width), "░".repeat(20 - bar_width));
                        lines.push(Line::from(Span::styled(
                            format!("  {:<5} {} {}", label, bar_str, val),
                            Style::default().fg(Color::Cyan),
                        )));
                    }
                }
            }
        }
        lines.push(Line::from(""));
    }

    // Scroll from bottom
    let total = lines.len() as i32;
    let visible = conv_area.height as i32;
    let max_scroll = (total - visible).max(0);
    let offset = (max_scroll - scroll_offset).max(0) as u16;

    let conversation = Paragraph::new(lines).scroll((offset, 0));
    frame.render_widget(conversation, conv_area);

    // Scroll indicator
    if scroll_offset > 0 {
        let indicator = Span::styled(
            format!(" ↑ {} lines above ", scroll_offset),
            Style::default().fg(Color::Yellow),
        );
        frame.render_widget(
            Paragraph::new(Line::from(indicator)),
            Rect::new(conv_area.x, conv_area.y, conv_area.width, 1),
        );
    }

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

fn render_sidebar(frame: &mut ratatui::Frame, area: Rect, state: &NeilState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(8),
            Constraint::Min(4),
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
    frame.render_widget(
        Paragraph::new(status_lines)
            .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray))),
        chunks[0],
    );

    // Memory
    let mut mem_lines = vec![
        Line::from(Span::styled(format!(" {} notes", state.palace.total_notes), Style::default().fg(Color::Cyan))),
    ];
    for wing in state.palace.wings.iter().take(4) {
        mem_lines.push(Line::from(Span::styled(
            format!("  {}: {}", wing.name, wing.count),
            Style::default().fg(Color::DarkGray),
        )));
    }
    frame.render_widget(
        Paragraph::new(mem_lines)
            .block(Block::default().borders(Borders::ALL).title(" memory ").border_style(Style::default().fg(Color::DarkGray))),
        chunks[1],
    );

    // Intentions
    let pending: Vec<_> = state.intentions.iter().filter(|i| i.status == "pending").collect();
    let mut intent_lines = Vec::new();
    if pending.is_empty() {
        intent_lines.push(Line::from(Span::styled(" (none)", Style::default().fg(Color::DarkGray))));
    } else {
        for i in pending.iter().take(5) {
            let color = match i.priority.as_str() {
                "high" => Color::Red, "medium" => Color::Yellow, _ => Color::Green,
            };
            intent_lines.push(Line::from(vec![
                Span::styled(format!(" [{}] ", i.priority.chars().next().unwrap_or('?')), Style::default().fg(color)),
                Span::styled(i.description.chars().take(18).collect::<String>(), Style::default().fg(Color::DarkGray)),
            ]));
        }
    }
    frame.render_widget(
        Paragraph::new(intent_lines)
            .block(Block::default().borders(Borders::ALL).title(" intents ").border_style(Style::default().fg(Color::DarkGray))),
        chunks[2],
    );
}

fn render_panel_selector(frame: &mut ratatui::Frame, area: Rect, selected: usize) {
    let w = 40.min(area.width - 4);
    let h = (PANEL_NAMES.len() as u16 + 4).min(area.height - 2);
    let x = (area.width - w) / 2;
    let y = (area.height - h) / 2;
    let popup = Rect::new(x, y, w, h);

    frame.render_widget(Clear, popup);

    let mut lines = vec![
        Line::from(Span::styled(" Select a panel:", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
        Line::from(""),
    ];

    for (i, (name, desc)) in PANEL_NAMES.iter().enumerate() {
        let marker = if i == selected { ">" } else { " " };
        let style = if i == selected {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        lines.push(Line::from(Span::styled(
            format!(" {} {}. {}", marker, i + 1, name),
            style,
        )));
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(" panels ")
                .border_style(Style::default().fg(Color::Cyan))),
        popup,
    );
}

fn render_panel_view(frame: &mut ratatui::Frame, area: Rect, idx: usize, state: &NeilState) {
    let (name, _desc) = PANEL_NAMES.get(idx).unwrap_or(&("?", ""));
    let title = format!(" {} | Esc to close, 1-7 to switch ", name);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines: Vec<Line> = match idx {
        0 => render_memory_panel(state),
        1 => render_heartbeat_panel(state),
        2 => render_intentions_panel(state),
        3 => render_system_panel(state),
        4 => render_services_panel(state),
        5 => render_failures_panel(state),
        6 => render_logs_panel(state),
        _ => vec![Line::from("Unknown panel")],
    };

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_memory_panel(state: &NeilState) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(Span::styled(
            format!("Palace: {} notes, {} classified, {} unclassified",
                state.palace.total_notes, state.palace.classified, state.palace.unclassified),
            Style::default().fg(Color::Cyan),
        )),
        Line::from(""),
    ];
    for wing in &state.palace.wings {
        lines.push(Line::from(Span::styled(
            format!("  wing/{} ({} notes)", wing.name, wing.count),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )));
        for (room, count) in &wing.rooms {
            lines.push(Line::from(Span::styled(
                format!("    room/{}: {}", room, count),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }
    lines
}

fn render_heartbeat_panel(state: &NeilState) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(Span::styled(
            format!("Beats today: {}/50 | Last: {}", state.heartbeat.beats_today, state.heartbeat.last_beat),
            Style::default().fg(Color::Cyan),
        )),
        Line::from(""),
    ];
    for entry in &state.heartbeat.entries {
        let color = match entry.status.as_str() {
            "ok" => Color::Green, "acted" => Color::Cyan, "error" => Color::Red, _ => Color::DarkGray,
        };
        lines.push(Line::from(vec![
            Span::styled(format!("  {} ", entry.timestamp), Style::default().fg(Color::DarkGray)),
            Span::styled(format!("[{}] ", entry.status), Style::default().fg(color)),
            Span::styled(entry.summary.clone(), Style::default().fg(Color::White)),
        ]));
    }
    lines
}

fn render_intentions_panel(state: &NeilState) -> Vec<Line<'static>> {
    let pending: Vec<_> = state.intentions.iter().filter(|i| i.status == "pending").collect();
    let completed: Vec<_> = state.intentions.iter().filter(|i| i.status == "completed").collect();

    let mut lines = vec![
        Line::from(Span::styled(
            format!("Pending: {} | Completed: {}", pending.len(), completed.len()),
            Style::default().fg(Color::Cyan),
        )),
        Line::from(""),
        Line::from(Span::styled("  PENDING", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
    ];
    for i in &pending {
        let color = match i.priority.as_str() { "high" => Color::Red, "medium" => Color::Yellow, _ => Color::Green };
        lines.push(Line::from(vec![
            Span::styled(format!("  [{}] ", i.priority), Style::default().fg(color)),
            Span::styled(i.description.clone(), Style::default().fg(Color::White)),
            Span::styled(if i.due.is_empty() { String::new() } else { format!(" (due: {})", i.due) }, Style::default().fg(Color::DarkGray)),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("  COMPLETED", Style::default().fg(Color::DarkGray))));
    for i in completed.iter().rev().take(10) {
        lines.push(Line::from(Span::styled(
            format!("  [done] {}", i.description),
            Style::default().fg(Color::DarkGray),
        )));
    }
    lines
}

fn render_system_panel(state: &NeilState) -> Vec<Line<'static>> {
    vec![
        Line::from(Span::styled("  ESSENCE FILES", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled(
            format!("  {}", state.essence_files.join(", ")),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled("  AUTOPROMPT", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled(
            format!("  status: {}", if state.system.autoprompt_active { "active" } else { "DOWN" }),
            Style::default().fg(if state.system.autoprompt_active { Color::Green } else { Color::Red }),
        )),
        Line::from(Span::styled(
            format!("  queue: {} pending", state.system.queue_count),
            Style::default().fg(Color::DarkGray),
        )),
    ]
}

fn render_services_panel(state: &NeilState) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(Span::styled(
            format!("  {} services registered", state.services.len()),
            Style::default().fg(Color::Cyan),
        )),
        Line::from(""),
    ];
    for svc in &state.services {
        lines.push(Line::from(Span::styled(
            format!("  - {}", svc.trim_end_matches(".md")),
            Style::default().fg(Color::White),
        )));
    }
    lines
}

fn render_failures_panel(state: &NeilState) -> Vec<Line<'static>> {
    let pending: Vec<_> = state.failures.iter().filter(|f| f.resolution == "pending").collect();
    let resolved: Vec<_> = state.failures.iter().filter(|f| f.resolution != "pending").collect();

    let mut lines = vec![
        Line::from(Span::styled(
            format!("  Unresolved: {} | Resolved: {}", pending.len(), resolved.len()),
            Style::default().fg(Color::Cyan),
        )),
        Line::from(""),
    ];
    for f in &pending {
        let color = match f.severity.as_str() { "high" | "critical" => Color::Red, "medium" => Color::Yellow, _ => Color::Green };
        lines.push(Line::from(vec![
            Span::styled(format!("  [{}] ", f.severity), Style::default().fg(color)),
            Span::styled(format!("{}: {}", f.source, f.error), Style::default().fg(Color::White)),
        ]));
    }
    lines
}

fn render_logs_panel(_state: &NeilState) -> Vec<Line<'static>> {
    // Read last 20 lines of neil.log
    let log_path = std::env::var("NEIL_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap_or("/tmp".into())).join(".neil"))
        .join("outputs/neil.log");

    let content = fs::read_to_string(&log_path).unwrap_or_else(|_| "(no logs)".into());
    content.lines().rev().take(30).collect::<Vec<_>>().into_iter().rev()
        .map(|l| Line::from(Span::styled(format!("  {}", l), Style::default().fg(Color::DarkGray))))
        .collect()
}

fn extract_between(content: &str, start: &str, end: &str) -> Option<String> {
    let s = content.find(start)? + start.len();
    let e = content[s..].find(end)? + s;
    Some(content[s..e].trim().to_string())
}
