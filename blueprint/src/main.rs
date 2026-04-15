mod panel;
mod state;
mod panels;
mod stream;
mod awareness;
mod seal;

use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style, Modifier};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap, Clear};
use ratatui::Terminal;

use state::NeilState;
use stream::{StreamEntry, EntryKind, RichBlock};

#[derive(Debug, Clone, PartialEq)]
enum View {
    Chat,
    PanelSelector,
    Panel(usize),
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

struct FpsTracker {
    frames: u32,
    last_second: Instant,
    fps: u32,
}

impl FpsTracker {
    fn new() -> Self {
        Self { frames: 0, last_second: Instant::now(), fps: 0 }
    }
    fn tick(&mut self) {
        self.frames += 1;
        if self.last_second.elapsed() >= Duration::from_secs(1) {
            self.fps = self.frames;
            self.frames = 0;
            self.last_second = Instant::now();
        }
    }
}

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
    // Mouse capture OFF by default -- native text selection works
    // Ctrl+M enables mouse capture for scroll wheel support
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let queue_dir = neil_home.join("tools/autoPrompter/queue");
    let history_dir = neil_home.join("tools/autoPrompter/history");

    let mut stream: Vec<StreamEntry> = Vec::new();
    let mut input = String::new();
    let mut cursor_pos: usize = 0;
    let mut scroll_offset: i32 = 0;
    let mut auto_scroll = true;
    let mut view = View::Chat;
    let mut panel_selection: usize = 0;
    let mut show_sidebar = true;
    let mut mouse_captured = false; // default: text selection enabled
    let mut tick: u64 = 0;
    let mut last_history_count: usize = 0;
    let mut last_input_time = Instant::now();
    let mut last_stream_len: usize = 0;
    let mut stream_active = false;
    let mut live_entry_idx: Option<usize> = None;
    let mut skip_next_result = false; // prevents duplicate after stream finishes
    let mut fps = FpsTracker::new();

    // Cache: only reload state on timed intervals
    let mut cached_state: Option<NeilState> = None;

    // Time-gated I/O -- replaces tick-based modulo checks
    let mut last_state_reload = Instant::now() - Duration::from_secs(10); // force first load
    let mut last_stream_check = Instant::now();
    let mut last_results_check = Instant::now();
    let mut last_awareness_write = Instant::now();

    // Seal rendering cache (expensive braille grid math)
    let mut cached_seal_lines: Vec<String> = Vec::new();
    let mut cached_seal_tick: u64 = u64::MAX; // force first render
    let mut cached_pose = seal::SealPose::default();

    // Stream line cache -- only rebuild when stream content changes
    let mut cached_chat_lines: Vec<Line<'static>> = Vec::new();
    let mut cached_chat_stream_len: usize = 0;
    let mut cached_chat_content_hint: usize = 0; // tracks in-place edits (streaming)
    let mut cached_chat_wrap_width: usize = 0;
    // Reserved for future sidebar caching

    stream.push(StreamEntry::new(
        EntryKind::System,
        "Neil is online. Type a message and press Enter. Tab for panels.".into(),
    ));
    load_history(&history_dir, &mut stream, &mut last_history_count);

    // Target ~30 FPS for smooth rendering, but only poll files slowly
    // render_rate removed: now using event-driven + 100ms animation timer
    let mut last_render = Instant::now();
    let mut needs_redraw = true;

    loop {
        // Reload state from disk every 5 seconds (time-gated, not tick-gated)
        if last_state_reload.elapsed() >= Duration::from_secs(5) || cached_state.is_none() {
            cached_state = Some(NeilState::load(&neil_home));
            cached_pose = seal::SealPose::load(&cached_state.as_ref().unwrap().neil_home);
            last_state_reload = Instant::now();
            needs_redraw = true;
        }
        // Update tick on state for animations (tick only increments on renders)
        if let Some(ref mut s) = cached_state {
            s.tick = tick;
        }
        let state = cached_state.as_ref().unwrap();

        // Tail stream file -- check every 100ms (not every frame)
        if last_stream_check.elapsed() >= Duration::from_millis(100) {
            last_stream_check = Instant::now();
            let stream_path = neil_home.join(".neil_stream");
            if let Ok(content) = fs::read_to_string(&stream_path) {
                if let Some(nl) = content.find('\n') {
                    let header = &content[..nl];
                    let body = &content[nl+1..];

                    let is_running = header.contains("\"running\"");
                    let is_done = body.contains("{\"status\":\"done\"");

                    let display_body = if let Some(done_pos) = body.rfind("\n{\"status\":\"done\"") {
                        &body[..done_pos]
                    } else {
                        body
                    };

                    if is_running && display_body.len() > last_stream_len {
                        if let Some(idx) = live_entry_idx {
                            if idx < stream.len() {
                                stream[idx] = StreamEntry::new(
                                    EntryKind::Neil,
                                    display_body.to_string(),
                                );
                            }
                        } else {
                            if let Some(last) = stream.last() {
                                if matches!(last.kind, EntryKind::System) {
                                    if last.blocks.first().map(|b| matches!(b, RichBlock::Text(t) if t.contains("thinking"))).unwrap_or(false) {
                                        stream.pop();
                                    }
                                }
                            }
                            stream.push(StreamEntry::new(
                                EntryKind::Neil,
                                display_body.to_string(),
                            ));
                            live_entry_idx = Some(stream.len() - 1);
                        }
                        last_stream_len = display_body.len();
                        stream_active = true;
                        if auto_scroll { scroll_offset = 0; }
                        needs_redraw = true;
                    }

                    if is_done && stream_active {
                        stream_active = false;
                        live_entry_idx = None;
                        skip_next_result = true; // result file will have same content
                    }

                    // Detect new stream (different prompt) -- reset len tracker
                    if is_running && !stream_active && display_body.is_empty() {
                        last_stream_len = 0;
                    }
                }
            }
        }

        // Check for new results every 2 seconds
        if last_results_check.elapsed() >= Duration::from_secs(2) && !stream_active {
            last_results_check = Instant::now();
            let prev = last_history_count;

            if skip_next_result {
                // Stream already delivered this response -- just update the file count
                if let Ok(entries) = fs::read_dir(&history_dir) {
                    last_history_count = entries.filter_map(|e| e.ok())
                        .filter(|e| e.file_name().to_string_lossy().ends_with(".result.md"))
                        .count();
                }
                skip_next_result = false;
            } else {
                let prev_len = stream.len();
                check_new_results(&history_dir, &mut stream, &mut last_history_count, &mut auto_scroll);
                // Dedup: if check_new_results added an entry that matches the previous last entry, remove it
                if stream.len() > prev_len && prev_len >= 2 {
                    let new_text = stream.last().and_then(|e| e.blocks.first().map(|b| match b {
                        RichBlock::Text(t) => t.clone(), _ => String::new()
                    })).unwrap_or_default();
                    let prev_text = stream.get(prev_len - 1).and_then(|e| e.blocks.first().map(|b| match b {
                        RichBlock::Text(t) => t.clone(), _ => String::new()
                    })).unwrap_or_default();
                    if !new_text.is_empty() && new_text == prev_text {
                        stream.pop(); // duplicate -- remove
                    }
                }
            }

            if last_history_count != prev {
                needs_redraw = true;
            }
        }

        // Cap stream to prevent unbounded growth
        if stream.len() > 100 {
            let drain = stream.len() - 80;
            stream.drain(..drain);
            // Reset cache since indices shifted
            cached_chat_stream_len = 0;
        }

        if auto_scroll { scroll_offset = 0; }

        // Only redraw when needed or at render rate
        // Use slower animation rate (100ms/10fps) for idle, fast (33ms) when content changes
        let anim_due = last_render.elapsed() >= Duration::from_millis(33);
        if needs_redraw || anim_due {
            fps.tick();

            // Cache seal rendering -- only recompute every ~500ms (every 5 anim ticks at 10fps)
            let seal_anim_tick = tick / 5;
            if seal_anim_tick != cached_seal_tick {
                cached_seal_lines = seal::render_seal(&cached_pose, tick);
                cached_seal_tick = seal_anim_tick;
            }

            // Compute wrap width based on terminal size and sidebar visibility
            let term_w = terminal::size().unwrap_or((80, 24)).0;
            let chat_w = if show_sidebar && term_w > 60 { term_w - 28 } else { term_w };
            let wrap_width = (chat_w as usize).saturating_sub(4);

            // Rebuild stream line cache only when content changed
            let content_hint = stream.last().map(|e| e.total_text_len()).unwrap_or(0);
            if stream.len() != cached_chat_stream_len
                || content_hint != cached_chat_content_hint
                || wrap_width != cached_chat_wrap_width
            {
                cached_chat_lines = build_chat_lines(&stream, wrap_width);
                cached_chat_stream_len = stream.len();
                cached_chat_content_hint = content_hint;
                cached_chat_wrap_width = wrap_width;
            }

            terminal.draw(|frame| {
                let size = frame.area();
                match &view {
                    View::Chat => {
                        if show_sidebar && size.width > 60 {
                            let h = Layout::default()
                                .direction(Direction::Horizontal)
                                .constraints([Constraint::Min(40), Constraint::Length(28)])
                                .split(size);
                            render_stream_cached(frame, h[0], &cached_chat_lines, &input, cursor_pos, scroll_offset, fps.fps, mouse_captured);
                            render_sidebar(frame, h[1], state, &cached_seal_lines);
                        } else {
                            render_stream_cached(frame, size, &cached_chat_lines, &input, cursor_pos, scroll_offset, fps.fps, mouse_captured);
                        }
                    }
                    View::PanelSelector => {
                        if show_sidebar && size.width > 60 {
                            let h = Layout::default()
                                .direction(Direction::Horizontal)
                                .constraints([Constraint::Min(40), Constraint::Length(28)])
                                .split(size);
                            render_stream_cached(frame, h[0], &cached_chat_lines, &input, cursor_pos, scroll_offset, fps.fps, mouse_captured);
                            render_sidebar(frame, h[1], state, &cached_seal_lines);
                        } else {
                            render_stream_cached(frame, size, &cached_chat_lines, &input, cursor_pos, scroll_offset, fps.fps, mouse_captured);
                        }
                        render_panel_selector(frame, size, panel_selection);
                    }
                    View::Panel(idx) => {
                        render_panel_view(frame, size, *idx, state, fps.fps);
                    }
                }
            })?;
            last_render = Instant::now();
            needs_redraw = false;
            tick += 1; // only increment on actual renders for smooth animation timing
        }

        // Poll input with short timeout for responsiveness
        if event::poll(Duration::from_millis(8))? {
            needs_redraw = true;
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    match &view {
                        View::Chat => match key.code {
                            KeyCode::Enter => {
                                if !input.is_empty() {
                                    last_input_time = Instant::now();
                                    let msg = input.clone();
                                    stream.push(StreamEntry::new(EntryKind::Human, msg.clone()));
                                    input.clear();
                                    cursor_pos = 0;
                                    auto_scroll = true;
                                    scroll_offset = 0;
                                    let ts = chrono::Local::now().format("%Y%m%dT%H%M%S");
                                    let path = queue_dir.join(format!("{}_chat.md", ts));
                                    if msg.len() > 50_000 {
                                        stream.push(StreamEntry::new(EntryKind::System,
                                            format!("Message too large ({} chars, max 50000). Truncated.", msg.len())));
                                        let _ = fs::write(&path, &msg[..50_000]);
                                    } else {
                                        let _ = fs::write(&path, &msg);
                                    }
                                    stream.push(StreamEntry::new(EntryKind::System, "thinking...".into()));
                                }
                            }
                            KeyCode::Tab => { view = View::PanelSelector; }
                            KeyCode::Char(c) => {
                                if key.modifiers.contains(KeyModifiers::CONTROL) {
                                    match c {
                                        'c' | 'q' => break,
                                        's' => show_sidebar = !show_sidebar,
                                        'm' => {
                                            mouse_captured = !mouse_captured;
                                            if mouse_captured {
                                                let _ = execute!(io::stdout(), crossterm::event::EnableMouseCapture);
                                                stream.push(StreamEntry::new(EntryKind::System, "Mouse scroll enabled. Ctrl+M for text select.".into()));
                                            } else {
                                                let _ = execute!(io::stdout(), crossterm::event::DisableMouseCapture);
                                                stream.push(StreamEntry::new(EntryKind::System, "Text select enabled. Ctrl+M for mouse scroll.".into()));
                                            }
                                        }
                                        'a' => cursor_pos = 0,
                                        'e' => cursor_pos = input.chars().count(),
                                        'u' => { input.clear(); cursor_pos = 0; }
                                        _ => {}
                                    }
                                } else if input.len() < 4096 {
                                    // cursor_pos is a char index; convert to byte index for insert
                                    let byte_pos = input.char_indices()
                                        .nth(cursor_pos)
                                        .map(|(i, _)| i)
                                        .unwrap_or(input.len());
                                    input.insert(byte_pos, c);
                                    cursor_pos += 1;
                                    last_input_time = Instant::now();
                                }
                            }
                            KeyCode::Backspace => {
                                if cursor_pos > 0 {
                                    cursor_pos -= 1;
                                    let byte_pos = input.char_indices()
                                        .nth(cursor_pos)
                                        .map(|(i, _)| i)
                                        .unwrap_or(input.len());
                                    if byte_pos < input.len() {
                                        input.remove(byte_pos);
                                    }
                                }
                            }
                            KeyCode::Delete => {
                                let char_count = input.chars().count();
                                if cursor_pos < char_count {
                                    let byte_pos = input.char_indices()
                                        .nth(cursor_pos)
                                        .map(|(i, _)| i)
                                        .unwrap_or(input.len());
                                    if byte_pos < input.len() {
                                        input.remove(byte_pos);
                                    }
                                }
                            }
                            KeyCode::Left => { cursor_pos = cursor_pos.saturating_sub(1); }
                            KeyCode::Right => {
                                let char_count = input.chars().count();
                                cursor_pos = (cursor_pos + 1).min(char_count);
                            }
                            KeyCode::Home => {
                                if key.modifiers.contains(KeyModifiers::SHIFT) {
                                    scroll_offset = 9999; auto_scroll = false;
                                } else {
                                    cursor_pos = 0;
                                }
                            }
                            KeyCode::End => {
                                if key.modifiers.contains(KeyModifiers::SHIFT) {
                                    scroll_offset = 0; auto_scroll = true;
                                } else {
                                    cursor_pos = input.chars().count();
                                }
                            }
                            KeyCode::Up => { scroll_offset += 3; auto_scroll = false; }
                            KeyCode::Down => {
                                scroll_offset = (scroll_offset - 3).max(0);
                                if scroll_offset == 0 { auto_scroll = true; }
                            }
                            KeyCode::PageUp => { scroll_offset += 20; auto_scroll = false; }
                            KeyCode::PageDown => {
                                scroll_offset = (scroll_offset - 20).max(0);
                                if scroll_offset == 0 { auto_scroll = true; }
                            }
                            KeyCode::Esc => {
                                if input.is_empty() { break; } else { input.clear(); cursor_pos = 0; }
                            }
                            _ => {}
                        },
                        View::PanelSelector => match key.code {
                            KeyCode::Esc | KeyCode::Tab => { view = View::Chat; }
                            KeyCode::Up => { if panel_selection > 0 { panel_selection -= 1; } }
                            KeyCode::Down => { if panel_selection < PANEL_NAMES.len() - 1 { panel_selection += 1; } }
                            KeyCode::Enter => { view = View::Panel(panel_selection); }
                            KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
                                let idx = (c as u8 - b'1') as usize;
                                if idx < PANEL_NAMES.len() { view = View::Panel(idx); }
                            }
                            _ => {}
                        },
                        View::Panel(_) => match key.code {
                            KeyCode::Esc | KeyCode::Tab => { view = View::Chat; }
                            KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
                                let idx = (c as u8 - b'1') as usize;
                                if idx < PANEL_NAMES.len() { view = View::Panel(idx); }
                            }
                            _ => {}
                        },
                    }
                }
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => { scroll_offset += 3; auto_scroll = false; }
                    MouseEventKind::ScrollDown => {
                        scroll_offset = (scroll_offset - 3).max(0);
                        if scroll_offset == 0 { auto_scroll = true; }
                    }
                    _ => {}
                },
                Event::Resize(_, _) => { needs_redraw = true; }
                _ => {}
            }
        }

        // Write awareness state every 5 seconds (time-gated)
        if last_awareness_write.elapsed() >= Duration::from_secs(5) {
            last_awareness_write = Instant::now();
            let term_size = terminal::size().unwrap_or((80, 24));
            let view_str = match &view {
                View::Chat => "chat".to_string(),
                View::PanelSelector => "panel_selector".to_string(),
                View::Panel(i) => format!("panel:{}", PANEL_NAMES.get(*i).map(|p| p.0).unwrap_or("?")),
            };
            let bp_state = awareness::BlueprintState {
                timestamp: chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
                running: true,
                view: view_str,
                terminal_size: term_size,
                stream_length: stream.len(),
                scroll_offset,
                auto_scroll,
                input_buffer: if input.is_empty() { String::new() } else { format!("({} chars)", input.len()) },
                last_user_message: stream.iter().rev()
                    .find(|e| matches!(e.kind, EntryKind::Human))
                    .and_then(|e| e.blocks.first().map(|b| match b {
                        RichBlock::Text(t) => t.chars().take(80).collect(),
                        _ => String::new(),
                    }))
                    .unwrap_or_default(),
                sidebar_visible: show_sidebar,
                user_active: last_input_time.elapsed() < Duration::from_secs(60),
                last_input_time: if last_input_time.elapsed() < Duration::from_secs(3600) {
                    format!("{}s ago", last_input_time.elapsed().as_secs())
                } else { "inactive".into() },
                streaming: stream_active,
                stream_chars: last_stream_len,
            };
            bp_state.write(&neil_home);
        }
    }

    awareness::BlueprintState::clear(&neil_home);
    terminal::disable_raw_mode()?;
    if mouse_captured {
        execute!(terminal.backend_mut(), LeaveAlternateScreen, crossterm::event::DisableMouseCapture)?;
    } else {
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    }
    Ok(())
}

// ── Text wrapping helper ──

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 { return vec![text.to_string()]; }
    let mut lines = Vec::new();
    for raw_line in text.lines() {
        if raw_line.len() <= width {
            lines.push(raw_line.to_string());
        } else {
            // Word-wrap
            let mut current = String::new();
            for word in raw_line.split_whitespace() {
                if current.is_empty() {
                    current = word.to_string();
                } else if current.len() + 1 + word.len() <= width {
                    current.push(' ');
                    current.push_str(word);
                } else {
                    lines.push(current);
                    current = word.to_string();
                }
            }
            if !current.is_empty() { lines.push(current); }
        }
    }
    if lines.is_empty() { lines.push(String::new()); }
    lines
}

// ── Rendering ──

/// Build the chat stream lines (expensive -- only call when stream changes)
fn build_chat_lines(stream: &[StreamEntry], wrap_width: usize) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let separator = format!("  {}", "─".repeat(wrap_width.saturating_sub(2)));
    let sep_style = Style::default().fg(Color::Rgb(40, 40, 40));

    for entry in stream.iter() {
        let (prefix, color, text_color) = match entry.kind {
            EntryKind::Neil => ("neil", Color::Cyan, Color::White),
            EntryKind::Human => (" you", Color::Green, Color::Green),
            EntryKind::System => (" sys", Color::DarkGray, Color::DarkGray),
        };

        if !lines.is_empty() {
            lines.push(Line::from(Span::styled(separator.clone(), sep_style)));
        }

        lines.push(Line::from(vec![
            Span::styled(format!(" {} ", prefix), Style::default().fg(Color::Black).bg(color)),
            Span::styled(format!(" {}", entry.time.format("%H:%M")), Style::default().fg(Color::DarkGray)),
        ]));

        for block in &entry.blocks {
            match block {
                RichBlock::Text(t) => {
                    for wrapped in wrap_text(t, wrap_width) {
                        let style = if wrapped.starts_with("MEMORY:") || wrapped.starts_with("CALL:")
                            || wrapped.starts_with("NOTIFY:") || wrapped.starts_with("HEARTBEAT:")
                            || wrapped.starts_with("INTEND:") || wrapped.starts_with("DONE:")
                            || wrapped.starts_with("FAIL:") || wrapped.starts_with("SHOW:")
                        {
                            Style::default().fg(Color::Magenta)
                        } else if wrapped.starts_with("**") || wrapped.starts_with("##") {
                            Style::default().fg(text_color).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(text_color)
                        };
                        lines.push(Line::from(Span::styled(format!("  {}", wrapped), style)));
                    }
                }
                RichBlock::Code { lang, content } => {
                    let border_w = wrap_width.saturating_sub(4);
                    lines.push(Line::from(Span::styled(
                        format!("  ┌─ {} {}", lang, "─".repeat(border_w.saturating_sub(lang.len() + 2))),
                        Style::default().fg(Color::DarkGray),
                    )));
                    for cl in content.lines() {
                        lines.push(Line::from(Span::styled(
                            format!("  │ {}", cl), Style::default().fg(Color::Yellow),
                        )));
                    }
                    lines.push(Line::from(Span::styled(
                        format!("  └{}", "─".repeat(border_w)), Style::default().fg(Color::DarkGray),
                    )));
                }
                RichBlock::Diagram(d) => {
                    let border_w = wrap_width.saturating_sub(4);
                    lines.push(Line::from(Span::styled(
                        format!("  ┌─ diagram {}", "─".repeat(border_w.saturating_sub(10))),
                        Style::default().fg(Color::Blue),
                    )));
                    for dl in d.lines() {
                        lines.push(Line::from(Span::styled(
                            format!("  │ {}", dl), Style::default().fg(Color::Cyan),
                        )));
                    }
                    lines.push(Line::from(Span::styled(
                        format!("  └{}", "─".repeat(border_w)), Style::default().fg(Color::Blue),
                    )));
                }
                RichBlock::Table { headers, rows } => {
                    let col_w = wrap_width.saturating_sub(4) / headers.len().max(1);
                    let hdr: String = headers.iter().map(|h| format!("{:<w$}", h, w = col_w)).collect();
                    lines.push(Line::from(Span::styled(
                        format!("  {}", hdr), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    )));
                    lines.push(Line::from(Span::styled(
                        format!("  {}", "─".repeat(col_w * headers.len())), Style::default().fg(Color::DarkGray),
                    )));
                    for row in rows {
                        let r: String = row.iter().map(|c| format!("{:<w$}", c, w = col_w)).collect();
                        lines.push(Line::from(Span::styled(format!("  {}", r), Style::default().fg(Color::White))));
                    }
                }
                RichBlock::Chart { title, labels, data } => {
                    if !title.is_empty() {
                        lines.push(Line::from(Span::styled(
                            format!("  {}", title), Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                        )));
                    }
                    let max = data.iter().cloned().fold(0.0_f64, f64::max);
                    let bar_max = wrap_width.saturating_sub(12);
                    for (i, val) in data.iter().enumerate() {
                        let label = labels.get(i).map(|s| s.as_str()).unwrap_or("?");
                        let bw = if max > 0.0 { (val / max * bar_max as f64) as usize } else { 0 };
                        lines.push(Line::from(Span::styled(
                            format!("  {:<5} {}{} {}", label, "█".repeat(bw), "░".repeat(bar_max - bw), val),
                            Style::default().fg(Color::Cyan),
                        )));
                    }
                }
            }
        }
        lines.push(Line::from(""));
    }
    lines
}

/// Render the stream view using pre-built cached lines (cheap per frame)
fn render_stream_cached(
    frame: &mut ratatui::Frame, area: Rect, cached_lines: &[Line<'static>],
    input: &str, cursor_pos: usize, scroll_offset: i32, fps: u32,
    mouse_captured: bool,
) {
    let wrap_width = (area.width as usize).saturating_sub(4);

    // Dynamic input box: grows with content, min 3 lines, max 8
    let input_char_count = input.chars().count();
    let input_lines = if input.is_empty() { 1 }
        else if input_char_count > 200 { 2 }
        else { wrap_text(input, wrap_width.saturating_sub(2)).len() };
    let input_height = (input_lines as u16 + 2).clamp(3, 8);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(3), Constraint::Length(input_height)])
        .split(area);

    // Header bar
    let time_str = chrono::Local::now().format("%H:%M:%S").to_string();
    let header = Line::from(vec![
        Span::styled(" NEIL ", Style::default().fg(Color::Black).bg(Color::Cyan)),
        Span::styled(format!(" {} ", time_str), Style::default().fg(Color::DarkGray)),
        Span::styled("| Tab:panels Ctrl+S:sidebar Ctrl+M:mouse Esc:quit ", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(header), chunks[0]);

    let conv_area = chunks[1];

    // Scroll using cached lines (no rebuild needed)
    let total = cached_lines.len() as i32;
    let visible = conv_area.height as i32;
    let max_scroll = (total - visible).max(0);
    let offset = (max_scroll - scroll_offset).max(0) as u16;

    let conversation = Paragraph::new(cached_lines.to_vec()).scroll((offset, 0));
    frame.render_widget(conversation, conv_area);

    // Scroll indicator
    if scroll_offset > 0 {
        let ind = format!(" ↑ {} lines above ", scroll_offset);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(ind, Style::default().fg(Color::Yellow)))),
            Rect::new(conv_area.x, conv_area.y, conv_area.width, 1),
        );
    }

    // Input box (dynamic height, word-wrapped)
    let input_area = chunks[2];
    let inner_w = (input_area.width as usize).saturating_sub(4);

    let char_count_total = input.chars().count();
    let display_input = if input.is_empty() {
        vec![Line::from(Span::styled("_", Style::default().fg(Color::Cyan).add_modifier(Modifier::SLOW_BLINK)))]
    } else if char_count_total > 200 {
        let preview: String = input.chars().take(40).collect();
        let lines_est = input.lines().count();
        vec![
            Line::from(vec![
                Span::styled(format!("{}...", preview), Style::default().fg(Color::DarkGray)),
            ]),
            Line::from(vec![
                Span::styled(
                    format!(" [{} chars, ~{} lines] ", char_count_total, lines_est),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled("Enter to send, Esc to clear", Style::default().fg(Color::DarkGray)),
            ]),
        ]
    } else {
        let wrapped = wrap_text(input, inner_w);
        let mut result: Vec<Line> = Vec::new();
        let mut cc: usize = 0;
        for wl in &wrapped {
            let line_char_count = wl.chars().count();
            let line_start = cc;
            let line_end = cc + line_char_count;
            if cursor_pos >= line_start && cursor_pos <= line_end {
                let local_char_pos = cursor_pos - line_start;
                let byte_pos = wl.char_indices()
                    .nth(local_char_pos)
                    .map(|(i, _)| i)
                    .unwrap_or(wl.len());
                let before = &wl[..byte_pos];
                let after = &wl[byte_pos..];
                result.push(Line::from(vec![
                    Span::styled(before.to_string(), Style::default().fg(Color::White)),
                    Span::styled("_", Style::default().fg(Color::Cyan).add_modifier(Modifier::SLOW_BLINK)),
                    Span::styled(after.to_string(), Style::default().fg(Color::White)),
                ]));
            } else {
                result.push(Line::from(Span::styled(wl.clone(), Style::default().fg(Color::White))));
            }
            cc = line_end;
        }
        result
    };

    let input_widget = Paragraph::new(display_input)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" > "));
    frame.render_widget(input_widget, input_area);

    // FPS + mode indicator (bottom right of input area)
    let mode = if mouse_captured { "scroll" } else { "select" };
    let fps_text = format!(" {}fps {} ", fps, mode);
    let fps_len = fps_text.len() as u16;
    let fps_x = input_area.x + input_area.width.saturating_sub(fps_len + 1);
    let fps_y = input_area.y + input_area.height - 1;
    frame.render_widget(
        Paragraph::new(Span::styled(fps_text, Style::default().fg(Color::DarkGray))),
        Rect::new(fps_x, fps_y, fps_len, 1),
    );
}

fn render_sidebar(frame: &mut ratatui::Frame, area: Rect, state: &NeilState, seal_lines_raw: &[String]) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),   // status
            Constraint::Length(8),   // memory
            Constraint::Min(4),     // intents (dynamic)
            Constraint::Length(16),  // seal art (fixed bottom)
        ])
        .split(area);

    let status_lines = vec![
        Line::from(Span::styled(" NEIL ", Style::default().fg(Color::Black).bg(Color::Cyan))),
        Line::from(Span::styled(format!(" beats: {}/50", state.heartbeat.beats_today), Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled(format!(" queue: {}", state.system.queue_count),
            Style::default().fg(if state.system.queue_count > 0 { Color::Yellow } else { Color::DarkGray }))),
        Line::from(Span::styled(format!(" notes: {}", state.palace.total_notes), Style::default().fg(Color::Cyan))),
    ];
    frame.render_widget(
        Paragraph::new(status_lines).block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray))),
        chunks[0],
    );

    let mut mem_lines = vec![
        Line::from(Span::styled(format!(" {} notes", state.palace.total_notes), Style::default().fg(Color::Cyan))),
    ];
    for wing in state.palace.wings.iter().take(4) {
        mem_lines.push(Line::from(Span::styled(format!("  {}: {}", wing.name, wing.count), Style::default().fg(Color::DarkGray))));
    }
    frame.render_widget(
        Paragraph::new(mem_lines).block(Block::default().borders(Borders::ALL).title(" memory ").border_style(Style::default().fg(Color::DarkGray))),
        chunks[1],
    );

    let pending: Vec<_> = state.intentions.iter().filter(|i| i.status == "pending").collect();
    let mut intent_lines = Vec::new();
    if pending.is_empty() {
        intent_lines.push(Line::from(Span::styled(" (none)", Style::default().fg(Color::DarkGray))));
    } else {
        for i in pending.iter().take(5) {
            let color = match i.priority.as_str() { "high" => Color::Red, "medium" => Color::Yellow, _ => Color::Green };
            intent_lines.push(Line::from(vec![
                Span::styled(format!(" [{}] ", i.priority.chars().next().unwrap_or('?')), Style::default().fg(color)),
                Span::styled(i.description.chars().take(18).collect::<String>(), Style::default().fg(Color::DarkGray)),
            ]));
        }
    }
    frame.render_widget(
        Paragraph::new(intent_lines).block(Block::default().borders(Borders::ALL).title(" intents ").border_style(Style::default().fg(Color::DarkGray))),
        chunks[2],
    );

    // Seal art -- uses pre-cached render (computed outside draw closure)
    let mut seal_lines: Vec<Line> = Vec::new();
    for art_line in seal_lines_raw {
        seal_lines.push(Line::from(Span::styled(
            art_line.clone(),
            Style::default().fg(Color::Cyan),
        )));
    }
    frame.render_widget(
        Paragraph::new(seal_lines).block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray))),
        chunks[3],
    );
}

// Seal rendering moved to seal.rs (parameterized engine)

fn render_panel_selector(frame: &mut ratatui::Frame, area: Rect, selected: usize) {
    let w = 40.min(area.width.saturating_sub(4));
    let h = (PANEL_NAMES.len() as u16 + 4).min(area.height.saturating_sub(2));
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 2;
    let popup = Rect::new(x, y, w, h);
    frame.render_widget(Clear, popup);

    let mut lines = vec![
        Line::from(Span::styled(" Select a panel:", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
        Line::from(""),
    ];
    for (i, (name, _)) in PANEL_NAMES.iter().enumerate() {
        let (marker, style) = if i == selected {
            (">", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        } else {
            (" ", Style::default().fg(Color::DarkGray))
        };
        lines.push(Line::from(Span::styled(format!(" {} {}. {}", marker, i + 1, name), style)));
    }
    frame.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(" panels ").border_style(Style::default().fg(Color::Cyan))),
        popup,
    );
}

fn render_panel_view(frame: &mut ratatui::Frame, area: Rect, idx: usize, state: &NeilState, fps: u32) {
    let (name, _) = PANEL_NAMES.get(idx).unwrap_or(&("?", ""));
    let title = format!(" {} | Esc:close 1-7:switch ", name);
    let block = Block::default().borders(Borders::ALL).title(title).border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines: Vec<Line> = match idx {
        0 => render_memory_panel(state),
        1 => render_heartbeat_panel(state),
        2 => render_intentions_panel(state),
        3 => render_system_panel(state),
        4 => render_services_panel(state),
        5 => render_failures_panel(state),
        6 => render_logs_panel(),
        _ => vec![Line::from("Unknown panel")],
    };
    frame.render_widget(Paragraph::new(lines), inner);

    // FPS bottom right
    let fps_text = format!(" {}fps ", fps);
    let fl = fps_text.len() as u16;
    let fx = area.x + area.width.saturating_sub(fl + 1);
    let fy = area.y + area.height - 1;
    frame.render_widget(Paragraph::new(Span::styled(fps_text, Style::default().fg(Color::DarkGray))), Rect::new(fx, fy, fl, 1));
}

// ── Panel renderers ──

fn render_memory_panel(s: &NeilState) -> Vec<Line<'static>> {
    let mut l = vec![
        Line::from(Span::styled(format!("Palace: {} notes, {} classified, {} unclassified", s.palace.total_notes, s.palace.classified, s.palace.unclassified), Style::default().fg(Color::Cyan))),
        Line::from(""),
    ];
    for w in &s.palace.wings {
        l.push(Line::from(Span::styled(format!("  wing/{} ({})", w.name, w.count), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))));
        for (r, c) in &w.rooms { l.push(Line::from(Span::styled(format!("    room/{}: {}", r, c), Style::default().fg(Color::DarkGray)))); }
    }
    l
}

fn render_heartbeat_panel(s: &NeilState) -> Vec<Line<'static>> {
    let mut l = vec![
        Line::from(Span::styled(format!("Beats today: {}/50 | Last: {}", s.heartbeat.beats_today, s.heartbeat.last_beat), Style::default().fg(Color::Cyan))),
        Line::from(""),
    ];
    for e in &s.heartbeat.entries {
        let c = match e.status.as_str() { "ok" => Color::Green, "acted" => Color::Cyan, "error" => Color::Red, _ => Color::DarkGray };
        l.push(Line::from(vec![
            Span::styled(format!("  {} ", e.timestamp), Style::default().fg(Color::DarkGray)),
            Span::styled(format!("[{}] ", e.status), Style::default().fg(c)),
            Span::styled(e.summary.clone(), Style::default().fg(Color::White)),
        ]));
    }
    l
}

fn render_intentions_panel(s: &NeilState) -> Vec<Line<'static>> {
    let p: Vec<_> = s.intentions.iter().filter(|i| i.status == "pending").collect();
    let d: Vec<_> = s.intentions.iter().filter(|i| i.status == "completed").collect();
    let mut l = vec![
        Line::from(Span::styled(format!("Pending: {} | Completed: {}", p.len(), d.len()), Style::default().fg(Color::Cyan))),
        Line::from(""), Line::from(Span::styled("  PENDING", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
    ];
    for i in &p {
        let c = match i.priority.as_str() { "high" => Color::Red, "medium" => Color::Yellow, _ => Color::Green };
        l.push(Line::from(vec![
            Span::styled(format!("  [{}] ", i.priority), Style::default().fg(c)),
            Span::styled(i.description.clone(), Style::default().fg(Color::White)),
        ]));
    }
    l.push(Line::from("")); l.push(Line::from(Span::styled("  COMPLETED", Style::default().fg(Color::DarkGray))));
    for i in d.iter().rev().take(10) { l.push(Line::from(Span::styled(format!("  [done] {}", i.description), Style::default().fg(Color::DarkGray)))); }
    l
}

fn render_system_panel(s: &NeilState) -> Vec<Line<'static>> {
    vec![
        Line::from(Span::styled("  ESSENCE FILES", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled(format!("  {}", s.essence_files.join(", ")), Style::default().fg(Color::DarkGray))),
        Line::from(""),
        Line::from(Span::styled("  AUTOPROMPT", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled(format!("  status: {}", if s.system.autoprompt_active { "active" } else { "DOWN" }),
            Style::default().fg(if s.system.autoprompt_active { Color::Green } else { Color::Red }))),
        Line::from(Span::styled(format!("  queue: {} pending", s.system.queue_count), Style::default().fg(Color::DarkGray))),
    ]
}

fn render_services_panel(s: &NeilState) -> Vec<Line<'static>> {
    let mut l = vec![
        Line::from(Span::styled(format!("  {} services", s.services.len()), Style::default().fg(Color::Cyan))),
        Line::from(""),
    ];
    for svc in &s.services { l.push(Line::from(Span::styled(format!("  - {}", svc.trim_end_matches(".md")), Style::default().fg(Color::White)))); }
    l
}

fn render_failures_panel(s: &NeilState) -> Vec<Line<'static>> {
    let p: Vec<_> = s.failures.iter().filter(|f| f.resolution == "pending").collect();
    let mut l = vec![Line::from(Span::styled(format!("  Unresolved: {}", p.len()), Style::default().fg(Color::Cyan))), Line::from("")];
    for f in &p {
        let c = match f.severity.as_str() { "high" | "critical" => Color::Red, "medium" => Color::Yellow, _ => Color::Green };
        l.push(Line::from(vec![
            Span::styled(format!("  [{}] ", f.severity), Style::default().fg(c)),
            Span::styled(format!("{}: {}", f.source, f.error), Style::default().fg(Color::White)),
        ]));
    }
    l
}

fn render_logs_panel() -> Vec<Line<'static>> {
    let lp = env::var("NEIL_HOME").map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env::var("HOME").unwrap_or("/tmp".into())).join(".neil"))
        .join("outputs/neil.log");
    let content = fs::read_to_string(&lp).unwrap_or_else(|_| "(no logs)".into());
    content.lines().rev().take(30).collect::<Vec<_>>().into_iter().rev()
        .map(|l| Line::from(Span::styled(format!("  {}", l), Style::default().fg(Color::DarkGray))))
        .collect()
}

// ── Helpers ──

fn load_history(hd: &PathBuf, stream: &mut Vec<StreamEntry>, count: &mut usize) {
    if let Ok(entries) = fs::read_dir(hd) {
        let mut rf: Vec<_> = entries.filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".result.md")).collect();
        rf.sort_by_key(|e| e.file_name());
        let recent = if rf.len() > 5 { &rf[rf.len()-5..] } else { &rf[..] };
        for entry in recent {
            if let Ok(c) = fs::read_to_string(entry.path()) {
                let p = extract_between(&c, "## Prompt\n```\n", "\n```");
                let o = extract_between(&c, "## Output\n```\n", "\n```");
                if let Some(p) = p { if !p.starts_with("# Heartbeat") && !p.starts_with("# Wake Up") { stream.push(StreamEntry::new(EntryKind::Human, p)); } }
                if let Some(o) = o { if !o.is_empty() { stream.push(StreamEntry::new(EntryKind::Neil, o)); } }
            }
        }
        *count = rf.len();
    }
}

fn check_new_results(hd: &PathBuf, stream: &mut Vec<StreamEntry>, count: &mut usize, auto_scroll: &mut bool) {
    if let Ok(entries) = fs::read_dir(hd) {
        let rf: Vec<_> = entries.filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".result.md")).collect();
        if rf.len() > *count {
            let mut sorted: Vec<_> = rf.iter().collect();
            sorted.sort_by_key(|e| e.file_name());
            if let Some(latest) = sorted.last() {
                if let Ok(c) = fs::read_to_string(latest.path()) {
                    if let Some(o) = extract_between(&c, "## Output\n```\n", "\n```") {
                        if !o.is_empty() {
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
            *count = rf.len();
        }
    }
}

fn extract_between(c: &str, start: &str, end: &str) -> Option<String> {
    let s = c.find(start)? + start.len();
    let e = c[s..].find(end)? + s;
    Some(c[s..e].trim().to_string())
}
