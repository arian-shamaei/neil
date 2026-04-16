use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Complete snapshot of Neil's state, read from files on each tick.
/// Panels read from this -- never modify it.
#[derive(Debug, Clone, Default)]
pub struct NeilState {
    pub neil_home: PathBuf,
    pub now: DateTime<Local>,
    pub heartbeat: HeartbeatState,
    pub palace: PalaceState,
    pub intentions: Vec<Intention>,
    pub failures: Vec<Failure>,
    pub system: SystemState,
    pub essence_files: Vec<String>,
    pub services: Vec<String>,
    pub tick: u64,
    pub max_daily_beats: Option<usize>, // from config.toml, None = no cap
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HeartbeatEntry {
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub improvement: String,
    #[serde(default)]
    pub contribution: String,
}

#[derive(Debug, Clone, Default)]
pub struct HeartbeatState {
    pub entries: Vec<HeartbeatEntry>,
    pub beats_today: usize,
    pub last_beat: String,
}

#[derive(Debug, Clone, Default)]
pub struct PalaceState {
    pub total_notes: usize,
    pub classified: usize,
    pub unclassified: usize,
    pub wings: Vec<WingInfo>,
}

#[derive(Debug, Clone, Default)]
pub struct WingInfo {
    pub name: String,
    pub count: usize,
    pub rooms: Vec<(String, usize)>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Intention {
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub priority: String,
    #[serde(default)]
    pub due: String,
    #[serde(default)]
    pub tag: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub status: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Failure {
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub resolution: String,
}

#[derive(Debug, Clone, Default)]
pub struct SystemState {
    pub uptime: String,
    pub disk_usage: String,
    pub ram_usage: String,
    pub load: String,
    pub autoprompt_active: bool,
    pub queue_count: usize,
}

impl NeilState {
    pub fn load(neil_home: &PathBuf) -> Self {
        let now = Local::now();

        let heartbeat = Self::load_heartbeat(neil_home);
        let palace = Self::load_palace(neil_home);
        let intentions = Self::load_intentions(neil_home);
        let failures = Self::load_failures(neil_home);
        let system = Self::load_system(neil_home);
        let essence_files = Self::list_dir_files(&neil_home.join("essence"), "md");
        let services = Self::list_dir_files(&neil_home.join("services/registry"), "md");

        NeilState {
            neil_home: neil_home.clone(),
            now,
            heartbeat,
            palace,
            intentions,
            failures,
            system,
            essence_files,
            services,
            tick: 0,
            max_daily_beats: Self::load_max_daily(neil_home),
        }
    }

    fn load_max_daily(home: &PathBuf) -> Option<usize> {
        let config = fs::read_to_string(home.join("config.toml")).unwrap_or_default();
        for line in config.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("max_daily") {
                if let Some(val) = trimmed.split('=').nth(1) {
                    let val = val.trim();
                    if val == "0" || val.is_empty() { return None; } // 0 = no cap
                    return val.parse().ok();
                }
            }
        }
        None // no setting = no cap
    }

    fn load_heartbeat(home: &PathBuf) -> HeartbeatState {
        let path = home.join("heartbeat_log.json");
        let content = fs::read_to_string(&path).unwrap_or_default();
        let today = Local::now().format("%Y-%m-%d").to_string();

        let mut entries = Vec::new();

        for line in content.lines() {
            if line.trim().is_empty() { continue; }
            if let Ok(entry) = serde_json::from_str::<HeartbeatEntry>(line) {
                entries.push(entry);
            }
        }

        let last_beat = entries.last()
            .map(|e| e.timestamp.clone())
            .unwrap_or_default();

        // Count beats today from result files (log is capped at 10)
        let history_dir = home.join("tools/autoPrompter/history");
        let beats_today = fs::read_dir(&history_dir)
            .map(|dir| dir.filter_map(|e| e.ok())
                .filter(|e| {
                    let n = e.file_name().to_string_lossy().to_string();
                    n.contains(&today) && n.ends_with(".result.md")
                })
                .count())
            .unwrap_or(0);

        HeartbeatState { entries, beats_today, last_beat }
    }

    fn load_palace(home: &PathBuf) -> PalaceState {
        let rooms_path = home.join("memory/palace/index/rooms.idx");
        let content = fs::read_to_string(&rooms_path).unwrap_or_default();

        let mut wing_map: HashMap<String, HashMap<String, usize>> = HashMap::new();
        let mut total = 0;

        // Count notes
        if let Ok(dir) = fs::read_dir(home.join("memory/palace/notes")) {
            for entry in dir.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".md") && name != "mempalace.yaml" {
                    total += 1;
                }
            }
        }

        // Parse rooms.idx
        for line in content.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 {
                let wing = parts[0].to_string();
                let room = parts[1].to_string();
                *wing_map.entry(wing).or_default().entry(room).or_insert(0) += 1;
            }
        }

        let mut classified = 0;
        let mut wings: Vec<WingInfo> = Vec::new();
        for (name, rooms) in &wing_map {
            let count: usize = rooms.values().sum();
            classified += count;
            let mut room_list: Vec<(String, usize)> = rooms.iter()
                .map(|(r, c)| (r.clone(), *c))
                .collect();
            room_list.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            wings.push(WingInfo { name: name.clone(), count, rooms: room_list });
        }
        wings.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));

        PalaceState {
            total_notes: total,
            classified,
            unclassified: total.saturating_sub(classified),
            wings,
        }
    }

    fn load_intentions(home: &PathBuf) -> Vec<Intention> {
        let path = home.join("intentions.json");
        let content = fs::read_to_string(&path).unwrap_or_default();
        content.lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str::<Intention>(l).ok())
            .collect()
    }

    fn load_failures(home: &PathBuf) -> Vec<Failure> {
        let path = home.join("self/failures.json");
        let content = fs::read_to_string(&path).unwrap_or_default();
        content.lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str::<Failure>(l).ok())
            .collect()
    }

    fn load_system(home: &PathBuf) -> SystemState {
        let queue_count = fs::read_dir(home.join("tools/autoPrompter/queue"))
            .map(|d| d.filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().ends_with(".md"))
                .count())
            .unwrap_or(0);

        SystemState {
            queue_count,
            autoprompt_active: true, // simplified; could probe systemd
            ..Default::default()
        }
    }

    fn list_dir_files(dir: &PathBuf, ext: &str) -> Vec<String> {
        fs::read_dir(dir)
            .map(|d| d.filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().ends_with(&format!(".{}", ext)))
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect())
            .unwrap_or_default()
    }
}

/// A single entry in the command log extracted from a heartbeat result file.
#[derive(Debug, Clone)]
pub enum CommandLogEntry {
    /// A shell command and its output
    Command { cmd: String, output: String },
    /// A memory write (MEMORY: line)
    Memory(String),
    /// A service call (CALL: line)
    ServiceCall(String),
    /// A mempalace operation
    Mempalace(String),
}

/// Load command log from a heartbeat's result file.
/// Finds the result file by matching the prompt name in history/.
pub fn load_command_log(neil_home: &PathBuf, prompt_name: &str) -> Vec<CommandLogEntry> {
    let history_dir = neil_home.join("tools/autoPrompter/history");
    let mut entries = Vec::new();

    // Find the result file matching this prompt
    let result_file = fs::read_dir(&history_dir)
        .ok()
        .and_then(|dir| {
            dir.filter_map(|e| e.ok())
                .find(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    name.ends_with(".result.md") && name.contains(prompt_name)
                })
                .map(|e| e.path())
        });

    let path = match result_file {
        Some(p) => p,
        None => return entries,
    };

    let content = fs::read_to_string(&path).unwrap_or_default();

    // Extract the ## Output section content (between ```...```)
    // The output is wrapped in a code fence. Inner code fences (from stream_action)
    // also use ```, so we find the LAST ``` before the next ## section or EOF.
    let output = match content.find("## Output\n```\n") {
        Some(start) => {
            let body_start = start + "## Output\n```\n".len();
            let rest = &content[body_start..];
            // Find closing: last \n``` before next ## section or end of file
            let body_end = rest.rfind("\n```")
                .map(|i| body_start + i)
                .unwrap_or(content.len());
            &content[body_start..body_end]
        }
        None => return entries,
    };

    // Parse the output for command log entries
    let mut lines = output.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();

        // Bash code blocks (from stream_action)
        if trimmed.starts_with("```bash") || trimmed.starts_with("```sh") {
            let mut cmd = String::new();
            let mut cmd_output = String::new();
            let mut past_cmd = false;
            while let Some(code_line) = lines.next() {
                if code_line.trim().starts_with("```") { break; }
                if !past_cmd && code_line.starts_with("$ ") {
                    if !cmd.is_empty() { cmd.push('\n'); }
                    cmd.push_str(&code_line[2..]);
                } else {
                    past_cmd = true;
                    if !cmd_output.is_empty() { cmd_output.push('\n'); }
                    cmd_output.push_str(code_line);
                }
            }
            if !cmd.is_empty() {
                entries.push(CommandLogEntry::Command { cmd, output: cmd_output });
            }
        }
        // MEMORY: lines (from Claude's output, not inside code blocks)
        else if trimmed.starts_with("MEMORY:") {
            entries.push(CommandLogEntry::Memory(trimmed[7..].trim().to_string()));
        }
        // CALL: lines
        else if trimmed.starts_with("CALL:") {
            entries.push(CommandLogEntry::ServiceCall(trimmed[5..].trim().to_string()));
        }
        // mempalace commands (only actual command lines, not prose mentioning it)
        else if (trimmed.starts_with("$ ") || trimmed.starts_with("mempalace "))
            && (trimmed.contains("mempalace mine") || trimmed.contains("mempalace search"))
        {
            entries.push(CommandLogEntry::Mempalace(trimmed.to_string()));
        }
    }

    entries
}
