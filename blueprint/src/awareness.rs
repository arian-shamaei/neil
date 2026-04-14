use std::fs;
use std::path::PathBuf;
use serde::Serialize;
use chrono::Local;

/// State snapshot written by the TUI every tick.
/// Neil reads this via observe.sh to know what the user sees.
#[derive(Debug, Serialize)]
pub struct BlueprintState {
    pub timestamp: String,
    pub running: bool,
    pub view: String,           // "chat", "panel:Memory", "panel_selector"
    pub terminal_size: (u16, u16), // (width, height)
    pub stream_length: usize,   // total conversation entries
    pub scroll_offset: i32,     // how far scrolled from bottom
    pub auto_scroll: bool,      // true = user at bottom
    pub input_buffer: String,   // what user is typing (not yet sent)
    pub last_user_message: String, // last sent message
    pub sidebar_visible: bool,
    pub user_active: bool,      // true if user typed in last 60s
    pub last_input_time: String,
    pub streaming: bool,        // true if Neil is producing output live
    pub stream_chars: usize,    // bytes received so far in live stream
}

impl BlueprintState {
    pub fn write(&self, neil_home: &PathBuf) {
        let path = neil_home.join(".blueprint_state.json");
        if let Ok(json) = serde_json::to_string(self) {
            let _ = fs::write(path, json);
        }
    }

    pub fn clear(neil_home: &PathBuf) {
        let path = neil_home.join(".blueprint_state.json");
        let stopped = serde_json::json!({
            "timestamp": Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
            "running": false
        });
        let _ = fs::write(path, stopped.to_string());
    }
}
