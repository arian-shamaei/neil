mod panel;
mod state;
mod panels;

use std::env;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::Terminal;

use panel::Panel;
use state::NeilState;
use panels::header::HeaderPanel;
use panels::heartbeat::HeartbeatPanel;
use panels::memory::MemoryPanel;
use panels::intentions::IntentionsPanel;
use panels::system::SystemPanel;
use panels::status::StatusPanel;

fn main() -> anyhow::Result<()> {
    // Resolve NEIL_HOME
    let neil_home = env::var("NEIL_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(home).join(".neil")
        });

    // Setup terminal
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Build panels (the "cartridges")
    let header = HeaderPanel;
    let heartbeat = HeartbeatPanel;
    let memory = MemoryPanel;
    let intentions = IntentionsPanel;
    let system = SystemPanel;
    let status = StatusPanel;

    // Main loop
    let tick_rate = Duration::from_millis(500);
    let mut last_tick = Instant::now();
    let mut tick: u64 = 0;

    loop {
        // Load state from files
        let mut state = NeilState::load(&neil_home);
        state.tick = tick;

        // Render
        terminal.draw(|frame| {
            let size = frame.area();

            // Layout: header (1 line) | content grid | status (1 line)
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),  // header
                    Constraint::Min(3),     // content
                    Constraint::Length(1),  // status bar
                ])
                .split(size);

            // Header
            header.render(chunks[0], frame.buffer_mut(), &state);

            // Content: 2x2 grid
            let content = chunks[1];
            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(50),
                    Constraint::Percentage(50),
                ])
                .split(content);

            let top_cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(50),
                    Constraint::Percentage(50),
                ])
                .split(rows[0]);

            let bottom_cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(50),
                    Constraint::Percentage(50),
                ])
                .split(rows[1]);

            // Render panels in grid
            heartbeat.render(top_cols[0], frame.buffer_mut(), &state);
            memory.render(top_cols[1], frame.buffer_mut(), &state);
            intentions.render(bottom_cols[0], frame.buffer_mut(), &state);
            system.render(bottom_cols[1], frame.buffer_mut(), &state);

            // Status bar
            status.render(chunks[2], frame.buffer_mut(), &state);
        })?;

        // Handle input
        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('r') => { /* force refresh on next tick */ }
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

    // Cleanup
    terminal::disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    Ok(())
}
