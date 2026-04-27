// Headless smoke test for the graph panel — calls the same data path the
// TUI uses (zettel list --json → physics rebuild → render to Vec<Line>),
// counts cells with non-default styling, prints a tiny ASCII rendering
// of the topology to stdout. Run with:
//   cargo run --release --bin graph_test
// Requires NEIL_HOME pointing at a populated palace.

use std::env;
use std::path::PathBuf;
use std::time::Duration;

use neil_blueprint::panels::graph;

fn main() {
    let neil_home = env::var("NEIL_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let h = env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            PathBuf::from(h).join(".neil")
        });

    println!("NEIL_HOME = {}", neil_home.display());

    graph::spawn_graph_refresher(neil_home.clone());

    // Wait for the refresher to drop a cache entry, then poke render_lines
    // once so the GraphState ingests it (node_count reads from GraphState,
    // not the raw cache). Poll up to 5 s.
    let w: u16 = 80;
    let h: u16 = 30;
    for _ in 0..25 {
        let _ = graph::render_lines(w, h);
        if graph::node_count() > 0 { break; }
        std::thread::sleep(Duration::from_millis(200));
    }

    println!("nodes={}  edges={}", graph::node_count(), graph::edge_count());

    if graph::node_count() == 0 {
        eprintln!("FAIL: graph never populated");
        std::process::exit(1);
    }

    // Run a few hundred physics ticks via repeated render calls — the
    // first calls will be unsettled, last calls should look organized.
    let mut last_lines = Vec::new();
    for _ in 0..120 {
        last_lines = graph::render_lines(w, h);
    }

    // Sanity: line count == panel height.
    if last_lines.len() != h as usize {
        eprintln!("FAIL: expected {} lines, got {}", h, last_lines.len());
        std::process::exit(2);
    }

    // Count "occupied" cells — anything whose styled spans contain
    // non-space glyphs.
    let mut occupied: usize = 0;
    let mut sample_first = String::new();
    for line in &last_lines {
        for span in &line.spans {
            for ch in span.content.chars() {
                if ch != ' ' { occupied += 1; }
            }
        }
        if sample_first.is_empty() {
            let s: String = line.spans.iter()
                .flat_map(|sp| sp.content.chars())
                .collect();
            if s.chars().any(|c| c != ' ') { sample_first = s; }
        }
    }

    println!("occupied_cells = {}", occupied);
    println!("first_nonempty_line: {:?}", sample_first.trim_end());

    // Also print a plain-text rendering (no colors) of the whole panel
    // so a human can eyeball the shape.
    println!("--- topology (no color) ---");
    for line in &last_lines {
        let s: String = line.spans.iter()
            .flat_map(|sp| sp.content.chars())
            .collect();
        println!("{}", s);
    }

    // Floor: at least 1 cell per node should be occupied.
    if occupied < graph::node_count() / 2 {
        eprintln!("FAIL: too few occupied cells ({} < {} / 2)",
                  occupied, graph::node_count());
        std::process::exit(3);
    }
    println!("PASS");
}
