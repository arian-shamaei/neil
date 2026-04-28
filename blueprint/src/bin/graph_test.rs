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

    println!("nodes={}  edges={}  explicit={}  orphans={}  Q={:.3}",
             graph::node_count(),
             graph::edge_count(),
             graph::explicit_count(),
             graph::orphan_count(),
             graph::modularity());

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

    // Sanity check anchor toggle: Q is cached at rebuild but anchor
    // changes should at least visibly reorganize the layout. Toggle on,
    // run more ticks, render again, then toggle off.
    println!("--- anchors=free  Q={:.3} ---", graph::modularity());
    let _ = graph::toggle_anchors();
    println!("anchor_strength now = {}", graph::anchor_strength());
    for _ in 0..120 {
        last_lines = graph::render_lines(w, h);
    }
    let _ = graph::toggle_anchors();
    println!("anchor_strength now = {}", graph::anchor_strength());
    for _ in 0..120 {
        last_lines = graph::render_lines(w, h);
    }
    let _ = graph::toggle_anchors();  // back to 0
    println!("anchor_strength now = {} (back to free)", graph::anchor_strength());
    for _ in 0..120 {
        last_lines = graph::render_lines(w, h);
    }

    // ── Phase 4: real-time access flash ─────────────────────────────────
    // Spawn the access watcher, trigger a zettel show, wait long enough
    // for the watcher to pick up the new line, render, then count cells
    // whose fg color is in the "red flash" region (R high, G+B low).
    println!("--- access flash: triggering zettel show ---");
    graph::spawn_access_watcher(neil_home.clone());
    std::thread::sleep(Duration::from_millis(300));

    // Pick the first note in the palace and zettel show it.
    let zettel_bin = neil_home.join("memory/zettel/zettel");
    let palace_dir = neil_home.join("memory/palace");
    let notes_dir = palace_dir.join("notes");
    let first_note = std::fs::read_dir(&notes_dir).ok()
        .and_then(|d| d.filter_map(|e| e.ok())
            .filter_map(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                if n.ends_with(".md") { Some(n.trim_end_matches(".md").to_string()) }
                else { None }
            })
            .next());
    let Some(target_id) = first_note else {
        eprintln!("FAIL: no note files in palace, can't test access flash");
        std::process::exit(4);
    };
    // Trigger one WRITE (zettel new) and one READ (zettel show) in
    // sequence. Two distinct fresh accesses; render should produce
    // both red and green cells.
    println!("triggering: zettel new \"smoke-write-{}\"", target_id);
    let _ = std::process::Command::new(&zettel_bin)
        .arg("new").arg(&format!("smoke-test-write {}", std::process::id()))
        .env("ZETTEL_HOME", &palace_dir)
        .output();
    println!("triggering: zettel show {} (read)", target_id);
    let _ = std::process::Command::new(&zettel_bin)
        .arg("show").arg(&target_id)
        .env("ZETTEL_HOME", &palace_dir)
        .output();
    // Watcher polls every 250ms — give it ~700ms for both events.
    std::thread::sleep(Duration::from_millis(700));

    // Trigger one extra render so the watcher's id (smoke-write) — a
    // brand-new note that wasn't in the graph at startup — gets picked
    // up. We need to also force a graph rebuild. The easiest way:
    // request a rebuild by reseeding (which re-runs zettel list --json
    // implicitly via cache version bump on next refresher pass — but
    // refresher only runs every 30s). Skip the smoke-write check —
    // even without rebuild, the existing target_id WAS read so its
    // green flash is enough proof of the read path.
    let flashed_lines = graph::render_lines(w, h);

    // Count cells by gradient channel.
    let mut red_cells = 0usize;
    let mut green_cells = 0usize;
    for line in &flashed_lines {
        for span in &line.spans {
            if let Some(ratatui::style::Color::Rgb(r, g, b)) = span.style.fg {
                if r > 200 && g < 100 && b < 100 {
                    red_cells += span.content.chars().count();
                }
                // Read-flash green: G dominant, R lower, B moderate.
                if g > 200 && r < 120 && b < 140 {
                    green_cells += span.content.chars().count();
                }
            }
        }
    }
    println!("red_write_flash_cells  = {}", red_cells);
    println!("green_read_flash_cells = {}", green_cells);
    // The new note isn't in the loaded graph yet (cache refreshes every
    // 30s) — its id won't match any rendered node, so its flash won't
    // appear. The READ is on an existing note (target_id) so green is
    // visible. Assert green ≥ 1; red is best-effort.
    if green_cells == 0 {
        eprintln!("FAIL: no green-flashed cells — read color path not active");
        std::process::exit(5);
    }

    // ── Phase 5: trail mode ─────────────────────────────────────────────
    // In flash mode (3s window), an access from 5s ago is invisible.
    // In trail mode (60s window), the same access should still render
    // somewhere in the orange-to-amber gradient (R high, G mid, B low).
    println!("--- access trail: aging the flash, then toggling l ---");
    // Wait long enough that the live 3s flash has decayed to invisible.
    std::thread::sleep(Duration::from_millis(3500));

    // Re-render in flash mode — should be no red cells now.
    let flash_decayed = graph::render_lines(w, h);
    let mut still_flashing = 0usize;
    for line in &flash_decayed {
        for span in &line.spans {
            if let Some(ratatui::style::Color::Rgb(r, g, b)) = span.style.fg {
                if r > 200 && g < 100 && b < 100 {
                    still_flashing += span.content.chars().count();
                }
            }
        }
    }
    println!("flash_mode_after_3.5s_decay = {} red cells (expect 0)", still_flashing);

    // Toggle trail and render. The previously-flashed READ should
    // re-appear with a fade color in the green-teal band (the read
    // gradient at ~4s elapsed: roughly R 90, G 215, B 130).
    let new = graph::toggle_trail();
    println!("trail_enabled now = {}", new);
    let trail_view = graph::render_lines(w, h);
    let mut trail_cells = 0usize;
    for line in &trail_view {
        for span in &line.spans {
            if let Some(ratatui::style::Color::Rgb(r, g, b)) = span.style.fg {
                // Read-trail green-teal: G dominant, R low/moderate.
                if g > 180 && r < 140 && b > 90 && b < 180 {
                    trail_cells += span.content.chars().count();
                }
            }
        }
    }
    println!("trail_mode_aged_read_cells = {} (expect ≥1)", trail_cells);
    if trail_cells == 0 {
        eprintln!("FAIL: trail mode shows no aged-read cells");
        std::process::exit(6);
    }

    // ── Phase 6: matrix view ────────────────────────────────────────────
    println!("--- matrix view ---");
    let _ = graph::toggle_matrix_view();
    let matrix_lines = graph::render_matrix_lines(80, 30);
    println!("matrix line count = {}", matrix_lines.len());
    let mut blue_cells = 0usize;
    for line in &matrix_lines {
        for span in &line.spans {
            if let Some(ratatui::style::Color::Rgb(r, g, b)) = span.style.bg {
                // Heatmap cells: blue-dominant
                if b > r && b > g {
                    blue_cells += span.content.chars().count();
                }
            }
        }
    }
    println!("blue_heatmap_cells = {} (expect > 0)", blue_cells);
    if blue_cells == 0 {
        eprintln!("FAIL: matrix view rendered no heatmap cells");
        std::process::exit(7);
    }
    if let Some((a, b, w)) = graph::top_cross_wing_pair() {
        println!("top_cross_wing_pair: {} ↔ {} (weight={:.2})", a, b, w);
    } else {
        println!("(no cross-wing pairs)");
    }
    println!("--- matrix rendered (text only) ---");
    for line in &matrix_lines {
        let s: String = line.spans.iter()
            .flat_map(|sp| sp.content.chars())
            .collect();
        println!("{}", s);
    }
    let _ = graph::toggle_matrix_view();  // back to graph view

    let _ = last_lines;

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

    // Sanity floor — settled layouts should occupy a meaningful fraction
    // of the panel. We don't require occupied ≥ n because clustered
    // layouts (high modularity OR strong tag co-occurrence) intentionally
    // overlap multiple nodes onto the same cell — overlap is success,
    // not failure. 100 cells is enough to reject "everything pinned to
    // one corner" pathologies without rejecting clustering.
    if occupied < 100 {
        eprintln!("FAIL: too few occupied cells ({}); layout never settled?",
                  occupied);
        std::process::exit(3);
    }
    println!("PASS");
}
