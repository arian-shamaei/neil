// Topology panel — force-directed layout of every note in the palace.
//
// Phase 1: free Fruchterman-Reingold (no wing anchors), O(n²) repulsion +
// Hooke springs along bidirectional links, anisotropic Y-stretch to fix
// terminal-cell aspect ratio, two render layers — edge density as
// background tint, nodes as foreground glyphs colored by wing with degree
// driving glyph size and labeling. Background thread re-runs `zettel list
// --json` every 30 s; render thread steps physics each frame.
//
// Position carries information: clusters are notes Neil keeps thinking
// about; bridges are notes that span domains; hubs are landmarks.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use serde::Deserialize;

#[derive(Deserialize, Default)]
struct NoteRaw {
    id: String,
    #[serde(default)] wing: String,
    #[serde(default)] room: String,
    #[serde(default)] preview: String,
    #[serde(default)] tags: Vec<String>,
    #[serde(default)] links: Vec<String>,
}

#[derive(Deserialize, Default)]
struct ListJson { notes: Vec<NoteRaw> }

#[derive(Default, Clone)]
struct Node {
    id: String,
    wing: String,
    preview: String,
    degree: usize,
    pos: (f32, f32),
    vel: (f32, f32),
    force: (f32, f32),
}

/// Weighted edge between two nodes. Weight is the spring stiffness scaling
/// factor in the F-R attraction formula. Explicit `zettel link` edges
/// carry a fixed high weight (EXPLICIT_LINK_WEIGHT); implicit tag-cooccur
/// edges carry a smaller weight that's the sum of `1/log(tag_size+2)`
/// over all tags shared by the two notes (so rare shared tags pull more
/// strongly than common ones — single-note tags can't form edges, and
/// tags with > MAX_TAG_FANOUT notes are skipped entirely as too generic
/// to encode meaningful similarity).
#[derive(Default, Clone, Copy)]
struct WEdge { i: usize, j: usize, weight: f32, explicit: bool }

const EXPLICIT_LINK_WEIGHT: f32 = 3.0;
const MAX_TAG_FANOUT: usize = 30;

#[derive(Default)]
pub struct GraphState {
    nodes: Vec<Node>,
    edges: Vec<WEdge>,
    /// Modularity Q computed with wings as the community partition,
    /// over the weighted edge graph. Cached from last rebuild.
    modularity: f32,
    /// Notes with no edges at all (no shared tag, no explicit link).
    orphan_count: usize,
    /// Number of explicit (zettel link) edges — the rare curated bridges.
    explicit_count: usize,
    /// Strength of the wing-centroid anchor force. 0.0 = pure free
    /// physics (the topology Neil's tags+links say); 0.3 = soft
    /// anchor (wings bias clustering); 0.6 = strong anchor (wings
    /// dominate, layout shows wing taxonomy). Cycles via `s` key.
    anchor_strength: f32,
    settle_ticks: u32,
    last_load_version: u64,
    seed_state: u64,
}

static GRAPH_STATE: OnceLock<Arc<Mutex<GraphState>>> = OnceLock::new();
static GRAPH_CACHE: OnceLock<Arc<Mutex<(u64, Option<String>)>>> = OnceLock::new();

fn graph_state() -> &'static Arc<Mutex<GraphState>> {
    GRAPH_STATE.get_or_init(|| Arc::new(Mutex::new(GraphState::default())))
}

fn graph_cache() -> &'static Arc<Mutex<(u64, Option<String>)>> {
    GRAPH_CACHE.get_or_init(|| Arc::new(Mutex::new((0, None))))
}

/// Spawn a thread that re-runs `zettel list --json` every 30 s and stores
/// the output in the cache. Render reads from the cache without blocking
/// the loader, and rebuilds physics state only when the version bumps.
pub fn spawn_graph_refresher(neil_home: PathBuf) {
    let zettel_bin = neil_home.join("memory/zettel/zettel");
    if !zettel_bin.exists() { return; }
    let palace_dir = neil_home.join("memory/palace");
    let cache = graph_cache().clone();

    std::thread::spawn(move || {
        loop {
            let output = std::process::Command::new(&zettel_bin)
                .arg("list").arg("--json")
                .env("ZETTEL_HOME", &palace_dir)
                .output();
            if let Ok(o) = output {
                if o.status.success() {
                    let json = String::from_utf8_lossy(&o.stdout).to_string();
                    if let Ok(mut g) = cache.lock() {
                        g.0 = g.0.wrapping_add(1);
                        g.1 = Some(json);
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(30));
        }
    });
}

// Stable per-wing color via DJB2 hash → small palette.
fn wing_color(wing: &str) -> Color {
    if wing.is_empty() { return Color::DarkGray; }
    let mut h: u32 = 5381;
    for b in wing.bytes() { h = h.wrapping_mul(33).wrapping_add(b as u32); }
    const PALETTE: [Color; 8] = [
        Color::Cyan, Color::Magenta, Color::Yellow, Color::Green,
        Color::Blue, Color::LightCyan, Color::LightYellow, Color::LightGreen,
    ];
    PALETTE[(h as usize) % PALETTE.len()]
}

// xorshift64* — small, deterministic, zero-dep RNG used to seed initial
// positions. Pure Rust because the existing blueprint Cargo.toml has no
// rand crate and we're keeping it that way.
fn xorshift(state: &mut u64) -> u64 {
    if *state == 0 { *state = 0x_dead_beef_dead_beef; }
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}
fn xrand_unit(state: &mut u64) -> f32 {
    (xorshift(state) >> 32) as f32 / (u32::MAX as f32)
}

impl GraphState {
    fn rebuild_from_json(&mut self, json: &str) {
        let parsed: ListJson = match serde_json::from_str(json) {
            Ok(p) => p,
            Err(_) => return,
        };

        // Bump seed each rebuild — re-arms physics so freshly-added notes
        // visibly settle rather than appearing pre-placed.
        self.seed_state = self.seed_state.wrapping_add(1).wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let mut rng = self.seed_state;

        let mut nodes: Vec<Node> = Vec::with_capacity(parsed.notes.len());
        let mut id_to_idx: HashMap<String, usize> = HashMap::with_capacity(parsed.notes.len());
        for raw in &parsed.notes {
            id_to_idx.insert(raw.id.clone(), nodes.len());
            nodes.push(Node {
                id: raw.id.clone(),
                wing: raw.wing.clone(),
                preview: raw.preview.clone(),
                degree: 0,
                pos: (xrand_unit(&mut rng) * 20.0 - 10.0,
                      xrand_unit(&mut rng) * 20.0 - 10.0),
                vel: (0.0, 0.0),
                force: (0.0, 0.0),
            });
        }

        // Combined weighted edge map keyed on (min, max) — explicit
        // and implicit edges between the same pair accumulate weight.
        let mut edge_weights: HashMap<(usize, usize), (f32, bool)> = HashMap::new();
        let mut explicit_count = 0usize;

        // Pass 1: explicit `zettel link` edges. These are the curated
        // bridges Neil consciously created — fixed high weight.
        for raw in &parsed.notes {
            let Some(&i) = id_to_idx.get(&raw.id) else { continue };
            for link in &raw.links {
                let Some(&j) = id_to_idx.get(link) else { continue };
                if i == j { continue; }
                let key = if i < j { (i, j) } else { (j, i) };
                let entry = edge_weights.entry(key).or_insert((0.0, false));
                if !entry.1 {
                    entry.1 = true;
                    explicit_count += 1;
                }
                entry.0 += EXPLICIT_LINK_WEIGHT;
            }
        }

        // Pass 2: implicit tag-cooccurrence edges. Build tag → notes map,
        // skip tags above MAX_TAG_FANOUT (too generic), and accumulate
        // 1/log2(k+2) per shared tag onto each pair.
        let mut tag_to_notes: HashMap<String, Vec<usize>> = HashMap::new();
        for raw in &parsed.notes {
            let Some(&i) = id_to_idx.get(&raw.id) else { continue };
            for tag in &raw.tags {
                if tag.is_empty() { continue; }
                tag_to_notes.entry(tag.clone()).or_default().push(i);
            }
        }
        for (_tag, members) in &tag_to_notes {
            let k = members.len();
            if k < 2 || k > MAX_TAG_FANOUT { continue; }
            // Rarity weight: rare tags (small k) pull harder. log2(k+2)
            // gives 2.0 at k=2, ~3.5 at k=10, ~5.0 at k=30.
            let rarity = 1.0 / ((k as f32 + 2.0).log2());
            for a in 0..k {
                for b in (a + 1)..k {
                    let (i, j) = if members[a] < members[b]
                        { (members[a], members[b]) } else { (members[b], members[a]) };
                    edge_weights.entry((i, j)).or_insert((0.0, false)).0 += rarity;
                }
            }
        }

        // Materialize the edge list and update node degrees.
        let mut edges: Vec<WEdge> = Vec::with_capacity(edge_weights.len());
        for (&(i, j), &(w, explicit)) in &edge_weights {
            edges.push(WEdge { i, j, weight: w, explicit });
            nodes[i].degree += 1;
            nodes[j].degree += 1;
        }

        // Modularity Q with wings as the community partition.
        // Q = Σ_c [ L_c / m  -  (D_c / 2m)² ]
        // where L_c = sum of weights of edges entirely inside community c,
        //       D_c = sum of weights of edges incident to community c (×2 for internal),
        //       m   = sum of all edge weights
        let total_w: f32 = edges.iter().map(|e| e.weight).sum::<f32>().max(0.0001);
        let mut wing_internal: HashMap<&str, f32> = HashMap::new();
        let mut wing_total: HashMap<&str, f32> = HashMap::new();
        for e in &edges {
            let wi = nodes[e.i].wing.as_str();
            let wj = nodes[e.j].wing.as_str();
            // Each edge contributes 2*w to the sum of degrees (once per endpoint).
            *wing_total.entry(wi).or_default() += e.weight;
            *wing_total.entry(wj).or_default() += e.weight;
            if wi == wj {
                *wing_internal.entry(wi).or_default() += e.weight;
            }
        }
        let mut modularity = 0.0f32;
        for (wing, l_c) in &wing_internal {
            let d_c = wing_total.get(wing).copied().unwrap_or(0.0);
            modularity += l_c / total_w - (d_c / (2.0 * total_w)).powi(2);
        }
        // Subtract the (D_c/2m)² term for wings that have no internal edges
        // but still appear as endpoints — they reduce Q toward 0.
        for (wing, d_c) in &wing_total {
            if !wing_internal.contains_key(wing) {
                modularity -= (d_c / (2.0 * total_w)).powi(2);
            }
        }

        let orphan_count = nodes.iter().filter(|n| n.degree == 0).count();

        self.nodes = nodes;
        self.edges = edges;
        self.modularity = modularity;
        self.orphan_count = orphan_count;
        self.explicit_count = explicit_count;
        self.settle_ticks = 0;
    }

    /// Single physics step using canonical Fruchterman-Reingold.
    ///
    /// Layout proceeds in a virtual coordinate system sized to the node
    /// count, NOT to the rendering panel. With n=755 in an 80×30 panel
    /// you'd get hopeless wall-pinning if you used panel cells as the
    /// physics area; instead the physics works at "natural" spacing
    /// (k≈SPACING per node) and the renderer fits-to-view at the end.
    /// This means clusters look the same regardless of panel size — they
    /// just scale.
    ///
    /// Forces:
    ///   • repulsion k²/d between every pair (all-pairs O(n²))
    ///   • attraction d²/k along bidirectional links
    ///   • gravity toward origin scaled to ~match repulsion at the
    ///     virtual-area boundary, so isolated nodes return to the cloud
    ///   • move clamped to a temperature that exponentially cools
    fn step(&mut self) {
        let n = self.nodes.len();
        if n == 0 { return; }

        // Virtual layout area — proportional to node count so density is
        // constant. SPACING is the target per-node radius in virtual units.
        const SPACING: f32 = 5.0;
        let virt_size = (n as f32).sqrt() * SPACING;
        let area = virt_size * virt_size;
        let k = (area / (n as f32)).sqrt().max(2.0);  // = SPACING by construction
        let k_sq = k * k;

        // Cooling schedule. Initial temperature ≈ virt_size/10 so first
        // ticks make meaningful progress; cools over ~80 ticks then
        // floors at 0.05 (sub-cell adjustments).
        let t = (virt_size * 0.10 * (-(self.settle_ticks as f32) / 30.0).exp()).max(0.05);

        for nd in self.nodes.iter_mut() { nd.force = (0.0, 0.0); }

        // Repulsion — all pairs. Min-distance floor stops singular blow-up.
        for i in 0..n {
            let pi = self.nodes[i].pos;
            for j in (i + 1)..n {
                let pj = self.nodes[j].pos;
                let dx = pi.0 - pj.0;
                let dy = pi.1 - pj.1;
                let d2 = (dx * dx + dy * dy).max(0.25);
                let coef = k_sq / d2;
                let fx = coef * dx;
                let fy = coef * dy;
                self.nodes[i].force.0 += fx;
                self.nodes[i].force.1 += fy;
                self.nodes[j].force.0 -= fx;
                self.nodes[j].force.1 -= fy;
            }
        }

        // Attraction along weighted edges — F-R formulation scaled by
        // edge weight. F_a = w · d²/k toward partner. Explicit links
        // (weight 3.0) dominate; tag-cooccurrence edges (weight ~0.1-1.0
        // depending on tag rarity) cluster softly. The integrated
        // result: rare-tag-pairs form tight constellations; common-tag
        // pairs nudge toward each other; unrelated pairs only feel
        // repulsion + gravity.
        for e in &self.edges {
            let pi = self.nodes[e.i].pos;
            let pj = self.nodes[e.j].pos;
            let dx = pj.0 - pi.0;
            let dy = pj.1 - pi.1;
            let d2 = (dx * dx + dy * dy).max(0.01);
            let d = d2.sqrt();
            let coef = e.weight * d2 / k;
            let fx = coef * (dx / d);
            let fy = coef * (dy / d);
            self.nodes[e.i].force.0 += fx;
            self.nodes[e.i].force.1 += fy;
            self.nodes[e.j].force.0 -= fx;
            self.nodes[e.j].force.1 -= fy;
        }

        // Gravity toward origin. For an isolated node at the boundary of
        // the virtual area, the nearest cluster's repulsion is roughly
        // k²/d ≈ k. We scale gravity to ~match k at boundary radius so
        // sparse graphs (few/no edges) stay centered as a coherent cloud
        // rather than fragmenting. Linear-in-r so the field is uniform.
        let gravity = k * 0.05;
        for nd in self.nodes.iter_mut() {
            nd.force.0 -= gravity * nd.pos.0;
            nd.force.1 -= gravity * nd.pos.1;
        }

        // Wing-centroid anchor. When `anchor_strength > 0`, every node
        // feels a Hookean pull toward its wing's mean position. With
        // pairwise repulsion still active, the equilibrium is wings
        // forming visually distinct clusters: the wing with most notes
        // gets the largest cluster, smaller wings form satellites.
        // Comparing this view to anchor_strength=0 reveals visually how
        // far Neil's filing taxonomy diverges from his actual
        // tag-driven note structure.
        if self.anchor_strength > 0.0 {
            let mut sums: HashMap<String, ((f32, f32), usize)> = HashMap::new();
            for nd in &self.nodes {
                let entry = sums.entry(nd.wing.clone()).or_insert(((0.0, 0.0), 0));
                entry.0 .0 += nd.pos.0;
                entry.0 .1 += nd.pos.1;
                entry.1 += 1;
            }
            let centroids: HashMap<String, (f32, f32)> = sums.into_iter()
                .map(|(w, (sum, n))| (w, (sum.0 / n as f32, sum.1 / n as f32)))
                .collect();
            // Linear pull magnitude scales with distance × strength × k.
            // 0.3 strength keeps wings loose; 0.6 makes them dominate.
            let coef = self.anchor_strength * k * 0.5;
            for nd in self.nodes.iter_mut() {
                if let Some(c) = centroids.get(&nd.wing) {
                    let dx = c.0 - nd.pos.0;
                    let dy = c.1 - nd.pos.1;
                    nd.force.0 += dx * coef;
                    nd.force.1 += dy * coef;
                }
            }
        }

        // F-R move: step size = min(|F|, t) in direction of F. No
        // bounding box — gravity provides the global container.
        for nd in self.nodes.iter_mut() {
            let fx = nd.force.0;
            let fy = nd.force.1;
            let f_mag = (fx * fx + fy * fy).sqrt().max(0.0001);
            let step = f_mag.min(t);
            nd.pos.0 += fx / f_mag * step;
            nd.pos.1 += fy / f_mag * step;
        }

        self.settle_ticks = self.settle_ticks.saturating_add(1);
    }

    /// Compute logical bounding box of all node positions.
    /// Returns (min_x, min_y, max_x, max_y) — caller projects to cells.
    fn bbox(&self) -> (f32, f32, f32, f32) {
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        for nd in &self.nodes {
            if nd.pos.0 < min_x { min_x = nd.pos.0; }
            if nd.pos.1 < min_y { min_y = nd.pos.1; }
            if nd.pos.0 > max_x { max_x = nd.pos.0; }
            if nd.pos.1 > max_y { max_y = nd.pos.1; }
        }
        (min_x, min_y, max_x, max_y)
    }
}

#[derive(Clone, Copy)]
struct Cell {
    glyph: char,
    fg: Color,
    bg: Option<Color>,
    bold: bool,
}
impl Default for Cell {
    fn default() -> Self {
        Cell { glyph: ' ', fg: Color::Reset, bg: None, bold: false }
    }
}

/// Bresenham line. Calls `plot(x, y)` for every cell along the segment.
/// Bounds checking is the caller's responsibility.
fn bresenham(x0: i32, y0: i32, x1: i32, y1: i32, mut plot: impl FnMut(i32, i32)) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;
    loop {
        plot(x, y);
        if x == x1 && y == y1 { break; }
        let e2 = 2 * err;
        if e2 >= dy { err += dy; x += sx; }
        if e2 <= dx { err += dx; y += sy; }
    }
}

/// Total node count — used by the panel title for a count badge.
pub fn node_count() -> usize {
    graph_state().lock().map(|s| s.nodes.len()).unwrap_or(0)
}

/// Total weighted-edge count (explicit + tag-implicit).
pub fn edge_count() -> usize {
    graph_state().lock().map(|s| s.edges.len()).unwrap_or(0)
}

/// Just the explicit `zettel link` edges. The rare ones Neil curated.
pub fn explicit_count() -> usize {
    graph_state().lock().map(|s| s.explicit_count).unwrap_or(0)
}

/// Modularity Q with wings as the community partition. Range [-0.5, 1.0]
/// in theory; ≥ 0.3 is a meaningful community structure, ≥ 0.6 is
/// strong. Negative would mean wings are anti-correlated with structure.
pub fn modularity() -> f32 {
    graph_state().lock().map(|s| s.modularity).unwrap_or(0.0)
}

/// Notes with no incident edge — neither linked nor sharing a tag with
/// any other note within the MAX_TAG_FANOUT cap.
pub fn orphan_count() -> usize {
    graph_state().lock().map(|s| s.orphan_count).unwrap_or(0)
}

/// Current wing-anchor strength: 0.0 = free physics, 0.3 = soft, 0.6 = strong.
pub fn anchor_strength() -> f32 {
    graph_state().lock().map(|s| s.anchor_strength).unwrap_or(0.0)
}

/// Cycle wing-anchor strength: 0.0 → 0.3 → 0.6 → 0.0. Returns the new
/// value. Re-arms the cooling schedule so the layout visibly reshuffles
/// instead of jumping; the user gets a 3-second animated transition
/// from one mode to the next.
pub fn toggle_anchors() -> f32 {
    if let Ok(mut s) = graph_state().lock() {
        s.anchor_strength = if s.anchor_strength < 0.15 { 0.3 }
                            else if s.anchor_strength < 0.45 { 0.6 }
                            else { 0.0 };
        s.settle_ticks = 0;
        return s.anchor_strength;
    }
    0.0
}

/// Re-randomize all node positions from a fresh seed. Useful when
/// physics gets stuck in a poor local minimum — the global structure
/// (cluster count, modularity) is determined by the edge weights, but
/// the specific arrangement on screen is seed-dependent.
pub fn reseed() {
    if let Ok(mut s) = graph_state().lock() {
        s.seed_state = s.seed_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let mut rng = s.seed_state;
        for nd in s.nodes.iter_mut() {
            nd.pos = (xrand_unit(&mut rng) * 20.0 - 10.0,
                      xrand_unit(&mut rng) * 20.0 - 10.0);
            nd.vel = (0.0, 0.0);
            nd.force = (0.0, 0.0);
        }
        s.settle_ticks = 0;
    }
}

/// Render the topology into `area_w × area_h` lines of styled spans.
/// Returns `Vec<Line<'static>>` so the caller can drop the lock before
/// `frame.render_widget(Paragraph::new(lines), inner)`.
pub fn render_lines(area_w: u16, area_h: u16) -> Vec<Line<'static>> {
    let w = area_w as usize;
    let h = area_h as usize;
    if w < 4 || h < 4 {
        return vec![Line::from(Span::styled(
            "panel too small for graph view".to_string(),
            Style::default().fg(Color::DarkGray),
        ))];
    }

    // Snapshot cache version + JSON outside the physics lock.
    let (cur_version, json_opt) = match graph_cache().lock() {
        Ok(g) => (g.0, g.1.clone()),
        Err(_) => (0, None),
    };

    let mut state = match graph_state().lock() {
        Ok(s) => s,
        Err(_) => return vec![Line::from("graph: state lock poisoned")],
    };

    if cur_version != state.last_load_version {
        if let Some(ref json) = json_opt {
            state.rebuild_from_json(json);
            state.last_load_version = cur_version;
        }
    }

    if state.nodes.is_empty() {
        let msg = if json_opt.is_none() {
            "Loading note graph… (running `zettel list --json`)"
        } else {
            "No notes yet — try `zettel new \"first thought\"`"
        };
        return vec![Line::from(Span::styled(
            msg.to_string(), Style::default().fg(Color::DarkGray)))];
    }

    // Two physics ticks per frame — visibly converges in ~3 s at 30 fps.
    state.step();
    state.step();

    // Fit-to-view: physics positions live in an isotropic virtual coord
    // system sized to node count, not panel cells. Compute the actual
    // bounding box and scale to fit. Terminal cells are 2:1 (taller than
    // wide), so cells-per-Y-unit is half cells-per-X-unit; that's the
    // sole anisotropy correction.
    let (min_x, min_y, max_x, max_y) = state.bbox();
    let span_x = (max_x - min_x).max(0.001);
    let span_y = (max_y - min_y).max(0.001);
    let usable_w = (w as f32 - 2.0).max(1.0);
    let usable_h = (h as f32 - 2.0).max(1.0);
    // Pick scale_x so BOTH (span_x * scale_x ≤ usable_w) AND
    // (span_y * scale_y ≤ usable_h) hold, with scale_y = scale_x / 2.
    let scale_x = (usable_w / span_x).min(2.0 * usable_h / span_y);
    let scale_y = scale_x * 0.5;
    let off_x = 1.0 + (usable_w - span_x * scale_x) * 0.5;
    let off_y = 1.0 + (usable_h - span_y * scale_y) * 0.5;

    let proj = |pos: (f32, f32)| -> Option<(i32, i32)> {
        let px = (off_x + (pos.0 - min_x) * scale_x).floor() as i32;
        let py = (off_y + (pos.1 - min_y) * scale_y).floor() as i32;
        if px < 0 || py < 0 || px >= w as i32 || py >= h as i32 { None }
        else { Some((px, py)) }
    };

    // ── Layer 1: explicit-link edges only ───────────────────────────────
    // Tag-cooccurrence pulls notes together silently — its effect shows as
    // CLUSTERING in the layout, not as drawn lines. Only the rare curated
    // `zettel link` edges get rasterized, as bright cyan corridors that
    // pop against the otherwise-dark canvas. This way the user can read
    // two distinct organizational signals at the same time:
    //   • where things group     → tag intelligence
    //   • where bright lines run → explicit-link intelligence
    let mut grid: Vec<Cell> = vec![Cell::default(); w * h];
    for e in state.edges.iter().filter(|e| e.explicit) {
        let Some((x0, y0)) = proj(state.nodes[e.i].pos) else { continue };
        let Some((x1, y1)) = proj(state.nodes[e.j].pos) else { continue };
        bresenham(x0, y0, x1, y1, |x, y| {
            if x < 0 || y < 0 { return; }
            let xi = x as usize; let yi = y as usize;
            if xi >= w || yi >= h { return; }
            let idx = yi * w + xi;
            // Bright cyan background; node glyphs paint over endpoint cells.
            grid[idx].bg = Some(Color::Rgb(40, 110, 150));
        });
    }

    // ── Layer 2: nodes ─────────────────────────────────────────────────
    let max_degree = state.nodes.iter().map(|n| n.degree).max().unwrap_or(0);

    // Render lower-degree first so hubs paint on top.
    let mut order: Vec<usize> = (0..state.nodes.len()).collect();
    order.sort_by_key(|&i| state.nodes[i].degree);
    for &i in &order {
        let nd = &state.nodes[i];
        let Some((x, y)) = proj(nd.pos) else { continue };
        let idx = (y as usize) * w + (x as usize);
        let intensity = if max_degree == 0 { 0.0 }
                        else { nd.degree as f32 / max_degree as f32 };
        let glyph = if intensity >= 0.55 { '●' }
                    else if intensity >= 0.20 { '•' }
                    else { '·' };
        grid[idx].glyph = glyph;
        grid[idx].fg = wing_color(&nd.wing);
        grid[idx].bold = intensity >= 0.55;
    }

    // Hub labels: top ~5% by degree, capped 3..=8. Write to the right of
    // the node when there's room; bail on collision (prevents overwriting
    // other nodes with label characters).
    let mut by_degree: Vec<usize> = (0..state.nodes.len())
        .filter(|&i| state.nodes[i].degree > 0)
        .collect();
    by_degree.sort_by(|&a, &b| state.nodes[b].degree.cmp(&state.nodes[a].degree));
    let n_labels = (state.nodes.len() / 20).max(3).min(8).min(by_degree.len());

    for &i in by_degree.iter().take(n_labels) {
        let nd = &state.nodes[i];
        let Some((x, y)) = proj(nd.pos) else { continue };
        let label_src = if !nd.preview.is_empty() { nd.preview.as_str() }
                        else { nd.id.as_str() };
        let label: String = label_src.chars()
            .filter(|c| !c.is_control())
            .take(14)
            .collect();
        let yu = y as usize;
        let mut lx = (x as usize) + 1;
        for ch in label.chars() {
            if lx >= w { break; }
            let idx = yu * w + lx;
            let existing = grid[idx].glyph;
            if existing == '·' || existing == '•' || existing == '●' { break; }
            grid[idx].glyph = ch;
            grid[idx].fg = wing_color(&nd.wing);
            grid[idx].bold = true;
            lx += 1;
        }
    }

    // ── Grid → Vec<Line<'static>> with style coalescing ─────────────────
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(h);
    for y in 0..h {
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut cur = String::new();
        let mut cur_style = Style::default();
        for x in 0..w {
            let c = grid[y * w + x];
            let mut s = Style::default();
            if c.fg != Color::Reset { s = s.fg(c.fg); }
            if let Some(bg) = c.bg { s = s.bg(bg); }
            if c.bold { s = s.add_modifier(Modifier::BOLD); }
            if cur.is_empty() {
                cur.push(c.glyph);
                cur_style = s;
            } else if s == cur_style {
                cur.push(c.glyph);
            } else {
                spans.push(Span::styled(std::mem::take(&mut cur), cur_style));
                cur.push(c.glyph);
                cur_style = s;
            }
        }
        if !cur.is_empty() { spans.push(Span::styled(cur, cur_style)); }
        lines.push(Line::from(spans));
    }
    lines
}
