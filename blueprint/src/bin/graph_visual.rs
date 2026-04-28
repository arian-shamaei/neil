// Visual proof: trigger two REAL zettel ops against the live palace
// (one write that affects existing nodes, one read on an existing
// node), let the watcher pick them up, render once, then print an
// ASCII map of just the flashing cells:
//
//   R  = cell rendered in the red gradient (write)
//   G  = cell rendered in the green gradient (read)
//   ·  = stationary cell with any node glyph
//   (space) = no node here
//
// Also lists each flashing cell's (col, row, glyph, RGB, kind) so the
// reader can match a position to the underlying note id by visual
// inspection of the topology rendered immediately afterward.
//
// Run: cargo run --release --bin graph_visual

use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use neil_blueprint::panels::graph;

fn main() {
    let neil_home = env::var("NEIL_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let h = env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            PathBuf::from(h).join(".neil")
        });
    let zettel = neil_home.join("memory/zettel/zettel");
    let palace = neil_home.join("memory/palace");

    graph::spawn_graph_refresher(neil_home.clone());
    graph::spawn_access_watcher(neil_home.clone());

    let w: u16 = 80;
    let h: u16 = 30;

    // Wait for the loader thread to populate the cache, and run enough
    // render frames that physics settles.
    for _ in 0..30 {
        let _ = graph::render_lines(w, h);
        if graph::node_count() > 0 { break; }
        std::thread::sleep(Duration::from_millis(200));
    }
    if graph::node_count() == 0 {
        eprintln!("FAIL: graph never populated");
        std::process::exit(1);
    }
    for _ in 0..200 { let _ = graph::render_lines(w, h); }

    // Pick three real existing notes from the palace.
    let notes_dir = palace.join("notes");
    let mut existing_ids: Vec<String> = std::fs::read_dir(&notes_dir).ok()
        .map(|d| d.filter_map(|e| e.ok())
            .filter_map(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                if n.ends_with(".md") { Some(n.trim_end_matches(".md").to_string()) }
                else { None }
            })
            .collect())
        .unwrap_or_default();
    existing_ids.sort();
    if existing_ids.len() < 3 {
        eprintln!("FAIL: need ≥3 notes in palace, got {}", existing_ids.len());
        std::process::exit(2);
    }
    let id_a = &existing_ids[0];
    let id_b = &existing_ids[existing_ids.len() / 2];
    let id_c = &existing_ids[existing_ids.len() - 1];

    println!("--- triggering ops ---");
    println!("  WRITE: zettel link {} {}", id_a, id_b);
    let _ = Command::new(&zettel)
        .args(["link", id_a, id_b])
        .env("ZETTEL_HOME", &palace).output();
    std::thread::sleep(Duration::from_millis(150));
    println!("  READ:  zettel show {}", id_c);
    let _ = Command::new(&zettel)
        .args(["show", id_c])
        .env("ZETTEL_HOME", &palace).output();
    std::thread::sleep(Duration::from_millis(800));

    // Single render frame for visual analysis.
    let lines = graph::render_lines(w, h);

    // Build the proof grid + cell report.
    println!("\n--- visual proof grid (R=write-flash, G=read-flash) ---");
    let mut writes: Vec<(usize, usize, char, u8, u8, u8)> = Vec::new();
    let mut reads:  Vec<(usize, usize, char, u8, u8, u8)> = Vec::new();
    let mut ascii_rows: Vec<String> = Vec::with_capacity(lines.len());
    for (row, line) in lines.iter().enumerate() {
        let mut row_txt = String::with_capacity(w as usize);
        let mut col = 0usize;
        for span in &line.spans {
            let rgb = match span.style.fg {
                Some(ratatui::style::Color::Rgb(r, g, b)) => Some((r, g, b)),
                _ => None,
            };
            for ch in span.content.chars() {
                let (cell, classified) = match rgb {
                    Some((r, g, b)) if r > 200 && g < 100 && b < 100 => {
                        if !ch.is_whitespace() {
                            writes.push((col, row, ch, r, g, b));
                        }
                        ('R', true)
                    }
                    Some((r, g, b)) if g > 200 && r < 120 && b < 140 => {
                        if !ch.is_whitespace() {
                            reads.push((col, row, ch, r, g, b));
                        }
                        ('G', true)
                    }
                    _ => (
                        if ch == ' ' { ' ' }
                        else if "·•●".contains(ch) { '·' }
                        else { '·' },
                        false,
                    ),
                };
                let _ = classified;
                row_txt.push(cell);
                col += 1;
            }
        }
        ascii_rows.push(row_txt);
    }

    for r in &ascii_rows { println!("{}", r); }

    println!("\n--- flashing cells (col,row glyph rgb) ---");
    for (c, r, g, rr, gg, bb) in &writes {
        println!("  WRITE  ({:2},{:2})  glyph={}  rgb=({:3},{:3},{:3})",
                 c, r, g, rr, gg, bb);
    }
    for (c, r, g, rr, gg, bb) in &reads {
        println!("  READ   ({:2},{:2})  glyph={}  rgb=({:3},{:3},{:3})",
                 c, r, g, rr, gg, bb);
    }

    println!("\nsummary: {} red-flashed cells (writes), {} green-flashed cells (reads)",
             writes.len(), reads.len());

    if writes.is_empty() && reads.is_empty() {
        eprintln!("FAIL: no flashing cells at all — pipeline broken");
        std::process::exit(3);
    }
    if reads.is_empty() {
        eprintln!("FAIL: no green-flashed cells — read path not visible");
        std::process::exit(4);
    }
    if writes.is_empty() {
        eprintln!(
            "WARN: no red-flashed cells — both linked ids may not be in the\n\
             current loaded graph (cache lags 30s); rerun and the link write\n\
             will appear once the next refresh ingests the new edge");
    }
    println!("PASS");
}
