use std::io;
use std::time::Duration;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;

use neil_blueprint::seal::{self, SealPose};

fn main() -> anyhow::Result<()> {
    let neil_home = std::env::var("NEIL_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or("/tmp".into());
            std::path::PathBuf::from(home).join(".neil")
        });

    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut tick: u64 = 0;

    loop {
        let pose = SealPose::load(&neil_home);
        let seal_lines = seal::render_seal(&pose, tick);

        terminal.draw(|frame| {
            let size = frame.area();

            let seal_w = 28_u16;
            let seal_h = (seal_lines.len() as u16 + 2).min(size.height);
            let x = (size.width.saturating_sub(seal_w)) / 2;
            let y = (size.height.saturating_sub(seal_h)) / 2;
            let area = Rect::new(x, y, seal_w, seal_h);

            let lines: Vec<Line> = seal_lines.iter()
                .map(|l| Line::from(Span::styled(l.clone(), Style::default().fg(Color::Cyan))))
                .collect();

            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(format!(" seal | tick {} | q:quit ", tick));

            frame.render_widget(Paragraph::new(lines).block(block), area);

            let info = Line::from(Span::styled(
                " Edit ~/.neil/.seal_pose.json to change pose live ",
                Style::default().fg(Color::DarkGray),
            ));
            if y + seal_h + 1 < size.height {
                frame.render_widget(Paragraph::new(info),
                    Rect::new(0, y + seal_h + 1, size.width, 1));
            }
        })?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        _ => {}
                    }
                }
            }
        }

        tick += 1;
    }

    terminal::disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
