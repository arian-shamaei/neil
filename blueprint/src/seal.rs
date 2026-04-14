use std::fs;
use std::path::PathBuf;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct SealPose {
    #[serde(default = "default_eyes")]
    pub eyes: String,       // open, half, closed, wide, focused, stressed
    #[serde(default = "default_mouth")]
    pub mouth: String,      // smile, neutral, frown, open, relaxed
    #[serde(default = "default_body")]
    pub body: String,       // float, swim, dive, surface, curl, sleep
    #[serde(default = "default_indicator")]
    pub indicator: String,  // none, zzz, alert, thought, bubbles, music
    #[serde(default = "default_label")]
    pub label: String,
}

fn default_eyes() -> String { "open".into() }
fn default_mouth() -> String { "smile".into() }
fn default_body() -> String { "float".into() }
fn default_indicator() -> String { "none".into() }
fn default_label() -> String { "~ neil ~".into() }

impl Default for SealPose {
    fn default() -> Self {
        Self {
            eyes: "open".into(), mouth: "smile".into(), body: "float".into(),
            indicator: "none".into(), label: "~ neil ~".into(),
        }
    }
}

impl SealPose {
    pub fn load(neil_home: &PathBuf) -> Self {
        let path = neil_home.join(".seal_pose.json");
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }
}

// ── Dot grid for braille rendering ──────────────────────────
// Each braille char encodes a 2x4 dot block.
// Grid is DOT_W x DOT_H dots, rendered as (DOT_W/2) x (DOT_H/4) chars.

const DOT_W: usize = 52;  // 26 chars * 2 dots
const DOT_H: usize = 48;  // 12 chars * 4 dots

struct DotGrid {
    dots: [[bool; DOT_W]; DOT_H],
}

impl DotGrid {
    fn new() -> Self {
        Self { dots: [[false; DOT_W]; DOT_H] }
    }

    fn set(&mut self, x: i32, y: i32) {
        if x >= 0 && y >= 0 && (x as usize) < DOT_W && (y as usize) < DOT_H {
            self.dots[y as usize][x as usize] = true;
        }
    }

    /// Draw ellipse outline centered at (cx,cy) with radii (rx,ry)
    fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32) {
        let steps = ((rx + ry) * 4.0) as i32;
        for i in 0..steps {
            let angle = (i as f32 / steps as f32) * std::f32::consts::TAU;
            let x = cx + rx * angle.cos();
            let y = cy + ry * angle.sin();
            self.set(x as i32, y as i32);
        }
    }

    /// Fill an ellipse centered at (cx,cy) with radii (rx,ry)
    fn fill_ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32) {
        let x0 = (cx - rx).floor() as i32;
        let x1 = (cx + rx).ceil() as i32;
        let y0 = (cy - ry).floor() as i32;
        let y1 = (cy + ry).ceil() as i32;
        for y in y0..=y1 {
            for x in x0..=x1 {
                let dx = (x as f32 - cx) / rx;
                let dy = (y as f32 - cy) / ry;
                if dx * dx + dy * dy <= 1.0 {
                    self.set(x, y);
                }
            }
        }
    }

    /// Draw rotated ellipse outline
    fn ellipse_rotated(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, angle: f32) {
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let steps = ((rx + ry) * 4.0) as i32;
        for i in 0..steps {
            let t = (i as f32 / steps as f32) * std::f32::consts::TAU;
            let lx = rx * t.cos();
            let ly = ry * t.sin();
            let x = cx + lx * cos_a - ly * sin_a;
            let y = cy + lx * sin_a + ly * cos_a;
            self.set(x as i32, y as i32);
        }
    }

    /// Fill a rotated ellipse
    fn fill_ellipse_rotated(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, angle: f32) {
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let r = rx.max(ry);
        let x0 = (cx - r - 1.0).floor() as i32;
        let x1 = (cx + r + 1.0).ceil() as i32;
        let y0 = (cy - r - 1.0).floor() as i32;
        let y1 = (cy + r + 1.0).ceil() as i32;
        for y in y0..=y1 {
            for x in x0..=x1 {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let lx = dx * cos_a + dy * sin_a;
                let ly = -dx * sin_a + dy * cos_a;
                if (lx / rx) * (lx / rx) + (ly / ry) * (ly / ry) <= 1.0 {
                    self.set(x, y);
                }
            }
        }
    }

    /// Convert dot grid to braille character lines
    fn to_braille(&self) -> Vec<String> {
        let char_h = DOT_H / 4;
        let char_w = DOT_W / 2;
        let mut lines = Vec::new();

        for row in 0..char_h {
            let mut line = String::new();
            for col in 0..char_w {
                let bx = col * 2;
                let by = row * 4;
                // Braille dot positions:
                // (0,0) (1,0)
                // (0,1) (1,1)
                // (0,2) (1,2)
                // (0,3) (1,3)
                let mut code: u32 = 0x2800;
                if self.get(bx, by)     { code |= 0x01; }
                if self.get(bx, by+1)   { code |= 0x02; }
                if self.get(bx, by+2)   { code |= 0x04; }
                if self.get(bx+1, by)   { code |= 0x08; }
                if self.get(bx+1, by+1) { code |= 0x10; }
                if self.get(bx+1, by+2) { code |= 0x20; }
                if self.get(bx, by+3)   { code |= 0x40; }
                if self.get(bx+1, by+3) { code |= 0x80; }
                line.push(char::from_u32(code).unwrap_or(' '));
            }
            lines.push(line);
        }
        lines
    }

    fn get(&self, x: usize, y: usize) -> bool {
        if x < DOT_W && y < DOT_H { self.dots[y][x] } else { false }
    }

    /// Clear a rectangular region (for eyes/mouth cutouts)
    fn clear_rect(&mut self, x: i32, y: i32, w: i32, h: i32) {
        for dy in 0..h {
            for dx in 0..w {
                let px = x + dx;
                let py = y + dy;
                if px >= 0 && py >= 0 && (px as usize) < DOT_W && (py as usize) < DOT_H {
                    self.dots[py as usize][px as usize] = false;
                }
            }
        }
    }
}

/// Draw a line between two points using Bresenham
fn draw_line(g: &mut DotGrid, x0: f32, y0: f32, x1: f32, y1: f32) {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let steps = dx.max(dy) as i32 + 1;
    for i in 0..=steps {
        let t = if steps > 0 { i as f32 / steps as f32 } else { 0.0 };
        let x = x0 + (x1 - x0) * t;
        let y = y0 + (y1 - y0) * t;
        g.set(x as i32, y as i32);
    }
}

/// Render the seal as a side-profile swimming in water.
pub fn render_seal(pose: &SealPose, tick: u64) -> Vec<String> {
    let mut g = DotGrid::new();
    let t = tick as f64;

    // Body center
    let cx = 28.0_f32;
    let cy = 24.0_f32;

    // Breathing
    let breath = ((t * 0.15).sin() * 0.5 + 0.5) as f32;
    let breath_scale = 1.0 + breath * 0.08;

    // Body curl for different poses
    let curl = match pose.body.as_str() {
        "curl" | "sleep" => 0.3,
        "dive" => -0.2,
        "swim" => ((t * 0.2).sin() * 0.1) as f32,
        _ => 0.0,
    };

    // ── TAIL FLIPPERS ──
    let tail_x = cx + 12.0;
    let tail_y = cy + curl * 6.0;
    let splay = 4.0;
    let tail_wave = ((t * 0.3).sin() * 1.5) as f32;
    g.ellipse_rotated(tail_x + 2.0, tail_y - splay + tail_wave, 1.2, 4.0, -0.4);
    g.ellipse_rotated(tail_x + 2.0, tail_y + splay + tail_wave, 1.2, 4.0, 0.4);

    // ── BODY ── fusiform torpedo shape (outline only)
    let body_len = 13.0;
    let max_ry = 7.0 * breath_scale;

    // Draw body as a smooth outline using top and bottom contour
    let steps = 40;
    let mut top_points: Vec<(f32, f32)> = Vec::new();
    let mut bot_points: Vec<(f32, f32)> = Vec::new();

    for i in 0..=steps {
        let t_pos = i as f32 / steps as f32;
        let x = cx - body_len + t_pos * body_len * 2.0;
        let by = cy + curl * (t_pos - 0.5) * 10.0;

        let ry = if t_pos < 0.2 {
            let s = t_pos / 0.2;
            max_ry * (0.4 + 0.6 * (s * std::f32::consts::FRAC_PI_2).sin())
        } else if t_pos < 0.6 {
            max_ry
        } else {
            let s = (t_pos - 0.6) / 0.4;
            max_ry * (0.15 + 0.85 * (s * std::f32::consts::FRAC_PI_2).cos())
        };

        top_points.push((x, by - ry));
        bot_points.push((x, by + ry));
    }

    // Draw contour lines (double for thickness)
    for i in 1..top_points.len() {
        draw_line(&mut g, top_points[i-1].0, top_points[i-1].1, top_points[i].0, top_points[i].1);
        draw_line(&mut g, top_points[i-1].0, top_points[i-1].1 + 1.0, top_points[i].0, top_points[i].1 + 1.0);
        draw_line(&mut g, bot_points[i-1].0, bot_points[i-1].1, bot_points[i].0, bot_points[i].1);
        draw_line(&mut g, bot_points[i-1].0, bot_points[i-1].1 - 1.0, bot_points[i].0, bot_points[i].1 - 1.0);
    }

    // ── HEAD + SNOUT as one smooth profile ──
    // Draw the head as a series of connected points (smooth contour)
    let head_x = cx - body_len + 2.0;
    let head_y = cy - 1.0 + curl * -3.0;
    let nose_x = head_x - 14.0;
    let nose_y = head_y + 5.0;

    // Top of head: rounded dome, slopes to snout
    let head_top: Vec<(f32, f32)> = vec![
        (head_x + 5.0, head_y - 4.0),     // back of head
        (head_x + 3.0, head_y - 8.0),     // rear skull
        (head_x, head_y - 10.0),           // top of dome
        (head_x - 4.0, head_y - 9.0),     // front skull
        (head_x - 7.0, head_y - 6.0),     // forehead slope
        (head_x - 9.0, head_y - 3.0),     // brow ridge
        (head_x - 11.0, head_y),           // bridge of nose
        (head_x - 13.0, head_y + 2.0),    // upper snout
        (nose_x, nose_y - 1.0),            // nose tip top
    ];
    // Bottom: chin curve, jowl, throat
    let head_bot: Vec<(f32, f32)> = vec![
        (nose_x, nose_y + 2.0),            // nose tip bottom
        (nose_x + 2.0, nose_y + 4.0),     // lower lip
        (head_x - 10.0, head_y + 6.0),    // under jaw
        (head_x - 6.0, head_y + 9.0),     // chin fullest
        (head_x - 2.0, head_y + 10.0),    // jowl
        (head_x + 2.0, head_y + 8.0),     // throat
        (head_x + 5.0, head_y + 5.0),     // neck into body
    ];

    for i in 1..head_top.len() {
        draw_line(&mut g, head_top[i-1].0, head_top[i-1].1, head_top[i].0, head_top[i].1);
    }
    for i in 1..head_bot.len() {
        draw_line(&mut g, head_bot[i-1].0, head_bot[i-1].1, head_bot[i].0, head_bot[i].1);
    }
    // Close the nose
    draw_line(&mut g, nose_x, nose_y - 1.0, nose_x, nose_y + 1.0);

    // ── NOSE dots ──
    g.set(nose_x as i32, nose_y as i32);

    // ── EYE ── larger, more visible
    let eye_x = (head_x - 5.0) as i32;
    let eye_y = (head_y - 2.0) as i32;
    let blink = tick % 55 < 3;
    let eye_closed = pose.eyes == "closed" || pose.body == "sleep";

    // Clear area around eye for contrast
    g.clear_rect(eye_x - 2, eye_y - 2, 5, 5);

    if !blink && !eye_closed {
        // Open eye - 3x3 filled square with highlight
        for dy in 0..3 {
            for dx in 0..3 {
                g.set(eye_x + dx, eye_y + dy);
            }
        }
        // Clear center for pupil highlight
        g.clear_rect(eye_x + 1, eye_y, 1, 1);
    } else {
        // Closed/blink - horizontal line
        for dx in 0..4 {
            g.set(eye_x + dx, eye_y + 1);
        }
    }

    // ── MOUTH ── slight curve from nose
    let mouth_curve = match pose.mouth.as_str() {
        "smile" => 1.0_f32,
        "frown" => -1.0,
        "open" => 0.5,
        _ => 0.0,
    };
    draw_line(&mut g, nose_x + 1.0, nose_y + 1.5, nose_x + 5.0, nose_y + 2.5 + mouth_curve);

    // ── WHISKERS ── from nose area
    let wh_x = nose_x - 1.0;
    let wh_y = nose_y;
    draw_line(&mut g, wh_x, wh_y - 1.0, wh_x - 5.0, wh_y - 4.0);
    draw_line(&mut g, wh_x, wh_y, wh_x - 6.0, wh_y - 1.0);
    draw_line(&mut g, wh_x, wh_y + 1.5, wh_x - 5.0, wh_y + 3.0);

    // ── FRONT FLIPPER ── outline, larger
    let flip_angle = 0.7 + ((t * 0.25).sin() * 0.25) as f32;
    g.ellipse_rotated(cx - 5.0, cy + max_ry * 0.6, 2.0, 7.0, flip_angle);

    // ── EYE cutout + draw ──
    let eye_x = (head_x - 2.0) as i32;
    let eye_y = (head_y - 2.0) as i32;

    // Blink
    let blink = tick % 50 < 2;
    let eye_closed = pose.eyes == "closed" || pose.eyes == "half" || pose.body == "sleep";

    if !blink && !eye_closed {
        // Clear eye area and redraw as open eye
        g.clear_rect(eye_x, eye_y, 3, 3);
        // Pupil dot
        g.set(eye_x + 1, eye_y + 1);
        if pose.eyes == "wide" || pose.eyes == "focused" {
            g.set(eye_x, eye_y + 1);
            g.set(eye_x + 2, eye_y + 1);
        }
    } else {
        // Closed eye - just a line
        g.clear_rect(eye_x, eye_y, 3, 3);
        g.set(eye_x, eye_y + 1);
        g.set(eye_x + 1, eye_y + 1);
        g.set(eye_x + 2, eye_y + 1);
    }

    // ── NOSE ──
    let nose_x = (head_x - 7.0) as i32;
    let nose_y = (head_y + 2.0) as i32;
    g.set(nose_x, nose_y);
    g.set(nose_x + 1, nose_y);

    // ── Convert to braille ──
    let mut braille_lines = g.to_braille();

    // ── WATER SURFACE ── at the head level, goes through the body
    let water_y = 3; // Where the head pokes out
    let char_w = DOT_W / 2;
    if water_y < braille_lines.len() {
        let wave_offset = (tick % 12) as usize;
        let water_chars = ['~', '∼', '~', '≈', '~', '~', '∿', '~', '≈', '~', '∼', '~'];
        let orig = &braille_lines[water_y];
        let mut blended = String::new();
        for (i, ch) in orig.chars().enumerate() {
            if ch == '\u{2800}' || ch == ' ' {
                blended.push(water_chars[(i + wave_offset) % water_chars.len()]);
            } else {
                blended.push(ch);
            }
        }
        braille_lines[water_y] = blended;
    }
    // Also add water to line above for thickness
    let water_y2 = 2;
    if water_y2 < braille_lines.len() {
        let wave_offset = (tick % 12 + 3) as usize;
        let water_chars = ['~', '~', '≈', '~', '∼', '~', '~', '∿', '~', '≈', '~', '~'];
        let orig = &braille_lines[water_y2];
        let mut blended = String::new();
        for (i, ch) in orig.chars().enumerate() {
            if ch == '\u{2800}' || ch == ' ' {
                blended.push(water_chars[(i + wave_offset) % water_chars.len()]);
            } else {
                blended.push(ch);
            }
        }
        braille_lines[water_y2] = blended;
    }

    // ── INDICATOR ── top-right corner
    let indicator_str = match pose.indicator.as_str() {
        "zzz" => {
            let phase = (tick % 9) as usize;
            match phase / 3 { 0 => "  z", 1 => " z ", _ => "z  " }
        }
        "alert" => if tick % 4 < 2 { " !!" } else { "! !" },
        "thought" => if tick % 6 < 3 { " .o" } else { "o. " },
        "bubbles" => if tick % 6 < 2 { " °°" } else if tick % 6 < 4 { "°  " } else { "  °" },
        "music" => if tick % 4 < 2 { " ♪ " } else { "♫  " },
        _ => "   ",
    };

    // Insert indicator at top-right of first line
    if let Some(first) = braille_lines.first_mut() {
        let flen = first.chars().count();
        if flen >= 3 {
            let keep: String = first.chars().take(flen - 3).collect();
            *first = format!("{}{}", keep, indicator_str);
        }
    }

    // ── UNDERWATER particles ── below water line
    for line_idx in 6..braille_lines.len() {
        let orig = &braille_lines[line_idx];
        let mut modified = String::new();
        for (i, ch) in orig.chars().enumerate() {
            if ch == '\u{2800}' || ch == ' ' {
                // Sparse underwater particles
                let particle_hash = ((line_idx * 37 + i * 13 + tick as usize) % 100) as u32;
                let pch = if particle_hash < 2 { '·' }
                    else if particle_hash < 3 { '∙' }
                    else if particle_hash < 4 { '°' }
                    else { ' ' };
                modified.push(pch);
            } else {
                modified.push(ch);
            }
        }
        braille_lines[line_idx] = modified;
    }

    // ── LABEL ──
    braille_lines.push(format!("   {}   ", pose.label));

    braille_lines
}
