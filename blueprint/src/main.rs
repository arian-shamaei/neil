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

use state::{NeilState, CommandLogEntry, load_command_log};
use stream::{StreamEntry, EntryKind, RichBlock, DiffLine};

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
    ("Cluster", "Live Neil instances and their connections"),
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
    // Mouse capture ON by default for scroll wheel
    // Shift+click for text selection in most terminals
    // Ctrl+M toggles off if needed
    execute!(stdout, crossterm::event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let queue_dir = neil_home.join("tools/autoPrompter/queue");
    let history_dir = neil_home.join("tools/autoPrompter/history");

    let mut stream: Vec<StreamEntry> = Vec::new();
    let mut activity: Vec<String> = Vec::new(); // heartbeat/system activity (shown in sidebar)
    let mut input = String::new();
    let mut cursor_pos: usize = 0;
    let mut scroll_offset: i32 = 0;
    let mut auto_scroll = true;
    let mut view = View::Chat;
    let mut panel_selection: usize = 0;
    let mut show_sidebar = true;
    let mut mouse_captured = true; // default: scroll enabled, Shift+click for text select
    let mut tick: u64 = 0;
    let mut last_history_count: usize = 0;
    let mut last_input_time = Instant::now();
    let mut last_stream_len: usize = 0;
    let mut stream_active = false;
    let mut live_entry_idx: Option<usize> = None;
    let mut skip_next_result = false;
    let mut prompt_pending = false; // true between submit and stream_active
    let mut prompt_history: Vec<String> = Vec::new();
    let mut history_idx: Option<usize> = None;
    let mut saved_input: String = String::new();
    let mut hb_selection: usize = 0; // selected heartbeat in panel
    let mut hb_expanded: bool = false; // whether detail view is open
    let mut hb_scroll: usize = 0; // scroll offset in content pane
    let mut hb_section: usize = 0; // selected section in expanded view
    let mut cluster_selection: usize = 0; // selected instance in Cluster panel
    let mut cluster_expanded: bool = false; // detail view open
    let mut cluster_scroll: usize = 0;

    // Pre-load prompt history from past result files
    if let Ok(entries) = fs::read_dir(&history_dir) {
        let mut files: Vec<_> = entries.filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".result.md"))
            .collect();
        files.sort_by_key(|e| e.file_name());
        for entry in files.iter().rev().take(50) {
            if let Ok(content) = fs::read_to_string(entry.path()) {
                if let Some(prompt) = extract_between(&content, "## Prompt\n```\n", "\n```") {
                    let trimmed = prompt.trim().to_string();
                    // Skip heartbeats, wakeups, and empty prompts
                    if !trimmed.is_empty()
                        && !trimmed.starts_with("# Heartbeat")
                        && !trimmed.starts_with("# Wake Up")
                        && !trimmed.starts_with("[EVENT]")
                    {
                        prompt_history.push(trimmed);
                    }
                }
            }
        }
        prompt_history.reverse(); // oldest first, newest last (Up arrow starts from end)
    }
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
    load_history(&history_dir, &mut stream, &mut activity, &mut last_history_count);

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

                    // Extract prompt name from header
                    let stream_prompt_name = header.split("\"prompt\":\"").nth(1)
                        .and_then(|s| s.split('"').next())
                        .unwrap_or("");
                    let is_system_stream = is_system_prompt(stream_prompt_name);

                    // Detect new stream -- reset tracker when a new prompt appears
                    if is_running && !stream_active && !is_done && display_body.is_empty() {
                        last_stream_len = 0;
                    }

                    // Mark as active as soon as stream shows "running" (not yet done)
                    if is_running && !stream_active && !is_done {
                        stream_active = true;
                        prompt_pending = false;
                        needs_redraw = true;
                    }

                    // Only show in chat if it's a user prompt, not heartbeat/system
                    if is_running && !is_system_stream && display_body.len() > last_stream_len {
                        if let Some(idx) = live_entry_idx {
                            if idx < stream.len() {
                                stream[idx] = StreamEntry::new(
                                    EntryKind::Neil,
                                    display_body.to_string(),
                                );
                            }
                        } else if !is_done || !stream_active {
                            // Create new entry only if not in a stale done state
                            if let Some(last) = stream.last() {
                                if matches!(last.kind, EntryKind::System) {
                                    if last.blocks.first().map(|b| matches!(b, RichBlock::Text(t) if t.contains("sending to neil") || t.contains("thinking") || t.contains("queued"))).unwrap_or(false) {
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

                    // On done: final flush -- re-parse entry with complete content
                    // (catches stream_action output appended after Claude's text)
                    if is_done && stream_active {
                        if !is_system_stream && !display_body.is_empty() {
                            if let Some(idx) = live_entry_idx {
                                if idx < stream.len() {
                                    stream[idx] = StreamEntry::new(
                                        EntryKind::Neil,
                                        display_body.to_string(),
                                    );
                                    needs_redraw = true;
                                }
                            }
                        }
                        stream_active = false;
                        live_entry_idx = None;
                        // Keep last_stream_len so we don't re-read this same stream
                        skip_next_result = true;
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
                check_new_results(&history_dir, &mut stream, &mut activity, &mut last_history_count, &mut auto_scroll);
                // Dedup: if the new entry matches ANY recent Neil entry, remove it
                if stream.len() > prev_len {
                    let new_hint = stream.last().map(|e| e.total_text_len()).unwrap_or(0);
                    if new_hint > 0 {
                        let is_dup = stream.iter().rev().skip(1).take(5).any(|e| {
                            matches!(e.kind, EntryKind::Neil) && e.total_text_len() == new_hint
                        });
                        if is_dup {
                            stream.pop();
                        }
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
                            render_stream_cached(frame, h[0], &cached_chat_lines, &input, cursor_pos, scroll_offset, fps.fps, mouse_captured, stream_active, prompt_pending, tick);
                            render_sidebar(frame, h[1], state, &cached_seal_lines, stream_active, tick, &activity);
                        } else {
                            render_stream_cached(frame, size, &cached_chat_lines, &input, cursor_pos, scroll_offset, fps.fps, mouse_captured, stream_active, prompt_pending, tick);
                        }
                    }
                    View::PanelSelector => {
                        if show_sidebar && size.width > 60 {
                            let h = Layout::default()
                                .direction(Direction::Horizontal)
                                .constraints([Constraint::Min(40), Constraint::Length(28)])
                                .split(size);
                            render_stream_cached(frame, h[0], &cached_chat_lines, &input, cursor_pos, scroll_offset, fps.fps, mouse_captured, stream_active, prompt_pending, tick);
                            render_sidebar(frame, h[1], state, &cached_seal_lines, stream_active, tick, &activity);
                        } else {
                            render_stream_cached(frame, size, &cached_chat_lines, &input, cursor_pos, scroll_offset, fps.fps, mouse_captured, stream_active, prompt_pending, tick);
                        }
                        render_panel_selector(frame, size, panel_selection);
                    }
                    View::Panel(idx) => {
                        render_panel_view(frame, size, *idx, state, fps.fps, hb_selection, hb_expanded, hb_scroll, hb_section, cluster_selection, cluster_expanded, cluster_scroll);
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

                                    // Save to prompt history
                                    prompt_history.push(msg.clone());
                                    history_idx = None;
                                    saved_input.clear();

                                    // Check for slash commands
                                    let trimmed = msg.trim();
                                    if trimmed.starts_with('/') {
                                        let cmd = trimmed.split_whitespace().next().unwrap_or("");
                                        match cmd {
                                            "/clear" => {
                                                stream.clear();
                                                stream.push(StreamEntry::new(EntryKind::System, "Chat cleared.".into()));
                                                cached_chat_stream_len = 0;
                                            }
                                            "/status" => {
                                                let s = cached_state.as_ref().unwrap();
                                                let beats_str = match s.max_daily_beats {
                                                    Some(cap) => format!("{}/{}", s.heartbeat.beats_today, cap),
                                                    None => format!("{}", s.heartbeat.beats_today),
                                                };
                                                stream.push(StreamEntry::new(EntryKind::System, format!(
                                                    "Notes: {} | Beats: {} | Queue: {} | Failures: {} | Intents: {}",
                                                    s.palace.total_notes, beats_str,
                                                    s.system.queue_count,
                                                    s.failures.iter().filter(|f| f.resolution == "pending").count(),
                                                    s.intentions.iter().filter(|i| i.status == "pending").count(),
                                                )));
                                            }
                                            "/help" => {
                                                stream.push(StreamEntry::new(EntryKind::System,
                                                    "/clear - Clear chat\n/status - System status\n/help - This help\n/panels - Open panel selector\n/heartbeat - Trigger a heartbeat\n/history - Show prompt history\nUp/Down - Browse previous prompts\nTab - Open panels\nCtrl+S - Toggle sidebar\nCtrl+M - Toggle scroll/select (select = highlight to copy)".into()
                                                ));
                                            }
                                            "/panels" => {
                                                view = View::PanelSelector;
                                            }
                                            "/heartbeat" => {
                                                let hb_path = neil_home.join("tools/autoPrompter/heartbeat.sh");
                                                let _ = std::process::Command::new("sh").arg(&hb_path).output();
                                                stream.push(StreamEntry::new(EntryKind::System, "Heartbeat queued.".into()));
                                            }
                                            "/history" => {
                                                let hist: String = prompt_history.iter().rev().take(10)
                                                    .enumerate()
                                                    .map(|(i, h)| format!("  {}: {}", i + 1, h.chars().take(60).collect::<String>()))
                                                    .collect::<Vec<_>>().join("\n");
                                                stream.push(StreamEntry::new(EntryKind::System,
                                                    if hist.is_empty() { "(no history)".into() } else { hist }
                                                ));
                                            }
                                            _ => {
                                                stream.push(StreamEntry::new(EntryKind::System,
                                                    format!("Unknown command: {}. Type /help for commands.", cmd)
                                                ));
                                            }
                                        }
                                        input.clear();
                                        cursor_pos = 0;
                                        auto_scroll = true;
                                        scroll_offset = 0;
                                    } else {
                                        // Normal prompt -- send to Neil
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
                                        prompt_pending = true;
                                    }
                                }
                            }
                            KeyCode::Tab => {
                                if input.starts_with('/') {
                                    // Autocomplete slash command
                                    const SLASH_CMDS: &[&str] = &[
                                        "/clear", "/status", "/help", "/panels",
                                        "/heartbeat", "/history",
                                    ];
                                    let partial: &str = &input;
                                    let matches: Vec<&&str> = SLASH_CMDS.iter()
                                        .filter(|c| c.starts_with(partial) && **c != partial)
                                        .collect();
                                    if matches.len() == 1 {
                                        // Exact single match -- complete it
                                        input = matches[0].to_string();
                                        cursor_pos = input.chars().count();
                                    } else if matches.len() > 1 {
                                        // Multiple matches -- complete common prefix
                                        let first = matches[0].as_bytes();
                                        let mut common = first.len();
                                        for m in &matches[1..] {
                                            let mb = m.as_bytes();
                                            common = common.min(mb.len());
                                            for i in 0..common {
                                                if first[i] != mb[i] { common = i; break; }
                                            }
                                        }
                                        if common > input.len() {
                                            input = first[..common].iter().map(|&b| b as char).collect();
                                            cursor_pos = input.chars().count();
                                        }
                                    }
                                } else {
                                    view = View::PanelSelector;
                                }
                            }
                            KeyCode::Char(c) => {
                                if key.modifiers.contains(KeyModifiers::ALT) {
                                    // Alt+1-7 expand panels
                                    if c.is_ascii_digit() && c != '0' {
                                        let idx = (c as u8 - b'1') as usize;
                                        if idx < PANEL_NAMES.len() {
                                            view = View::Panel(idx);
                                        }
                                    }
                                } else if key.modifiers.contains(KeyModifiers::CONTROL) {
                                    match c {
                                        'c' | 'q' => break,
                                        's' => show_sidebar = !show_sidebar,
                                        'm' => {
                                            mouse_captured = !mouse_captured;
                                            if mouse_captured {
                                                let _ = execute!(io::stdout(), crossterm::event::EnableMouseCapture);
                                            } else {
                                                let _ = execute!(io::stdout(), crossterm::event::DisableMouseCapture);
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
                            KeyCode::Up => {
                                // Browse prompt history
                                if !prompt_history.is_empty() {
                                    match history_idx {
                                        None => {
                                            saved_input = input.clone();
                                            history_idx = Some(prompt_history.len() - 1);
                                            input = prompt_history.last().unwrap().clone();
                                            cursor_pos = input.chars().count();
                                        }
                                        Some(idx) if idx > 0 => {
                                            history_idx = Some(idx - 1);
                                            input = prompt_history[idx - 1].clone();
                                            cursor_pos = input.chars().count();
                                        }
                                        _ => {} // at oldest, do nothing
                                    }
                                }
                            }
                            KeyCode::Down => {
                                // Browse prompt history forward
                                match history_idx {
                                    Some(idx) if idx + 1 < prompt_history.len() => {
                                        history_idx = Some(idx + 1);
                                        input = prompt_history[idx + 1].clone();
                                        cursor_pos = input.chars().count();
                                    }
                                    Some(_) => {
                                        // Past newest -- restore saved input
                                        history_idx = None;
                                        input = saved_input.clone();
                                        cursor_pos = input.chars().count();
                                        saved_input.clear();
                                    }
                                    None => {} // not browsing history
                                }
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
                        View::Panel(pidx) => match key.code {
                            KeyCode::Esc | KeyCode::Tab => {
                                if hb_expanded && *pidx == 1 {
                                    hb_expanded = false;
                                } else if cluster_expanded && *pidx == 7 {
                                    cluster_expanded = false;
                                } else {
                                    view = View::Chat;
                                }
                            }
                            KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
                                let idx = (c as u8 - b'1') as usize;
                                if idx < PANEL_NAMES.len() {
                                    view = View::Panel(idx);
                                    hb_expanded = false;
                                    hb_scroll = 0;
                                    cluster_expanded = false;
                                    cluster_scroll = 0;
                                }
                            }
                            KeyCode::Up if *pidx == 1 => {
                                if hb_expanded {
                                    if hb_section > 0 { hb_section -= 1; hb_scroll = 0; }
                                } else if hb_selection > 0 {
                                    hb_selection -= 1;
                                }
                            }
                            KeyCode::Down if *pidx == 1 => {
                                if hb_expanded {
                                    if hb_section < 4 { hb_section += 1; hb_scroll = 0; }
                                } else if let Some(ref st) = cached_state {
                                    if hb_selection + 1 < st.heartbeat.entries.len() {
                                        hb_selection += 1;
                                    }
                                }
                            }
                            KeyCode::Enter if *pidx == 1 => {
                                if !hb_expanded {
                                    hb_expanded = true;
                                    hb_section = 0;
                                    hb_scroll = 0;
                                }
                            }
                            KeyCode::Up if *pidx == 7 => {
                                if cluster_expanded {
                                    if cluster_scroll > 0 { cluster_scroll = cluster_scroll.saturating_sub(1); }
                                } else if cluster_selection > 0 {
                                    cluster_selection -= 1;
                                }
                            }
                            KeyCode::Down if *pidx == 7 => {
                                if cluster_expanded {
                                    cluster_scroll += 1;
                                } else {
                                    // Bounded by snapshot size at render time; allow overshoot and clamp in render.
                                    cluster_selection += 1;
                                }
                            }
                            KeyCode::Enter if *pidx == 7 => {
                                // If the selection lands on a peer card, suspend
                                // the TUI and SSH into that peer (Phase 4 hook).
                                if let Some(peer_ip) = peer_ip_at(cluster_selection) {
                                    let key_path = env::var("NEIL_HOME")
                                        .map(PathBuf::from)
                                        .unwrap_or_else(|_| PathBuf::from(env::var("HOME").unwrap_or("/tmp".into())).join(".neil"))
                                        .join("keys/peer_ed25519");
                                    // Suspend TUI
                                    let _ = terminal::disable_raw_mode();
                                    let _ = execute!(io::stdout(), LeaveAlternateScreen);
                                    let _ = std::process::Command::new("ssh")
                                        .args([
                                            "-t",
                                            "-o", "StrictHostKeyChecking=no",
                                            "-o", "UserKnownHostsFile=/dev/null",
                                            "-i", key_path.to_str().unwrap_or(""),
                                            &format!("root@{}", peer_ip),
                                            "command -v neil-blueprint >/dev/null && neil-blueprint || exec bash -l",
                                        ])
                                        .status();
                                    // Resume TUI
                                    let _ = execute!(io::stdout(), EnterAlternateScreen);
                                    let _ = terminal::enable_raw_mode();
                                    needs_redraw = true;
                                } else {
                                    cluster_expanded = !cluster_expanded;
                                    cluster_scroll = 0;
                                }
                            }
                            _ => {}
                        },
                    }
                }
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        if matches!(view, View::Panel(1)) && hb_expanded {
                            if hb_scroll > 0 { hb_scroll = hb_scroll.saturating_sub(3); }
                            needs_redraw = true;
                        } else {
                            scroll_offset += 3; auto_scroll = false;
                        }
                    }
                    MouseEventKind::ScrollDown => {
                        if matches!(view, View::Panel(1)) && hb_expanded {
                            hb_scroll += 3;
                            needs_redraw = true;
                        } else {
                            scroll_offset = (scroll_offset - 3).max(0);
                            if scroll_offset == 0 { auto_scroll = true; }
                        }
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
        execute!(terminal.backend_mut(), LeaveAlternateScreen, crossterm::event::DisableMouseCapture)?;
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
                        let trimmed = wrapped.trim_start();
                        let style = if trimmed.starts_with("$ ") || trimmed.starts_with("> ") {
                            Style::default().fg(Color::Yellow)
                        } else if trimmed.starts_with("##") {
                            Style::default().fg(text_color).add_modifier(Modifier::BOLD)
                        } else if trimmed.starts_with("**") && trimmed.ends_with("**") {
                            Style::default().fg(text_color).add_modifier(Modifier::BOLD)
                        } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                            Style::default().fg(Color::Cyan)
                        } else if trimmed.contains("~/") || trimmed.contains("/.neil/")
                            || trimmed.contains("/home/")
                        {
                            Style::default().fg(Color::Rgb(180, 180, 220))
                        } else {
                            Style::default().fg(text_color)
                        };
                        lines.push(Line::from(Span::styled(format!("  {}", wrapped), style)));
                    }
                }
                RichBlock::ToolCall { action, detail } => {
                    let border_w = wrap_width.saturating_sub(4);
                    let (icon, action_color) = match action.as_str() {
                        "MEMORY" => (">>", Color::Rgb(180, 130, 255)),  // purple
                        "CALL"   => ("->", Color::Rgb(100, 200, 255)),  // blue
                        "INTEND" => ("++", Color::Rgb(255, 200, 100)),  // amber
                        "DONE"   => ("OK", Color::Green),
                        "FAIL"   => ("!!", Color::Red),
                        "NOTIFY" => ("<>", Color::Rgb(255, 180, 100)),  // orange
                        "HEARTBEAT" => ("~~", Color::Rgb(100, 180, 255)), // light blue
                        "PROMPT" => ("?>", Color::Rgb(200, 200, 100)),  // yellow-ish
                        _ => ("--", Color::Magenta),
                    };
                    // Render as a compact action card
                    let header = format!("  {} {}: {}", icon, action, truncate_str(detail, border_w.saturating_sub(action.len() + 6)));
                    lines.push(Line::from(Span::styled(header, Style::default().fg(action_color))));
                }
                RichBlock::FileEdit { path, lang, lines: diff_lines } => {
                    let border_w = wrap_width.saturating_sub(4);
                    let added = diff_lines.iter().filter(|l| matches!(l, DiffLine::Added(_))).count();
                    let removed = diff_lines.iter().filter(|l| matches!(l, DiffLine::Removed(_))).count();

                    // Header: file path with change summary
                    let path_display = if path.is_empty() { "file".to_string() } else { path.clone() };
                    let change_summary = if added > 0 || removed > 0 {
                        let mut parts = Vec::new();
                        if added > 0 { parts.push(format!("+{}", added)); }
                        if removed > 0 { parts.push(format!("-{}", removed)); }
                        format!(" ({})", parts.join(", "))
                    } else {
                        String::new()
                    };

                    let label = format!(" {} {}{} ", lang, path_display, change_summary);
                    let pad = border_w.saturating_sub(label.len());
                    lines.push(Line::from(vec![
                        Span::styled("  ┌─".to_string(), Style::default().fg(Color::Rgb(80, 80, 80))),
                        Span::styled(label, Style::default().fg(Color::Rgb(180, 180, 220)).add_modifier(Modifier::BOLD)),
                        Span::styled("─".repeat(pad), Style::default().fg(Color::Rgb(80, 80, 80))),
                    ]));

                    // Diff lines with coloring
                    for dl in diff_lines {
                        let (prefix_char, text, style) = match dl {
                            DiffLine::Added(t) => ("+", t.as_str(), Style::default().fg(Color::Green)),
                            DiffLine::Removed(t) => ("-", t.as_str(), Style::default().fg(Color::Red)),
                            DiffLine::Context(t) => {
                                if t.starts_with("@@") {
                                    ("@", t.as_str(), Style::default().fg(Color::Rgb(100, 100, 200)))
                                } else {
                                    (" ", t.as_str(), Style::default().fg(Color::Rgb(140, 140, 140)))
                                }
                            }
                        };
                        let display = format!("  {} {} {}", "|", prefix_char, text);
                        let truncated = truncate_str(&display, wrap_width);
                        lines.push(Line::from(Span::styled(truncated.to_string(), style)));
                    }

                    lines.push(Line::from(Span::styled(
                        format!("  └{}", "─".repeat(border_w)),
                        Style::default().fg(Color::Rgb(80, 80, 80)),
                    )));
                }
                RichBlock::Command { cmd, output } => {
                    let border_w = wrap_width.saturating_sub(4);
                    // Command header
                    lines.push(Line::from(vec![
                        Span::styled("  $ ".to_string(), Style::default().fg(Color::Rgb(100, 200, 100))),
                        Span::styled(
                            truncate_str(cmd, border_w.saturating_sub(2)).to_string(),
                            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                        ),
                    ]));
                    // Output (dimmed, indented)
                    if !output.is_empty() {
                        for ol in output.lines() {
                            let display = format!("    {}", ol);
                            let truncated = truncate_str(&display, wrap_width);
                            lines.push(Line::from(Span::styled(
                                truncated.to_string(),
                                Style::default().fg(Color::Rgb(140, 140, 140)),
                            )));
                        }
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
                            format!("  | {}", cl), Style::default().fg(Color::Yellow),
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
                            format!("  | {}", dl), Style::default().fg(Color::Cyan),
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
                            format!("  {:<5} {}{} {}", label, "=".repeat(bw), ".".repeat(bar_max - bw), val),
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

/// Truncate a string to fit within max_width characters
fn truncate_str(s: &str, max_width: usize) -> &str {
    if s.len() <= max_width {
        s
    } else {
        // Find a safe char boundary
        let mut end = max_width;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        &s[..end]
    }
}

/// Render the stream view using pre-built cached lines (cheap per frame)
fn render_stream_cached(
    frame: &mut ratatui::Frame, area: Rect, cached_lines: &[Line<'static>],
    input: &str, cursor_pos: usize, scroll_offset: i32, fps: u32,
    mouse_captured: bool, stream_active: bool, prompt_pending: bool, tick: u64,
) {
    let wrap_width = (area.width as usize).saturating_sub(4);

    // Dynamic input box: grows with content, min 3 lines, max 8
    let input_char_count = input.chars().count();
    let input_lines = if input.is_empty() { 1 }
        else if input_char_count > 200 { 2 }
        else { wrap_text(input, wrap_width.saturating_sub(2)).len() };
    let input_height = (input_lines as u16 + 2).clamp(3, 8);
    let show_loading = stream_active || prompt_pending;
    let loading_height: u16 = if show_loading { 1 } else { 0 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),           // header
            Constraint::Min(3),              // conversation
            Constraint::Length(loading_height), // loading animation (0 when idle)
            Constraint::Length(input_height), // input box
        ])
        .split(area);

    // Header bar with animated seal status
    let time_str = chrono::Local::now().format("%H:%M:%S").to_string();

    let status_span = if stream_active || prompt_pending {
        Span::styled(" working ", Style::default().fg(Color::Yellow))
    } else {
        Span::styled(" idle ", Style::default().fg(Color::DarkGray))
    };

    let header = Line::from(vec![
        Span::styled(" NEIL ", Style::default().fg(Color::Black).bg(Color::Cyan)),
        status_span,
        Span::styled(format!("{} ", time_str), Style::default().fg(Color::DarkGray)),
        Span::styled("Alt+1-8:panels Ctrl+S:sidebar Esc:quit ", Style::default().fg(Color::Rgb(60, 60, 60))),
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

    // Loading animation -- ocean wave physics
    if show_loading {
        let t = tick as f64 * 0.15;
        let w = (area.width as usize).saturating_sub(4);
        let wave: String = (0..w).map(|i| {
            let x = i as f64;
            // Three overlapping sine waves at different frequencies
            let h = (x * 0.3 + t).sin() * 0.4
                  + (x * 0.15 - t * 0.7).sin() * 0.3
                  + (x * 0.5 + t * 1.3).sin() * 0.2;
            // Map wave height to characters
            if h > 0.5 { '≈' }
            else if h > 0.2 { '∿' }
            else if h > -0.1 { '~' }
            else if h > -0.3 { '∼' }
            else if h > -0.5 { '·' }
            else { ' ' }
        }).collect();
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("  {}", wave),
                Style::default().fg(Color::Cyan),
            ))),
            chunks[2],
        );
    }

    // Input box (dynamic height, word-wrapped)
    let input_area = chunks[3];
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
        // Compute slash command suggestion if input starts with /
        let slash_hint: String = if input.starts_with('/') && !input.contains(' ') {
            const SLASH_CMDS: &[&str] = &[
                "/clear", "/status", "/help", "/panels",
                "/heartbeat", "/history",
            ];
            let partial: &str = &input;
            SLASH_CMDS.iter()
                .find(|c| c.starts_with(partial) && **c != partial)
                .map(|c| c[partial.len()..].to_string())
                .unwrap_or_default()
        } else {
            String::new()
        };

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
                let mut spans = vec![
                    Span::styled(before.to_string(), Style::default().fg(Color::White)),
                ];
                // Show autocomplete hint flowing directly from typed text (no cursor break)
                if !slash_hint.is_empty() && after.is_empty() && cc + line_char_count >= input.chars().count() {
                    spans.push(Span::styled(slash_hint.clone(), Style::default().fg(Color::DarkGray)));
                } else {
                    spans.push(Span::styled("_", Style::default().fg(Color::Cyan).add_modifier(Modifier::SLOW_BLINK)));
                    spans.push(Span::styled(after.to_string(), Style::default().fg(Color::White)));
                }
                result.push(Line::from(spans));
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

fn render_sidebar(frame: &mut ratatui::Frame, area: Rect, state: &NeilState, seal_lines_raw: &[String], stream_active: bool, tick: u64, activity: &[String]) {
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
        Line::from(Span::styled(
            match state.max_daily_beats {
                Some(cap) => format!(" beats: {}/{}", state.heartbeat.beats_today, cap),
                None => format!(" beats: {}", state.heartbeat.beats_today),
            },
            Style::default().fg(if state.max_daily_beats.map(|c| state.heartbeat.beats_today > c).unwrap_or(false) { Color::Red } else { Color::DarkGray }),
        )),
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
        Paragraph::new(mem_lines).block(Block::default().borders(Borders::ALL).title(" [1] memory ").border_style(Style::default().fg(Color::DarkGray))),
        chunks[1],
    );

    // Activity panel -- recent heartbeat summaries + pending intents
    let mut inbox_lines = Vec::new();

    // Show recent activity (heartbeat summaries)
    if !activity.is_empty() {
        for act in activity.iter().rev().take(4) {
            inbox_lines.push(Line::from(Span::styled(
                format!(" {}", act.chars().take(24).collect::<String>()),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    // Show pending intents below activity
    let pending: Vec<_> = state.intentions.iter().filter(|i| i.status == "pending").collect();
    if !pending.is_empty() {
        if !inbox_lines.is_empty() {
            inbox_lines.push(Line::from(Span::styled(" ───", Style::default().fg(Color::Rgb(40, 40, 40)))));
        }
        for i in pending.iter().take(3) {
            let color = match i.priority.as_str() { "high" => Color::Red, "medium" => Color::Yellow, _ => Color::Green };
            inbox_lines.push(Line::from(vec![
                Span::styled(format!(" [{}] ", i.priority.chars().next().unwrap_or('?')), Style::default().fg(color)),
                Span::styled(i.description.chars().take(18).collect::<String>(), Style::default().fg(Color::DarkGray)),
            ]));
        }
    }

    if inbox_lines.is_empty() {
        inbox_lines.push(Line::from(Span::styled(" (quiet)", Style::default().fg(Color::DarkGray))));
    }

    frame.render_widget(
        Paragraph::new(inbox_lines).block(Block::default().borders(Borders::ALL).title(" [2] activity ").border_style(Style::default().fg(Color::DarkGray))),
        chunks[2],
    );

    // Seal art with speech bubble
    let mut seal_lines: Vec<Line> = Vec::new();

    // Speech bubble -- contextual message above the seal
    let bubble_text = if stream_active {
        let dots = ".".repeat(((tick / 4) % 4) as usize + 1);
        format!("working{}", dots)
    } else {
        let pending_fails = state.failures.iter().filter(|f| f.resolution == "pending").count();
        let pending_intents = state.intentions.iter().filter(|i| i.status == "pending").count();
        if pending_fails > 0 {
            "need to fix something...".into()
        } else if pending_intents > 0 {
            format!("{} things on my mind", pending_intents)
        } else if state.heartbeat.beats_today > 40 {
            "getting tired...".into()
        } else {
            "all good :)".into()
        }
    };

    seal_lines.push(Line::from(vec![
        Span::styled(" ◃ ", Style::default().fg(Color::DarkGray)),
        Span::styled(&bubble_text, Style::default().fg(Color::White)),
    ]));

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

fn render_panel_view(frame: &mut ratatui::Frame, area: Rect, idx: usize, state: &NeilState, fps: u32,
                     hb_sel: usize, hb_expanded: bool, hb_scroll: usize, hb_section: usize,
                     cluster_sel: usize, cluster_expanded: bool, cluster_scroll: usize) {
    let (name, _) = PANEL_NAMES.get(idx).unwrap_or(&("?", ""));

    // Heartbeat expanded: two-pane layout rendered directly
    if idx == 1 && hb_expanded {
        render_heartbeat_expanded(frame, area, state, hb_sel, hb_section, hb_scroll, fps);
        return;
    }
    // Cluster expanded: two-pane detail view
    if idx == 7 && cluster_expanded {
        render_cluster_expanded(frame, area, cluster_sel, cluster_scroll, fps);
        return;
    }

    let title = if idx == 1 {
        format!(" {} | Up/Down:select Enter:expand Esc:close ", name)
    } else if idx == 7 {
        format!(" {} | Up/Down:select Enter:open Esc:close ", name)
    } else {
        format!(" {} | Esc:close 1-8:switch ", name)
    };
    let block = Block::default().borders(Borders::ALL).title(title).border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines: Vec<Line> = match idx {
        0 => render_memory_panel(state),
        1 => render_heartbeat_panel(state, hb_sel),
        2 => render_intentions_panel(state),
        3 => render_system_panel(state),
        4 => render_services_panel(state),
        5 => render_failures_panel(state),
        6 => render_logs_panel(),
        7 => render_cluster_panel_selectable(state, cluster_sel),
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

fn render_heartbeat_panel(s: &NeilState, selected: usize) -> Vec<Line<'static>> {
    let cap_str = s.max_daily_beats.map(|n| format!("/{}", n)).unwrap_or_default();
    let mut l = vec![
        Line::from(Span::styled(
            format!("Beats today: {}{} | Last: {}", s.heartbeat.beats_today, cap_str, s.heartbeat.last_beat),
            Style::default().fg(Color::Cyan),
        )),
        Line::from(""),
    ];

    {
        // List view -- show all entries, highlight selected
        for (i, e) in s.heartbeat.entries.iter().enumerate() {
            let is_sel = i == selected;
            let status_color = match e.status.as_str() {
                "ok" => Color::Green, "acted" => Color::Cyan, "error" => Color::Red, _ => Color::DarkGray
            };

            let summary_text: String = if !e.action.is_empty() {
                e.action.chars().take(60).collect()
            } else {
                e.summary.chars().take(60).collect()
            };

            let has_report = !e.question.is_empty() || !e.contribution.is_empty();
            let indicator = if has_report { "+" } else { " " };

            if is_sel {
                l.push(Line::from(vec![
                    Span::styled(" > ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::styled(format!("{} ", e.timestamp), Style::default().fg(Color::White)),
                    Span::styled(format!("[{}] ", e.status), Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
                    Span::styled(format!("{} ", indicator), Style::default().fg(Color::Yellow)),
                    Span::styled(summary_text, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                ]));
            } else {
                l.push(Line::from(vec![
                    Span::styled("   ", Style::default()),
                    Span::styled(format!("{} ", e.timestamp), Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("[{}] ", e.status), Style::default().fg(status_color)),
                    Span::styled(format!("{} ", indicator), Style::default().fg(Color::DarkGray)),
                    Span::styled(summary_text, Style::default().fg(Color::White)),
                ]));
            }
        }

        if s.heartbeat.entries.is_empty() {
            l.push(Line::from(Span::styled("  No heartbeats recorded yet", Style::default().fg(Color::DarkGray))));
        }
    }
    l
}

/// Simple word wrap for panel text
fn textwrap_simple(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            if line.len() + word.len() + 1 > width && !line.is_empty() {
                lines.push(line);
                line = String::new();
            }
            if !line.is_empty() { line.push(' '); }
            line.push_str(word);
        }
        if !line.is_empty() { lines.push(line); }
    }
    if lines.is_empty() { lines.push(text.to_string()); }
    lines
}

const HB_SECTIONS: &[(&str, Color)] = &[
    ("ACTION",       Color::White),
    ("QUESTION",     Color::Rgb(255, 200, 100)),
    ("IMPROVEMENT",  Color::Green),
    ("CONTRIBUTION", Color::Rgb(180, 130, 255)),
    ("COMMAND LOG",  Color::Rgb(100, 200, 255)),
];

fn render_heartbeat_expanded(
    frame: &mut ratatui::Frame, area: Rect, state: &NeilState,
    hb_sel: usize, section: usize, scroll: usize, fps: u32,
) {
    let e = match state.heartbeat.entries.get(hb_sel) {
        Some(e) => e,
        None => return,
    };

    let status_color = match e.status.as_str() {
        "ok" => Color::Green, "acted" => Color::Cyan, "error" => Color::Red, _ => Color::DarkGray
    };

    // Outer border
    let title = format!(" {} [{}] | Esc:back Up/Down:sections ", e.timestamp, e.status);
    let outer = Block::default().borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(status_color));
    let outer_inner = outer.inner(area);
    frame.render_widget(outer, area);

    // Split: left sidebar (22 cols) | right content
    let h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(22), Constraint::Min(20)])
        .split(outer_inner);

    let left_area = h_chunks[0];
    let right_area = h_chunks[1];

    // ── Left: section list ──
    let mut left_lines: Vec<Line<'static>> = Vec::new();
    left_lines.push(Line::from(""));
    for (i, (name, color)) in HB_SECTIONS.iter().enumerate() {
        let is_sel = i == section;
        if is_sel {
            left_lines.push(Line::from(vec![
                Span::styled(" > ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(*name, Style::default().fg(*color).add_modifier(Modifier::BOLD)),
            ]));
        } else {
            left_lines.push(Line::from(vec![
                Span::styled("   ", Style::default()),
                Span::styled(*name, Style::default().fg(Color::DarkGray)),
            ]));
        }
        left_lines.push(Line::from(""));
    }

    let left_block = Block::default().borders(Borders::RIGHT).border_style(Style::default().fg(Color::Rgb(50, 50, 50)));
    let left_inner = left_block.inner(left_area);
    frame.render_widget(left_block, left_area);
    frame.render_widget(Paragraph::new(left_lines), left_inner);

    // ── Right: content for selected section ──
    let content_width = right_area.width.saturating_sub(4) as usize;
    let mut right_lines: Vec<Line<'static>> = Vec::new();
    right_lines.push(Line::from(""));

    let (_, sec_color) = HB_SECTIONS[section.min(HB_SECTIONS.len() - 1)];

    match section {
        0 => {
            // ACTION
            let text = if e.action.is_empty() { &e.summary } else { &e.action };
            for line in textwrap_simple(text, content_width) {
                right_lines.push(Line::from(Span::styled(format!("  {}", line), Style::default().fg(sec_color))));
            }
        }
        1 => {
            // QUESTION
            if e.question.is_empty() {
                right_lines.push(Line::from(Span::styled("  (no question recorded)", Style::default().fg(Color::DarkGray))));
            } else {
                for line in textwrap_simple(&e.question, content_width) {
                    right_lines.push(Line::from(Span::styled(format!("  {}", line), Style::default().fg(sec_color))));
                }
            }
        }
        2 => {
            // IMPROVEMENT
            if e.improvement.is_empty() {
                right_lines.push(Line::from(Span::styled("  (none recorded)", Style::default().fg(Color::DarkGray))));
            } else {
                for line in textwrap_simple(&e.improvement, content_width) {
                    right_lines.push(Line::from(Span::styled(format!("  {}", line), Style::default().fg(sec_color))));
                }
            }
        }
        3 => {
            // CONTRIBUTION
            if e.contribution.is_empty() {
                right_lines.push(Line::from(Span::styled("  (none recorded)", Style::default().fg(Color::DarkGray))));
            } else {
                for line in textwrap_simple(&e.contribution, content_width) {
                    right_lines.push(Line::from(Span::styled(format!("  {}", line), Style::default().fg(sec_color))));
                }
            }
        }
        4 => {
            // COMMAND LOG
            let cmd_log = load_command_log(&state.neil_home, &e.prompt);
            if cmd_log.is_empty() {
                right_lines.push(Line::from(Span::styled("  (no commands recorded)", Style::default().fg(Color::DarkGray))));
            } else {
                // Usable width after 2-char indent + 2-char prefix + 2-space continuation
                let wrap_w = content_width.saturating_sub(6).max(20);
                let cont_w = content_width.saturating_sub(6).max(20);

                for entry in &cmd_log {
                    match entry {
                        CommandLogEntry::Command { cmd, output } => {
                            // Wrap the command itself
                            let cmd_lines = textwrap_simple(cmd, wrap_w);
                            for (i, cl) in cmd_lines.iter().enumerate() {
                                let prefix = if i == 0 { "  $ " } else { "    " };
                                let color = if i == 0 { Color::Rgb(100, 200, 100) } else { Color::DarkGray };
                                right_lines.push(Line::from(vec![
                                    Span::styled(prefix.to_string(), Style::default().fg(color)),
                                    Span::styled(cl.clone(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                                ]));
                            }
                            // Wrap output lines (max 8 source lines shown)
                            let mut source_line_count = 0;
                            for ol in output.lines() {
                                if source_line_count >= 8 { break; }
                                source_line_count += 1;
                                for wl in textwrap_simple(ol, cont_w) {
                                    right_lines.push(Line::from(Span::styled(
                                        format!("    {}", wl), Style::default().fg(Color::Rgb(120, 120, 120)),
                                    )));
                                }
                            }
                            let total = output.lines().count();
                            if total > 8 {
                                right_lines.push(Line::from(Span::styled(
                                    format!("    ... ({} more)", total - 8), Style::default().fg(Color::DarkGray),
                                )));
                            }
                            right_lines.push(Line::from(""));
                        }
                        CommandLogEntry::Memory(d) => {
                            let full = format!("MEMORY: {}", d);
                            let wrapped = textwrap_simple(&full, wrap_w);
                            for (i, wl) in wrapped.iter().enumerate() {
                                let prefix = if i == 0 { "  >> " } else { "     " };
                                right_lines.push(Line::from(vec![
                                    Span::styled(prefix.to_string(), Style::default().fg(Color::Rgb(180, 130, 255))),
                                    Span::styled(wl.clone(), Style::default().fg(Color::Rgb(180, 130, 255))),
                                ]));
                            }
                        }
                        CommandLogEntry::ServiceCall(d) => {
                            let full = format!("CALL: {}", d);
                            let wrapped = textwrap_simple(&full, wrap_w);
                            for (i, wl) in wrapped.iter().enumerate() {
                                let prefix = if i == 0 { "  -> " } else { "     " };
                                right_lines.push(Line::from(vec![
                                    Span::styled(prefix.to_string(), Style::default().fg(Color::Rgb(100, 200, 255))),
                                    Span::styled(wl.clone(), Style::default().fg(Color::Rgb(100, 200, 255))),
                                ]));
                            }
                        }
                        CommandLogEntry::Mempalace(d) => {
                            for (i, wl) in textwrap_simple(d, wrap_w).iter().enumerate() {
                                let prefix = if i == 0 { "  ~ " } else { "    " };
                                right_lines.push(Line::from(Span::styled(
                                    format!("{}{}", prefix, wl), Style::default().fg(Color::Rgb(100, 180, 255)),
                                )));
                            }
                        }
                        CommandLogEntry::FileWrite(d) => {
                            let wrapped = textwrap_simple(d, wrap_w);
                            for (i, wl) in wrapped.iter().enumerate() {
                                if i == 0 {
                                    right_lines.push(Line::from(vec![
                                        Span::styled("  W ".to_string(), Style::default().fg(Color::Black).bg(Color::Green)),
                                        Span::styled(format!(" {}", wl), Style::default().fg(Color::Green)),
                                    ]));
                                } else {
                                    right_lines.push(Line::from(Span::styled(
                                        format!("     {}", wl), Style::default().fg(Color::Green),
                                    )));
                                }
                            }
                        }
                        CommandLogEntry::FileRead(d) => {
                            let wrapped = textwrap_simple(d, wrap_w);
                            for (i, wl) in wrapped.iter().enumerate() {
                                if i == 0 {
                                    right_lines.push(Line::from(vec![
                                        Span::styled("  R ".to_string(), Style::default().fg(Color::Black).bg(Color::Cyan)),
                                        Span::styled(format!(" {}", wl), Style::default().fg(Color::Cyan)),
                                    ]));
                                } else {
                                    right_lines.push(Line::from(Span::styled(
                                        format!("     {}", wl), Style::default().fg(Color::Cyan),
                                    )));
                                }
                            }
                        }
                        CommandLogEntry::BashAction(d) => {
                            for (i, wl) in textwrap_simple(d, wrap_w).iter().enumerate() {
                                let prefix = if i == 0 { "  $ " } else { "    " };
                                let color = if i == 0 { Color::Rgb(100, 200, 100) } else { Color::DarkGray };
                                right_lines.push(Line::from(vec![
                                    Span::styled(prefix.to_string(), Style::default().fg(color)),
                                    Span::styled(wl.clone(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                                ]));
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }

    // Source footer
    right_lines.push(Line::from(""));
    right_lines.push(Line::from(Span::styled(
        format!("  source: {}", e.prompt), Style::default().fg(Color::Rgb(60, 60, 60)),
    )));

    // Apply scroll
    let skip = scroll.min(right_lines.len().saturating_sub(1));
    let visible: Vec<Line> = right_lines.into_iter().skip(skip).collect();

    frame.render_widget(Paragraph::new(visible), right_area);

    // FPS
    let fps_text = format!(" {}fps ", fps);
    let fl = fps_text.len() as u16;
    let fx = area.x + area.width.saturating_sub(fl + 1);
    let fy = area.y + area.height - 1;
    frame.render_widget(
        Paragraph::new(Span::styled(fps_text, Style::default().fg(Color::DarkGray))),
        Rect::new(fx, fy, fl, 1),
    );
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

/// Cluster panel with selectable cards (selection highlight but no expansion).
/// Instance index 0 = MAIN, 1..=N = live temps, N+1..=M = recent history.
fn render_cluster_panel_selectable(_state: &NeilState, selection: usize) -> Vec<Line<'static>> {
    cluster_lines(selection)
}

fn cluster_lines(selection: usize) -> Vec<Line<'static>> {
    // Invoke neil-cluster to get current cluster state
    let neil_home = env::var("NEIL_HOME").map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env::var("HOME").unwrap_or("/tmp".into())).join(".neil"));
    let bin = neil_home.join("bin/neil-cluster");
    if !bin.exists() {
        return vec![
            Line::from(Span::styled("  neil-cluster not installed", Style::default().fg(Color::Red))),
            Line::from(""),
            Line::from(Span::styled(format!("  expected at: {}", bin.display()), Style::default().fg(Color::DarkGray))),
        ];
    }

    let output = std::process::Command::new(&bin)
        .arg("status")
        .arg("--json")
        .arg("--compact")
        .env("NEIL_HOME", &neil_home)
        .output();

    let json_text = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        Ok(o) => {
            return vec![
                Line::from(Span::styled("  neil-cluster failed", Style::default().fg(Color::Red))),
                Line::from(Span::styled(format!("  stderr: {}", String::from_utf8_lossy(&o.stderr)), Style::default().fg(Color::DarkGray))),
            ];
        }
        Err(e) => {
            return vec![
                Line::from(Span::styled(format!("  error: {}", e), Style::default().fg(Color::Red))),
            ];
        }
    };

    // Parse JSON (handwritten; we already have serde in deps but avoid extra coupling here)
    // Extract main fields and temp list for rendering.
    let mut lines: Vec<Line<'static>> = Vec::new();

    let node_id = extract_string(&json_text, "\"node_id\"").unwrap_or("unknown".into());
    let ts = extract_string(&json_text, "\"timestamp\"").unwrap_or("?".into());
    lines.push(Line::from(vec![
        Span::styled("  Cluster on ", Style::default().fg(Color::DarkGray)),
        Span::styled(node_id.clone(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(format!("  @ {}", ts), Style::default().fg(Color::DarkGray)),
    ]));
    lines.push(Line::from(""));

    // ── Build the node list (MAIN + all children) ──
    let main_selected = selection == 0;
    let main_name    = extract_main_field(&json_text, "name").unwrap_or("main".into());
    let main_persona = extract_main_field(&json_text, "persona").unwrap_or("default".into());
    let main_mem     = extract_main_field(&json_text, "memory_type").unwrap_or("full".into());
    let main_status  = extract_main_field(&json_text, "status").unwrap_or("idle".into());
    let main_up      = extract_main_field(&json_text, "uptime_sec").unwrap_or("0".into());
    let main_task    = extract_main_field(&json_text, "current_task").unwrap_or_default();
    let main_pending = extract_main_field(&json_text, "pending_intentions").unwrap_or("0".into());

    let temps = parse_temps(&json_text);
    let peers = parse_peers(&json_text);

    let mut children: Vec<ChildCard> = Vec::new();
    for t in &temps {
        children.push(ChildCard {
            kind: "temp",
            name: t.name.clone(),
            status: t.status.clone(),
            line1: format!("persona={}", t.persona),
            line2: format!("mem={}", t.memory_type),
            line3: if t.current_task.is_empty() { format!("up={}", fmt_duration(&t.uptime_sec)) }
                   else { format!("task: {}", t.current_task.chars().take(20).collect::<String>()) },
        });
    }
    for p in &peers {
        children.push(ChildCard {
            kind: "peer",
            name: p.name.clone(),
            status: p.status.clone(),
            line1: format!("ip={}", p.ip),
            line2: format!("img={}", p.image.chars().take(18).collect::<String>()),
            line3: format!("status={}", p.status),
        });
    }

    // ── Layout constants (mirror cluster_preview.py) ──
    const INDENT:   usize = 2;
    const CARD_W:   usize = 22;
    const GAP:      usize = 2;
    const PER_ROW:  usize = 3;
    const MAIN_W:   usize = 48;

    let connector_style = Style::default().fg(Color::Rgb(90, 90, 110));

    // Compute centered layout: MAIN and first-row children share the same center.
    let n_first = children.len().min(PER_ROW);
    let (main_indent, card_indent) = if n_first == 0 {
        (INDENT, INDENT)
    } else {
        let row_w = n_first * CARD_W + (n_first - 1) * GAP;
        if row_w >= MAIN_W {
            (INDENT + (row_w - MAIN_W) / 2, INDENT)
        } else {
            (INDENT, INDENT + (MAIN_W - row_w) / 2)
        }
    };

    // ── MAIN card ──
    let main_card = build_main_box(&main_name, &main_status, &main_persona, &main_mem,
                                   &main_up, &main_task, &main_pending, main_selected);
    for cl in main_card {
        lines.push(prefix_line(&cl, main_indent));
    }

    if children.is_empty() {
        lines.push(Line::from(""));
        lines.push(prefix_line(&Line::from(Span::styled(
            "(no live children)", Style::default().fg(Color::DarkGray),
        )), INDENT));
    } else {
        let main_center = main_indent + MAIN_W / 2;
        let card_centers: Vec<usize> = (0..n_first)
            .map(|i| card_indent + i * (CARD_W + GAP) + CARD_W / 2)
            .collect();

        // Trunk line (single `│` at MAIN center)
        let mut trunk = String::new();
        for _ in 0..main_center { trunk.push(' '); }
        trunk.push('│');
        lines.push(Line::from(Span::styled(trunk, connector_style)));

        // Fan-out connector: special-case N=1 as a straight drop.
        if n_first == 1 {
            let mut drop = String::new();
            for _ in 0..card_centers[0] { drop.push(' '); }
            drop.push('│');
            lines.push(Line::from(Span::styled(drop, connector_style)));
        } else {
            let left = card_centers[0];
            let right = *card_centers.last().unwrap();
            let width = right.max(main_center) + 1;
            let mut buf: Vec<char> = vec![' '; width];
            for col in left..=right { buf[col] = '─'; }
            buf[left]  = '┌';
            buf[right] = '┐';
            for c in &card_centers[1..card_centers.len() - 1] { buf[*c] = '┬'; }
            if main_center > left && main_center < right {
                buf[main_center] = if card_centers.contains(&main_center) { '┼' } else { '┴' };
            }
            lines.push(Line::from(Span::styled(
                buf.into_iter().collect::<String>(), connector_style,
            )));
        }

        // Arrow row: ▼ at each card center
        {
            let mut buf: Vec<char> = vec![' '; card_centers[card_centers.len()-1] + 1];
            for c in &card_centers { buf[*c] = '▼'; }
            lines.push(Line::from(Span::styled(
                buf.into_iter().collect::<String>(),
                Style::default().fg(Color::Rgb(120, 120, 120)),
            )));
        }

        // Children cards (all rows use the same card_indent)
        for (row_idx, row) in children.chunks(PER_ROW).enumerate() {
            let row_cards: Vec<Vec<Line<'static>>> = row.iter().enumerate().map(|(i, c)| {
                let global_idx = row_idx * PER_ROW + i;
                let sel = selection == global_idx + 1;
                build_child_box(c, sel, CARD_W)
            }).collect();
            let card_h = row_cards[0].len();
            for li in 0..card_h {
                let mut spans: Vec<Span> = vec![Span::raw(" ".repeat(card_indent))];
                for (i, card) in row_cards.iter().enumerate() {
                    if i > 0 { spans.push(Span::raw(" ".repeat(GAP))); }
                    for s in &card[li].spans { spans.push(s.clone()); }
                }
                lines.push(Line::from(spans));
            }
            if row_idx < (children.len() - 1) / PER_ROW {
                lines.push(Line::from(""));
            }
        }
    }

    // Stats footer
    lines.push(Line::from(""));
    let pending = extract_number(&json_text, "\"pending_promotions_count\"").unwrap_or(0);
    let pending_color = if pending >= 5 { Color::Yellow } else { Color::DarkGray };
    lines.push(Line::from(vec![
        Span::styled("  pending promotions: ", Style::default().fg(Color::DarkGray)),
        Span::styled(pending.to_string(), Style::default().fg(pending_color).add_modifier(Modifier::BOLD)),
    ]));

    // Recent history section
    let recent = parse_recent(&json_text);
    if !recent.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Recent (last hour):",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )));
        for ev in recent.iter().rev().take(10) {
            let (icon, color) = match ev.event.as_str() {
                "spawn" => ("▸", Color::Yellow),
                "complete" => ("✓", Color::Green),
                "fail" => ("✗", Color::Red),
                _ => ("·", Color::DarkGray),
            };
            let short_id: String = ev.id.chars().skip("neil_temp_".len()).take(20).collect();
            let detail: String = ev.detail.chars().take(70).collect();
            lines.push(Line::from(vec![
                Span::styled("    ", Style::default()),
                Span::styled(icon.to_string(), Style::default().fg(color)),
                Span::styled(format!(" {} ", ev.ts), Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{:<10}", ev.event), Style::default().fg(color)),
                Span::styled(format!(" {:<20} ", short_id), Style::default().fg(Color::Rgb(120, 120, 120))),
                Span::styled(detail, Style::default().fg(Color::White)),
            ]));
        }
    }

    lines
}

struct RecentEvent {
    ts: String,
    event: String,
    id: String,
    detail: String,
}

fn parse_recent(json: &str) -> Vec<RecentEvent> {
    let mut out = Vec::new();
    let Some(idx) = json.find("\"recent\"") else { return out; };
    let after = &json[idx..];
    let Some(bracket) = after.find('[') else { return out; };
    let body = &after[bracket..];
    let mut depth = 0;
    let mut end = 0;
    for (i, c) in body.chars().enumerate() {
        match c {
            '[' | '{' => depth += 1,
            ']' | '}' => { depth -= 1; if depth == 0 { end = i; break; } }
            _ => {}
        }
    }
    if end == 0 { return out; }
    let arr = &body[1..end];
    let bytes = arr.as_bytes();
    let mut cur_depth = 0;
    let mut start = 0;
    let mut objects: Vec<&str> = Vec::new();
    for i in 0..bytes.len() {
        match bytes[i] {
            b'{' => { if cur_depth == 0 { start = i; } cur_depth += 1; }
            b'}' => { cur_depth -= 1; if cur_depth == 0 { objects.push(&arr[start..=i]); } }
            _ => {}
        }
    }
    for obj in objects {
        let e = RecentEvent {
            ts: extract_obj_field(obj, "ts").unwrap_or_default(),
            event: extract_obj_field(obj, "event").unwrap_or_default(),
            id: extract_obj_field(obj, "id").unwrap_or_default(),
            detail: extract_obj_field(obj, "detail").unwrap_or_default(),
        };
        out.push(e);
    }
    out
}

/// Cluster expanded: two-pane detail view for the selected instance.
/// Left = instance list. Right = detail pane for selected instance.
fn render_cluster_expanded(
    frame: &mut ratatui::Frame, area: Rect, selection: usize, scroll: usize, fps: u32,
) {
    // Fetch cluster snapshot
    let neil_home = env::var("NEIL_HOME").map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env::var("HOME").unwrap_or("/tmp".into())).join(".neil"));
    let bin = neil_home.join("bin/neil-cluster");
    let output = std::process::Command::new(&bin)
        .arg("status").arg("--json").arg("--compact")
        .env("NEIL_HOME", &neil_home)
        .output();
    let json_text = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => "{}".to_string(),
    };

    // Outer block
    let title = " Cluster: instance detail | Esc:back Up/Down:scroll ".to_string();
    let outer = Block::default().borders(Borders::ALL).title(title)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    // Split: left 24 cols, right flexible
    let h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(24), Constraint::Min(30)])
        .split(inner);
    let left_area = h_chunks[0];
    let right_area = h_chunks[1];

    // Build instance list (MAIN + temps). Selection is bounded to list size.
    let temps = parse_temps(&json_text);
    let total_instances = 1 + temps.len();
    let bounded_sel = if total_instances == 0 { 0 } else { selection.min(total_instances - 1) };

    // Left column: instance list
    let mut left_lines: Vec<Line<'static>> = vec![Line::from("")];
    // MAIN
    let sel_main = bounded_sel == 0;
    left_lines.push(Line::from(vec![
        Span::styled(if sel_main { " > " } else { "   " }, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled("MAIN", Style::default().fg(if sel_main { Color::Cyan } else { Color::DarkGray }).add_modifier(if sel_main { Modifier::BOLD } else { Modifier::empty() })),
    ]));
    left_lines.push(Line::from(""));
    for (i, t) in temps.iter().enumerate() {
        let sel = bounded_sel == i + 1;
        let short: String = t.name.chars().skip("neil_temp_".len()).take(16).collect();
        left_lines.push(Line::from(vec![
            Span::styled(if sel { " > " } else { "   " }, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(format!("temp: {}", short), Style::default().fg(if sel { Color::Yellow } else { Color::DarkGray }).add_modifier(if sel { Modifier::BOLD } else { Modifier::empty() })),
        ]));
    }
    let left_block = Block::default().borders(Borders::RIGHT).border_style(Style::default().fg(Color::Rgb(50, 50, 50)));
    let left_inner = left_block.inner(left_area);
    frame.render_widget(left_block, left_area);
    frame.render_widget(Paragraph::new(left_lines), left_inner);

    // Right pane: detail for selected
    let mut right_lines: Vec<Line<'static>> = vec![Line::from("")];
    if bounded_sel == 0 {
        // MAIN detail
        let name = extract_main_field(&json_text, "name").unwrap_or("main".into());
        let persona = extract_main_field(&json_text, "persona").unwrap_or("default".into());
        let mem = extract_main_field(&json_text, "memory_type").unwrap_or("full".into());
        let status = extract_main_field(&json_text, "status").unwrap_or("idle".into());
        let up = extract_main_field(&json_text, "uptime_sec").unwrap_or("0".into());
        let pid = extract_main_field(&json_text, "pid").unwrap_or("".into());
        let task = extract_main_field(&json_text, "current_task");
        let pending = extract_main_field(&json_text, "pending_intentions").unwrap_or("0".into());

        right_lines.push(Line::from(vec![
            Span::styled("  MAIN NEIL  ", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled(name.clone(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]));
        right_lines.push(Line::from(""));
        right_lines.push(field_line("status", &status, status_color(&status)));
        right_lines.push(field_line("persona", &persona, Color::Rgb(180, 130, 255)));
        right_lines.push(field_line("memory", &mem, Color::Rgb(100, 180, 255)));
        right_lines.push(field_line("uptime", &fmt_duration(&up), Color::White));
        if !pid.is_empty() {
            right_lines.push(field_line("pid", &pid, Color::DarkGray));
        }
        right_lines.push(field_line("pending intentions", &pending, Color::White));
        if let Some(t) = task.as_deref() {
            right_lines.push(Line::from(""));
            right_lines.push(Line::from(Span::styled("  current task:", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))));
            right_lines.push(Line::from(Span::styled(format!("    {}", t), Style::default().fg(Color::White))));
        }
        right_lines.push(Line::from(""));
        right_lines.push(Line::from(Span::styled(
            "  This is the running local Neil. Enter the chat view (Esc, Esc) to interact.",
            Style::default().fg(Color::DarkGray),
        )));
    } else if let Some(t) = temps.get(bounded_sel - 1) {
        right_lines.push(Line::from(vec![
            Span::styled("  TEMP NEIL  ", Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled(t.name.clone(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]));
        right_lines.push(Line::from(""));
        right_lines.push(field_line("status", &t.status, status_color(&t.status)));
        right_lines.push(field_line("persona", &t.persona, Color::Rgb(180, 130, 255)));
        right_lines.push(field_line("memory", &t.memory_type, Color::Rgb(100, 180, 255)));
        right_lines.push(field_line("uptime", &fmt_duration(&t.uptime_sec), Color::White));
        if let Some(pm) = &t.proposed_memories {
            right_lines.push(field_line("proposed memories", &pm.to_string(),
                if *pm > 0 { Color::Rgb(255, 200, 100) } else { Color::DarkGray }));
        }
        if !t.current_task.is_empty() {
            right_lines.push(Line::from(""));
            right_lines.push(Line::from(Span::styled("  current task:", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))));
            for line in textwrap_simple(&t.current_task, 72) {
                right_lines.push(Line::from(Span::styled(format!("    {}", line), Style::default().fg(Color::White))));
            }
        }
        right_lines.push(Line::from(""));
        right_lines.push(Line::from(Span::styled(
            "  Ephemeral instance -- will self-destruct on fulfillment or budget exhaustion.",
            Style::default().fg(Color::DarkGray),
        )));
        right_lines.push(Line::from(""));
        right_lines.push(Line::from(Span::styled(
            "  [note] SSH-into-instance is reserved for Phase 5 VM-based children.",
            Style::default().fg(Color::Rgb(80, 80, 80)),
        )));
    } else {
        right_lines.push(Line::from(Span::styled("  (no instance at this index)", Style::default().fg(Color::DarkGray))));
    }

    // Apply scroll
    let skip = scroll.min(right_lines.len().saturating_sub(1));
    let visible: Vec<Line> = right_lines.into_iter().skip(skip).collect();
    frame.render_widget(Paragraph::new(visible), right_area);

    // FPS
    let fps_text = format!(" {}fps ", fps);
    let fl = fps_text.len() as u16;
    let fx = area.x + area.width.saturating_sub(fl + 1);
    let fy = area.y + area.height - 1;
    frame.render_widget(
        Paragraph::new(Span::styled(fps_text, Style::default().fg(Color::DarkGray))),
        Rect::new(fx, fy, fl, 1),
    );
}

fn field_line(label: &str, value: &str, value_color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {:<20} ", label), Style::default().fg(Color::DarkGray)),
        Span::styled(value.to_string(), Style::default().fg(value_color)),
    ])
}

fn render_cluster_card(
    kind: &str, name: &str, persona: &str, memory: &str, status: &str,
    uptime: &str, task: Option<&str>, border: Color,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  [{}]    ", kind), Style::default().fg(border)),
        Span::styled(status_dot(status), Style::default().fg(status_color(status))),
        Span::raw(" "),
        Span::styled(name.to_string(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" persona={} ", persona), Style::default().fg(Color::Rgb(180, 130, 255))),
        Span::styled(format!("mem={} ", memory), Style::default().fg(Color::Rgb(100, 180, 255))),
        Span::styled(format!("up={}", fmt_duration(uptime)), Style::default().fg(Color::DarkGray)),
        Span::styled(task.map(|t| format!("\n      task: {}", t.chars().take(80).collect::<String>())).unwrap_or_default(), Style::default().fg(Color::White)),
    ])
}

fn status_dot(status: &str) -> &'static str {
    match status {
        "active" => "●",
        "idle" => "○",
        "dying" => "~",
        "error" => "✗",
        _ => "?",
    }
}

fn status_color(status: &str) -> Color {
    match status {
        "active" => Color::Green,
        "idle" => Color::DarkGray,
        "dying" => Color::Yellow,
        "error" => Color::Red,
        _ => Color::DarkGray,
    }
}

fn fmt_duration(sec_str: &str) -> String {
    let sec: u64 = sec_str.parse().unwrap_or(0);
    if sec < 60 { return format!("{}s", sec); }
    if sec < 3600 { return format!("{}m{}s", sec / 60, sec % 60); }
    let h = sec / 3600;
    let m = (sec % 3600) / 60;
    format!("{}h{}m", h, m)
}

/// Temp Neil descriptor parsed from the neil-cluster JSON blob.
struct TempInstance {
    name: String,
    persona: String,
    memory_type: String,
    status: String,
    uptime_sec: String,
    current_task: String,
    proposed_memories: Option<usize>,
}

struct PeerInstance {
    name: String,
    ip: String,
    image: String,
    status: String,
}

/// Handwritten tiny JSON extractor for a top-level string field.
/// Looks for "key":"value" and returns the value.
fn extract_string(json: &str, key: &str) -> Option<String> {
    let idx = json.find(key)?;
    let after = &json[idx + key.len()..];
    // Skip whitespace and ':'
    let colon = after.find(':')?;
    let after = &after[colon + 1..].trim_start();
    let quote = after.find('"')?;
    let start = quote + 1;
    let rest = &after[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn extract_number(json: &str, key: &str) -> Option<usize> {
    let idx = json.find(key)?;
    let after = &json[idx + key.len()..];
    let colon = after.find(':')?;
    let after = &after[colon + 1..].trim_start();
    let mut n = String::new();
    for c in after.chars() {
        if c.is_ascii_digit() { n.push(c); }
        else if !n.is_empty() { break; }
        else if c == '-' || c.is_whitespace() { continue; }
        else { break; }
    }
    n.parse().ok()
}

/// Extract a field from the "main" object specifically.
fn extract_main_field(json: &str, field: &str) -> Option<String> {
    let main_idx = json.find("\"main\"")?;
    let after_main = &json[main_idx..];
    // Find the opening { for main's object
    let brace = after_main.find('{')?;
    let body = &after_main[brace..];
    // Find matching close brace with simple depth counter
    let mut depth = 0;
    let mut end = 0;
    for (i, c) in body.chars().enumerate() {
        match c {
            '{' => depth += 1,
            '}' => { depth -= 1; if depth == 0 { end = i; break; } }
            _ => {}
        }
    }
    if end == 0 { return None; }
    let main_body = &body[..=end];

    let key_pat = format!("\"{}\"", field);
    let key_idx = main_body.find(&key_pat)?;
    let after_key = &main_body[key_idx + key_pat.len()..];
    let colon = after_key.find(':')?;
    let after_colon = after_key[colon + 1..].trim_start();

    if after_colon.starts_with('"') {
        let inner = &after_colon[1..];
        let end_q = inner.find('"')?;
        Some(inner[..end_q].to_string())
    } else if after_colon.starts_with("null") {
        None
    } else {
        // Number or boolean
        let mut out = String::new();
        for c in after_colon.chars() {
            if c.is_alphanumeric() || c == '.' || c == '-' { out.push(c); }
            else { break; }
        }
        if out.is_empty() { None } else { Some(out) }
    }
}

/// Parse the "temps" array into TempInstance records.
fn parse_temps(json: &str) -> Vec<TempInstance> {
    let mut out = Vec::new();
    let Some(temps_idx) = json.find("\"temps\"") else { return out; };
    let after = &json[temps_idx..];
    let Some(bracket) = after.find('[') else { return out; };
    let body = &after[bracket..];
    // Find matching ]
    let mut depth = 0;
    let mut end = 0;
    for (i, c) in body.chars().enumerate() {
        match c {
            '[' | '{' => depth += 1,
            ']' | '}' => { depth -= 1; if depth == 0 { end = i; break; } }
            _ => {}
        }
    }
    if end == 0 { return out; }
    let arr = &body[1..end]; // strip outer brackets

    // Split into objects at top-level }, { boundaries
    let mut cur_depth = 0;
    let mut start = 0;
    let bytes = arr.as_bytes();
    let mut objects: Vec<&str> = Vec::new();
    for i in 0..bytes.len() {
        match bytes[i] {
            b'{' => { if cur_depth == 0 { start = i; } cur_depth += 1; }
            b'}' => { cur_depth -= 1; if cur_depth == 0 { objects.push(&arr[start..=i]); } }
            _ => {}
        }
    }

    for obj in objects {
        let t = TempInstance {
            name: extract_obj_field(obj, "name").unwrap_or_default(),
            persona: extract_obj_field(obj, "persona").unwrap_or("minimal".into()),
            memory_type: extract_obj_field(obj, "memory_type").unwrap_or("ephemeral".into()),
            status: extract_obj_field(obj, "status").unwrap_or("active".into()),
            uptime_sec: extract_obj_field(obj, "uptime_sec").unwrap_or("0".into()),
            current_task: extract_obj_field(obj, "current_task").unwrap_or_default(),
            proposed_memories: extract_obj_field(obj, "proposed_memories").and_then(|s| s.parse().ok()),
        };
        out.push(t);
    }
    out
}

struct ChildCard {
    kind: &'static str,
    name: String,
    status: String,
    line1: String,
    line2: String,
    line3: String,
}

fn prefix_line(l: &Line<'static>, indent: usize) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = vec![Span::raw(" ".repeat(indent))];
    for s in &l.spans { spans.push(s.clone()); }
    Line::from(spans)
}

fn pad_to(s: &str, width: usize) -> String {
    let w = s.chars().count();
    if w >= width { s.chars().take(width).collect() }
    else { format!("{}{}", s, " ".repeat(width - w)) }
}

fn build_main_box(
    name: &str, status: &str, persona: &str, mem: &str,
    up: &str, task: &str, pending: &str, selected: bool,
) -> Vec<Line<'static>> {
    let inner = 46;
    let border_color = if selected { Color::Cyan } else { Color::Rgb(100, 100, 140) };
    let border_mod = if selected { Modifier::BOLD } else { Modifier::empty() };
    let bs = Style::default().fg(border_color).add_modifier(border_mod);

    let label = " MAIN ";
    let lhs_fill = (inner - label.len()) / 2;
    let rhs_fill = inner - label.len() - lhs_fill;
    let mut top = vec![Span::styled("┌".to_string(), bs)];
    top.push(Span::styled("─".repeat(lhs_fill), bs));
    top.push(Span::styled(label.to_string(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
    top.push(Span::styled("─".repeat(rhs_fill), bs));
    top.push(Span::styled("┐".to_string(), bs));
    let top_line = Line::from(top);

    let row = |content: Vec<Span<'static>>, used: usize| -> Line<'static> {
        let mut spans = vec![Span::styled("│".to_string(), bs)];
        for s in content { spans.push(s); }
        if used < inner { spans.push(Span::raw(" ".repeat(inner - used))); }
        spans.push(Span::styled("│".to_string(), bs));
        Line::from(spans)
    };

    let dot = status_dot(status);
    let dot_span = Span::styled(format!(" {} ", dot), Style::default().fg(status_color(status)));
    let name_s = pad_to(name, 18);
    let up_s = fmt_duration(up);

    let l1_used = 3 + 18 + 3 + up_s.chars().count();
    let line1 = row(vec![
        dot_span,
        Span::styled(name_s, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled(" up=".to_string(), Style::default().fg(Color::DarkGray)),
        Span::styled(up_s.clone(), Style::default().fg(Color::Rgb(180, 180, 180))),
    ], l1_used);

    let persona_s = format!(" persona={}", persona);
    let mem_s = format!("  mem={}", mem);
    let pending_s = format!("  pending={}", pending);
    let l2_used = persona_s.chars().count() + mem_s.chars().count() + pending_s.chars().count();
    let line2 = row(vec![
        Span::styled(persona_s, Style::default().fg(Color::Rgb(180, 130, 255))),
        Span::styled(mem_s, Style::default().fg(Color::Rgb(100, 180, 255))),
        Span::styled(pending_s, Style::default().fg(Color::Rgb(255, 200, 100))),
    ], l2_used);

    let task_disp: String = task.chars().take(inner - 8).collect();
    let task_str = format!(" task: {}", task_disp);
    let l3_used = task_str.chars().count();
    let line3 = row(vec![
        Span::styled(task_str, Style::default().fg(Color::White)),
    ], l3_used);

    let bottom = Line::from(vec![
        Span::styled("└".to_string(), bs),
        Span::styled("─".repeat(inner), bs),
        Span::styled("┘".to_string(), bs),
    ]);

    vec![top_line, line1, line2, line3, bottom]
}

fn build_child_box(c: &ChildCard, selected: bool, width: usize) -> Vec<Line<'static>> {
    let inner = width - 2;
    let border_color = if selected { Color::Cyan }
        else if c.kind == "peer" { Color::Rgb(100, 200, 200) }
        else { Color::Rgb(200, 180, 80) };
    let border_mod = if selected { Modifier::BOLD } else { Modifier::empty() };
    let bs = Style::default().fg(border_color).add_modifier(border_mod);

    let label = format!(" {} ", c.kind);
    let lhs_fill = (inner - label.chars().count()) / 2;
    let rhs_fill = inner - label.chars().count() - lhs_fill;
    let label_col = if c.kind == "peer" { Color::Rgb(100, 200, 200) } else { Color::Yellow };
    let mut top = vec![Span::styled("┌".to_string(), bs)];
    top.push(Span::styled("─".repeat(lhs_fill), bs));
    top.push(Span::styled(label, Style::default().fg(label_col).add_modifier(Modifier::BOLD)));
    top.push(Span::styled("─".repeat(rhs_fill), bs));
    top.push(Span::styled("┐".to_string(), bs));

    let row_line = |text: String, color: Color| -> Line<'static> {
        let padded = pad_to(&text, inner);
        Line::from(vec![
            Span::styled("│".to_string(), bs),
            Span::styled(padded, Style::default().fg(color)),
            Span::styled("│".to_string(), bs),
        ])
    };

    let dot = status_dot(&c.status);
    let name_trim: String = c.name.chars().take(inner - 3).collect();
    let name_row = Line::from(vec![
        Span::styled("│".to_string(), bs),
        Span::styled(format!(" {} ", dot), Style::default().fg(status_color(&c.status))),
        Span::styled(pad_to(&name_trim, inner - 3), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled("│".to_string(), bs),
    ]);

    let bottom = Line::from(vec![
        Span::styled("└".to_string(), bs),
        Span::styled("─".repeat(inner), bs),
        Span::styled("┘".to_string(), bs),
    ]);

    vec![
        Line::from(top),
        name_row,
        row_line(format!(" {}", c.line1), Color::Rgb(150, 150, 200)),
        row_line(format!(" {}", c.line2), Color::DarkGray),
        row_line(format!(" {}", c.line3), Color::Rgb(180, 180, 180)),
        bottom,
    ]
}

/// Resolve a cluster-panel selection index to a peer IP, if the selection
/// lands on a peer row. Layout: [MAIN=0] [temps=1..N] [peers=N+1..N+M].
fn peer_ip_at(selection: usize) -> Option<String> {
    let neil_home = env::var("NEIL_HOME").map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env::var("HOME").unwrap_or("/tmp".into())).join(".neil"));
    let bin = neil_home.join("bin/neil-cluster");
    if !bin.exists() { return None; }
    let out = std::process::Command::new(&bin)
        .args(["status", "--json", "--compact"])
        .env("NEIL_HOME", &neil_home)
        .output().ok()?;
    if !out.status.success() { return None; }
    let json = String::from_utf8_lossy(&out.stdout).to_string();
    let temps = parse_temps(&json);
    let peers = parse_peers(&json);
    let peer_start = 1 + temps.len();
    if selection >= peer_start && selection < peer_start + peers.len() {
        let idx = selection - peer_start;
        let ip = peers[idx].ip.clone();
        if ip.is_empty() || ip == "?" { None } else { Some(ip) }
    } else {
        None
    }
}

fn parse_peers(json: &str) -> Vec<PeerInstance> {
    let mut out = Vec::new();
    let Some(idx) = json.find("\"peers\"") else { return out; };
    let after = &json[idx..];
    let Some(bracket) = after.find('[') else { return out; };
    let body = &after[bracket..];
    let mut depth = 0;
    let mut end = 0;
    for (i, c) in body.chars().enumerate() {
        match c {
            '[' | '{' => depth += 1,
            ']' | '}' => { depth -= 1; if depth == 0 { end = i; break; } }
            _ => {}
        }
    }
    if end == 0 { return out; }
    let arr = &body[1..end];
    let mut cur_depth = 0;
    let mut start = 0;
    let bytes = arr.as_bytes();
    let mut objects: Vec<&str> = Vec::new();
    for i in 0..bytes.len() {
        match bytes[i] {
            b'{' => { if cur_depth == 0 { start = i; } cur_depth += 1; }
            b'}' => { cur_depth -= 1; if cur_depth == 0 { objects.push(&arr[start..=i]); } }
            _ => {}
        }
    }
    for obj in objects {
        out.push(PeerInstance {
            name:   extract_obj_field(obj, "name").unwrap_or_default(),
            ip:     extract_obj_field(obj, "ip").unwrap_or_else(|| "?".into()),
            image:  extract_obj_field(obj, "image").unwrap_or_else(|| "?".into()),
            status: extract_obj_field(obj, "status").unwrap_or_else(|| "?".into()),
        });
    }
    out
}

fn extract_obj_field(obj: &str, field: &str) -> Option<String> {
    let key_pat = format!("\"{}\"", field);
    let idx = obj.find(&key_pat)?;
    let after_key = &obj[idx + key_pat.len()..];
    let colon = after_key.find(':')?;
    let after_colon = after_key[colon + 1..].trim_start();

    if after_colon.starts_with('"') {
        let inner = &after_colon[1..];
        let end_q = inner.find('"')?;
        Some(inner[..end_q].to_string())
    } else if after_colon.starts_with("null") {
        None
    } else {
        let mut out = String::new();
        for c in after_colon.chars() {
            if c.is_alphanumeric() || c == '.' || c == '-' { out.push(c); }
            else { break; }
        }
        if out.is_empty() { None } else { Some(out) }
    }
}

// ── Helpers ──

fn load_history(hd: &PathBuf, stream: &mut Vec<StreamEntry>, activity: &mut Vec<String>, count: &mut usize) {
    if let Ok(entries) = fs::read_dir(hd) {
        let mut rf: Vec<_> = entries.filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".result.md")).collect();
        rf.sort_by_key(|e| e.file_name());
        let recent = if rf.len() > 10 { &rf[rf.len()-10..] } else { &rf[..] };
        for entry in recent {
            let fname = entry.file_name().to_string_lossy().to_string();
            if let Ok(c) = fs::read_to_string(entry.path()) {
                let p = extract_between(&c, "## Prompt\n```\n", "\n```");
                let o = extract_between(&c, "## Output\n```\n", "\n```");

                if is_system_prompt(&fname) {
                    // System prompt -> activity panel
                    if let Some(o) = o {
                        // Extract just the HEARTBEAT summary line
                        for line in o.lines() {
                            if line.starts_with("HEARTBEAT:") {
                                if let Some(sum) = line.split("summary=\"").nth(1) {
                                    let sum = sum.trim_end_matches('"');
                                    activity.push(sum.chars().take(60).collect());
                                }
                            }
                        }
                    }
                } else {
                    // User prompt -> chat stream
                    if let Some(p) = p {
                        stream.push(StreamEntry::new(EntryKind::Human, p));
                    }
                    if let Some(o) = o {
                        if !o.is_empty() {
                            stream.push(StreamEntry::new(EntryKind::Neil, o));
                        }
                    }
                }
            }
        }
        *count = rf.len();
    }
}

fn check_new_results(hd: &PathBuf, stream: &mut Vec<StreamEntry>, activity: &mut Vec<String>, count: &mut usize, auto_scroll: &mut bool) {
    if let Ok(entries) = fs::read_dir(hd) {
        let rf: Vec<_> = entries.filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".result.md")).collect();
        if rf.len() > *count {
            let mut sorted: Vec<_> = rf.iter().collect();
            sorted.sort_by_key(|e| e.file_name());
            if let Some(latest) = sorted.last() {
                let fname = latest.file_name().to_string_lossy().to_string();
                if let Ok(c) = fs::read_to_string(latest.path()) {
                    if let Some(o) = extract_between(&c, "## Output\n```\n", "\n```") {
                        if !o.is_empty() {
                            if is_system_prompt(&fname) {
                                // System -> activity panel only
                                for line in o.lines() {
                                    if line.starts_with("HEARTBEAT:") {
                                        if let Some(sum) = line.split("summary=\"").nth(1) {
                                            let sum = sum.trim_end_matches('"');
                                            activity.push(sum.chars().take(60).collect());
                                            if activity.len() > 20 { activity.drain(..10); }
                                        }
                                    }
                                }
                            } else {
                                // User chat -> stream
                                if let Some(last) = stream.last() {
                                    if matches!(last.kind, EntryKind::System) {
                                        if last.blocks.first().map(|b| matches!(b, RichBlock::Text(t) if t.contains("sending to neil") || t.contains("thinking") || t.contains("queued"))).unwrap_or(false) {
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
            }
            *count = rf.len();
        }
    }
}

fn is_system_prompt(filename: &str) -> bool {
    filename.contains("heartbeat") || filename.contains("wakeup")
        || filename.contains("_sched_") || filename.contains("_fs_")
        || filename.contains("_webhook") || filename.contains("_mirror_")
        || filename.contains("_vision")
}

fn extract_between(c: &str, start: &str, end: &str) -> Option<String> {
    let s = c.find(start)? + start.len();
    let e = c[s..].find(end)? + s;
    Some(c[s..e].trim().to_string())
}
