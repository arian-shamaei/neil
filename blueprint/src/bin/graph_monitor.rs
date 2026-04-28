// Active 40-second observer of the graph panel. Watches .access.jsonl
// for events, drives render frames at 2Hz so physics + flash decay
// progress, and emits a per-event timeline + stats + final topology
// snapshot.
//
// This is OBSERVATION-ONLY — it doesn't trigger zettel/mempalace ops.
// If the palace is idle (no heartbeat firing, no user-driven chat),
// we see zero events; that emptiness is itself a data point.

use std::env;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use neil_blueprint::panels::graph;
use serde::Deserialize;

#[derive(Deserialize, Default)]
struct AccessEvent {
    id: String,
    #[serde(default)] op: String,
}

const W: u16 = 80;
const H: u16 = 30;
const DURATION_S: u64 = 40;
const POLL_MS: u64 = 500;

fn main() {
    let neil_home = env::var("NEIL_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let h = env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            PathBuf::from(h).join(".neil")
        });

    println!("=== graph_monitor: {} second active observation ===\n",
             DURATION_S);

    graph::spawn_graph_refresher(neil_home.clone());
    graph::spawn_access_watcher(neil_home.clone());

    // Wait for cache → state to populate, settle physics.
    for _ in 0..30 {
        let _ = graph::render_lines(W, H);
        if graph::node_count() > 0 { break; }
        std::thread::sleep(Duration::from_millis(200));
    }
    if graph::node_count() == 0 {
        eprintln!("FAIL: graph never populated"); std::process::exit(1);
    }
    for _ in 0..120 { let _ = graph::render_lines(W, H); }

    println!("graph state at start:");
    println!("  nodes={}  edges={}  explicit_links={}  orphans={}  Q={:.3}",
             graph::node_count(),
             graph::edge_count(),
             graph::explicit_count(),
             graph::orphan_count(),
             graph::modularity());
    println!();

    // Tail .access.jsonl from end-of-file (skip history).
    let log_path = neil_home.join("memory/palace/.access.jsonl");
    let mut last_pos: u64 = std::fs::metadata(&log_path)
        .map(|m| m.len()).unwrap_or(0);

    let start = Instant::now();
    let mut timeline: Vec<(f32, String, String)> = Vec::new();
    let mut flash_density: Vec<(f32, usize, usize)> = Vec::new();
    let mut last_flash_print: Option<f32> = None;

    println!("event timeline (events emitted as they arrive):\n");
    println!("  T+sec  op           note_id");
    println!("  -----  -----------  --------------------");

    while start.elapsed() < Duration::from_secs(DURATION_S) {
        // Drive a render frame — physics steps, watcher map → render.
        let lines = graph::render_lines(W, H);

        // Count flash cells in this frame.
        let mut red = 0usize;
        let mut green = 0usize;
        for line in &lines {
            for span in &line.spans {
                if let Some(ratatui::style::Color::Rgb(r, g, b)) = span.style.fg {
                    if r > 200 && g < 100 && b < 100 {
                        red += span.content.chars().count();
                    }
                    if g > 200 && r < 120 && b < 140 {
                        green += span.content.chars().count();
                    }
                }
            }
        }
        let elapsed = start.elapsed().as_secs_f32();
        flash_density.push((elapsed, red, green));

        // Tail access log.
        if let Ok(meta) = std::fs::metadata(&log_path) {
            if meta.len() > last_pos {
                if let Ok(mut file) = File::open(&log_path) {
                    if file.seek(SeekFrom::Start(last_pos)).is_ok() {
                        let mut buf = String::new();
                        if file.read_to_string(&mut buf).is_ok() {
                            last_pos = meta.len();
                            for line in buf.lines() {
                                if line.trim().is_empty() { continue; }
                                if let Ok(ev) = serde_json::from_str::<AccessEvent>(line) {
                                    let trunc_id: String = ev.id.chars().take(20).collect();
                                    println!("  {:>5.1}  {:<11}  {}",
                                             elapsed, ev.op, trunc_id);
                                    timeline.push((elapsed, ev.op, ev.id));
                                }
                            }
                        }
                    }
                }
            }
        }

        // Print a flash-density tick every ~5 seconds even if no events.
        let print_tick = (elapsed / 5.0).floor();
        let last_tick = last_flash_print.map(|t| (t / 5.0).floor()).unwrap_or(-1.0);
        if print_tick > last_tick && elapsed >= 5.0 {
            if red + green > 0 {
                println!("  {:>5.1}  [flash]      red={} green={}",
                         elapsed, red, green);
            }
            last_flash_print = Some(elapsed);
        }

        std::thread::sleep(Duration::from_millis(POLL_MS));
    }

    // Summary.
    println!("\n=== summary ({:.1}s observed) ===", start.elapsed().as_secs_f32());
    println!("  total events: {}", timeline.len());
    let mut by_op: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (_, op, _) in &timeline { *by_op.entry(op.clone()).or_insert(0) += 1; }
    let mut by_op_sorted: Vec<_> = by_op.iter().collect();
    by_op_sorted.sort_by(|a, b| b.1.cmp(a.1));
    for (op, n) in by_op_sorted {
        println!("  {:<11} {} events", op, n);
    }

    let total_flash_frames = flash_density.iter().filter(|(_, r, g)| r + g > 0).count();
    println!("  frames with any flash visible: {} / {}",
             total_flash_frames, flash_density.len());

    let max_red = flash_density.iter().map(|(_,r,_)| *r).max().unwrap_or(0);
    let max_green = flash_density.iter().map(|(_,_,g)| *g).max().unwrap_or(0);
    println!("  peak red cells: {}, peak green cells: {}", max_red, max_green);

    // Final topology snapshot — text only, R/G/. classification.
    println!("\n=== final frame topology (R=write, G=read, ·=stationary) ===");
    let lines = graph::render_lines(W, H);
    for line in &lines {
        let mut row = String::new();
        for span in &line.spans {
            let cls = match span.style.fg {
                Some(ratatui::style::Color::Rgb(r, g, b)) if r > 200 && g < 100 && b < 100 => 'R',
                Some(ratatui::style::Color::Rgb(r, g, b)) if g > 200 && r < 120 && b < 140 => 'G',
                _ => '.',
            };
            for ch in span.content.chars() {
                row.push(if ch == ' ' { ' ' } else { cls });
            }
        }
        println!("{}", row);
    }
}
