use std::fs;
use std::path::PathBuf;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct SealPose {
    #[serde(default = "default_eyes")]
    pub eyes: String,
    #[serde(default = "default_mouth")]
    pub mouth: String,
    #[serde(default = "default_body")]
    pub body: String,
    #[serde(default = "default_indicator")]
    pub indicator: String,
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

/// Hand-crafted seal art with swappable features.
/// Side-profile seal swimming left, half-submerged.
pub fn render_seal(pose: &SealPose, tick: u64) -> Vec<String> {
    // Eye character
    let eye = match pose.eyes.as_str() {
        "open" => if tick % 55 < 2 { "─" } else { "●" },
        "half" => "◑",
        "closed" | "sleep" => "─",
        "wide" => "◎",
        "focused" => "◉",
        "stressed" => "×",
        "wink" => if tick % 30 < 15 { "●" } else { "─" },
        _ => if tick % 55 < 2 { "─" } else { "●" },
    };

    // Mouth character
    let mouth = match pose.mouth.as_str() {
        "smile" => "◡",
        "neutral" => "─",
        "frown" => "︵",
        "open" => "○",
        "relaxed" => "~",
        _ => "◡",
    };

    // Indicator (top-right, animated)
    let indicator = match pose.indicator.as_str() {
        "zzz" => match (tick / 8) % 3 { 0 => "  z", 1 => " z ", _ => "z  " },
        "alert" => if tick % 6 < 3 { " !!" } else { "! !" },
        "thought" => match (tick / 6) % 3 { 0 => "  .", 1 => " .o", _ => ".oO" },
        "bubbles" => match (tick / 5) % 3 { 0 => "  °", 1 => " °°", _ => "°  " },
        "music" => if tick % 8 < 4 { " ♪ " } else { " ♫ " },
        "heart" => " ♥ ",
        _ => "   ",
    };

    // Water wave animation
    let w = tick as usize;
    let wave = |offset: usize| -> String {
        let chars = ['~', '~', '∼', '≈', '~', '~', '∿', '~', '≈', '~', '∼', '~'];
        (0..26).map(|i| chars[(i + w + offset) % chars.len()]).collect()
    };

    // Flipper frame
    let flipper = match (tick / 6) % 3 {
        0 => "╱  ",
        1 => "╱╲ ",
        _ => " ╲ ",
    };

    // Tail flipper
    let tail = match (tick / 8) % 4 {
        0 => "⟩ ",
        1 => "⟩╲",
        2 => "⟩ ",
        _ => "⟩╱",
    };

    // Body pose (breathing alternates between two frames)
    let inhale = (tick / 12) % 2 == 0;

    let mut lines = Vec::new();

    if inhale {
        lines.push(format!("{}{}",  "                       ", indicator));
        lines.push(wave(0));
        lines.push(format!("{}{}{}","  ___                ", tail, "  "));
        lines.push(format!(" / {} \\_______________{}","  ", "╲ "));
        lines.push(format!("|  {}  {}  ⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿ |", eye, mouth));
        lines.push(format!(" \\__▼_/‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾╱ "));
        lines.push(format!("  {} ‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾  ", flipper));
        lines.push(wave(5));
        lines.push(format!("  · {} ·  °    ·", "      "));
        lines.push(format!("     {}    ", pose.label));
    } else {
        lines.push(format!("{}{}",  "                       ", indicator));
        lines.push(wave(0));
        lines.push(format!("{}{}{}","   ___               ", tail, "  "));
        lines.push(format!("  / {} \\______________{}","  ", "╲ "));
        lines.push(format!(" |  {}  {}  ⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿|", eye, mouth));
        lines.push(format!("  \\__▼_/‾‾‾‾‾‾‾‾‾‾‾‾‾‾╱  "));
        lines.push(format!("   {} ‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾  ", flipper));
        lines.push(wave(5));
        lines.push(format!("   ·  {} ·   °   ·", "     "));
        lines.push(format!("     {}    ", pose.label));
    };

    lines
}
