use std::fs;
use std::path::PathBuf;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct SealPose {
    #[serde(default = "default_eyes")]
    pub eyes: String,
    #[serde(default = "default_mouth")]
    pub mouth: String,
    #[serde(default = "default_whiskers")]
    pub whiskers: String,
    #[serde(default = "default_body")]
    pub body: String,
    #[serde(default)]
    pub breath_phase: f32,
    #[serde(default)]
    pub water_phase: f32,
    #[serde(default = "default_indicator")]
    pub indicator: String,
    #[serde(default = "default_label")]
    pub label: String,
}

fn default_eyes() -> String { "open".into() }
fn default_mouth() -> String { "smile".into() }
fn default_whiskers() -> String { "normal".into() }
fn default_body() -> String { "float".into() }
fn default_indicator() -> String { "none".into() }
fn default_label() -> String { "~ neil ~".into() }

impl Default for SealPose {
    fn default() -> Self {
        Self {
            eyes: "open".into(),
            mouth: "smile".into(),
            whiskers: "normal".into(),
            body: "float".into(),
            breath_phase: 0.5,
            water_phase: 0.0,
            indicator: "none".into(),
            label: "~ neil ~".into(),
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

/// Render the seal as lines of text based on the pose parameters.
/// Fits in ~24 chars wide, ~12 lines tall.
pub fn render_seal(pose: &SealPose, tick: u64) -> Vec<String> {
    // Auto-animate breath and water
    let t = tick as f64 * 0.1;
    let breath = ((t * 0.8).sin() * 0.5 + 0.5) as f32;
    let water = ((t * 0.5).sin() * 0.5 + 0.5) as f32;

    let breath = if pose.breath_phase > 0.01 { pose.breath_phase } else { breath };
    let _water = if pose.water_phase > 0.01 { pose.water_phase } else { water };

    // Eyes
    let (le, re) = match pose.eyes.as_str() {
        "open"     => ('●', '●'),
        "half"     => ('◑', '◑'),
        "closed"   => ('─', '─'),
        "wide"     => ('◎', '◎'),
        "focused"  => ('◉', '◉'),
        "stressed" => ('×', '×'),
        "wink"     => ('─', '●'),
        _ => ('●', '●'),
    };

    // Auto-blink every ~40 ticks
    let (le, re) = if tick % 40 < 2 { ('─', '─') } else { (le, re) };

    // Mouth
    let mouth = match pose.mouth.as_str() {
        "smile"   => '◡',
        "neutral" => '─',
        "frown"   => '∩',
        "open"    => '○',
        "relaxed" => 'ω',
        "smirk"   => '⌐',
        "o"       => '◯',
        _ => '◡',
    };

    // Whiskers
    let (wl, wr) = match pose.whiskers.as_str() {
        "normal"  => ("═══", "═══"),
        "perked"  => ("⟋⟋⟋", "⟍⟍⟍"),
        "droopy"  => ("───", "───"),
        "spread"  => ("⟋─⟋", "⟍─⟍"),
        _ => ("═══", "═══"),
    };

    // Indicator (top right)
    let indicator = match pose.indicator.as_str() {
        "zzz"     => if tick % 6 < 2 { "  z" } else if tick % 6 < 4 { " z " } else { "z  " },
        "alert"   => if tick % 4 < 2 { " !" } else { "!!" },
        "thought" => if tick % 8 < 3 { " ." } else if tick % 8 < 6 { ".o" } else { " O" },
        "bubbles" => if tick % 6 < 2 { " °" } else if tick % 6 < 4 { "° " } else { " ○" },
        "music"   => if tick % 4 < 2 { " ♪" } else { "♫ " },
        "heart"   => "♥ ",
        _ => "  ",
    };

    // Body shape varies with breath
    let inhale = breath > 0.5;

    // Water wave animation
    let wave_offset = (tick % 8) as usize;
    let waves = &[
        "~~~~~~~~~~~~~~~~~~~~~~",
        "~~~~~≈~~~~~~~~~~~~~~~~",
        "~~~~≈~~≈~~~~~~~~~~~~~~",
        "~~~≈~~~~≈~~~~~~~~~~~~~",
        "~~≈~~~~~~≈~~~~~~~~~~~~",
        "~≈~~~~~~~~≈~~~~~~~~~~~",
        "≈~~~~~~~~~~≈~~~~~~~~~~",
        "~~~~~~~~~~~~≈~~~~~~~~~",
    ];
    let water_line = waves[wave_offset % waves.len()];

    // Flipper animation
    let flip = (tick % 12) as i32;
    let (fl, fr) = if flip < 4 {
        ("⠿⠤", "⠤⠿")
    } else if flip < 8 {
        ("⠿⠄", "⠠⠿")
    } else {
        ("⠿⡀", "⢀⠿")
    };

    let mut lines = Vec::new();

    if inhale {
        // Expanded body
        lines.push(format!("     ⣀⣤⣴⣶⣶⣴⣤⣀     {}", indicator));
        lines.push(format!("   ⣴⣿⣿⣿⣿⣿⣿⣿⣿⣦   "));
        lines.push(format!("  ⣿⣿ {}    {} ⣿⣿  ", le, re));
        lines.push(format!("{} ⣿⣿⣿ ▼ ⣿⣿⣿ {}", wl, wr));
        lines.push(format!("   ⣿⣿⣿⣿{}⣿⣿⣿⣿   ", mouth));
        lines.push(format!("    ⠹⣿⣿⣿⣿⣿⣿⠏    "));
        lines.push(format!(" {}⣤⣿⣿⣿⣿⣿⣿⣤{} ", fl, fr));
        lines.push(format!("    ⠘⣿⣿⣿⣿⣿⣿⠃    "));
    } else {
        // Contracted body
        lines.push(format!("      ⣀⣤⣤⣤⣤⣀      {}", indicator));
        lines.push(format!("    ⣴⣿⣿⣿⣿⣿⣿⣦    "));
        lines.push(format!("   ⣿⣿ {}   {} ⣿⣿   ", le, re));
        lines.push(format!("{} ⣿⣿⣿ ▼ ⣿⣿⣿ {}", wl, wr));
        lines.push(format!("    ⣿⣿⣿{}⣿⣿⣿    ", mouth));
        lines.push(format!("     ⠹⣿⣿⣿⣿⠏     "));
        lines.push(format!(" {}⣤⣿⣿⣿⣿⣿⣤{} ", fl, fr));
        lines.push(format!("    ⠘⣿⣿⣿⣿⣿⠃     "));
    }

    lines.push(water_line.to_string());
    lines.push(format!("  {}  ", pose.label));

    lines
}
