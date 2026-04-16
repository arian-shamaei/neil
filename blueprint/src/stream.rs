use chrono::{DateTime, Local};

#[derive(Debug, Clone)]
pub enum EntryKind {
    Neil,
    Human,
    System,
}

#[derive(Debug, Clone)]
pub enum RichBlock {
    Text(String),
    Code { lang: String, content: String },
    FileEdit { path: String, lang: String, lines: Vec<DiffLine> },
    Command { cmd: String, output: String },
    ToolCall { action: String, detail: String },
    Diagram(String),
    Table { headers: Vec<String>, rows: Vec<Vec<String>> },
    Chart { title: String, labels: Vec<String>, data: Vec<f64> },
}

#[derive(Debug, Clone)]
pub enum DiffLine {
    Added(String),
    Removed(String),
    Context(String),
}

#[derive(Debug, Clone)]
pub struct StreamEntry {
    pub kind: EntryKind,
    pub blocks: Vec<RichBlock>,
    pub time: DateTime<Local>,
}

impl StreamEntry {
    pub fn new(kind: EntryKind, content: String) -> Self {
        let blocks = Self::parse_blocks(&content);
        Self {
            kind,
            blocks,
            time: Local::now(),
        }
    }

    /// Parse content into rich blocks.
    /// Detects file edits, commands, tool calls, SHOW: lines, and code fences.
    fn parse_blocks(content: &str) -> Vec<RichBlock> {
        let mut blocks = Vec::new();
        let mut text_buf = String::new();
        let mut lines = content.lines().peekable();

        while let Some(line) = lines.next() {
            // Tool call action lines
            if Self::is_action_line(line) {
                if !text_buf.is_empty() {
                    blocks.push(RichBlock::Text(text_buf.trim_end().to_string()));
                    text_buf.clear();
                }
                let (action, detail) = Self::split_action(line);
                blocks.push(RichBlock::ToolCall { action, detail });
            } else if line.starts_with("SHOW:") {
                if !text_buf.is_empty() {
                    blocks.push(RichBlock::Text(text_buf.trim_end().to_string()));
                    text_buf.clear();
                }
                if let Some(block) = Self::parse_show_line(line) {
                    blocks.push(block);
                }
            } else if line.starts_with("```") {
                // Check if preceding text mentions a file path
                let file_hint = Self::extract_file_hint(&text_buf);

                if !text_buf.is_empty() {
                    // If we extracted a file hint, remove the last line from text_buf
                    if file_hint.is_some() {
                        let trimmed = text_buf.trim_end().to_string();
                        if let Some(last_nl) = trimmed.rfind('\n') {
                            let remaining = &trimmed[..last_nl];
                            if !remaining.trim().is_empty() {
                                blocks.push(RichBlock::Text(remaining.to_string()));
                            }
                        }
                        // else: the entire text_buf was the file hint line, skip it
                    } else {
                        blocks.push(RichBlock::Text(text_buf.trim_end().to_string()));
                    }
                    text_buf.clear();
                }

                let lang = line.trim_start_matches('`').trim().to_string();
                let mut code_lines: Vec<String> = Vec::new();
                while let Some(code_line) = lines.next() {
                    if code_line.starts_with("```") { break; }
                    code_lines.push(code_line.to_string());
                }

                let block = Self::classify_code_block(
                    &lang,
                    &code_lines,
                    file_hint,
                );
                blocks.push(block);
            } else {
                text_buf.push_str(line);
                text_buf.push('\n');
            }
        }

        if !text_buf.is_empty() {
            blocks.push(RichBlock::Text(text_buf.trim_end().to_string()));
        }

        if blocks.is_empty() {
            blocks.push(RichBlock::Text(content.to_string()));
        }

        blocks
    }

    /// Check if a line is an action/tool call prefix
    fn is_action_line(line: &str) -> bool {
        let trimmed = line.trim_start();
        trimmed.starts_with("MEMORY:")
            || trimmed.starts_with("CALL:")
            || trimmed.starts_with("NOTIFY:")
            || trimmed.starts_with("INTEND:")
            || trimmed.starts_with("DONE:")
            || trimmed.starts_with("FAIL:")
            || trimmed.starts_with("HEARTBEAT:")
            || trimmed.starts_with("PROMPT:")
    }

    /// Split an action line into (action_name, detail)
    fn split_action(line: &str) -> (String, String) {
        let trimmed = line.trim_start();
        if let Some(colon) = trimmed.find(':') {
            let action = trimmed[..colon].to_string();
            let detail = trimmed[colon + 1..].trim().to_string();
            (action, detail)
        } else {
            (trimmed.to_string(), String::new())
        }
    }

    /// Look at the last line of text_buf for a file path hint.
    /// Patterns: "editing `path`", "writing to path", "in `path`:", "file: path", etc.
    fn extract_file_hint(text_buf: &str) -> Option<String> {
        let trimmed = text_buf.trim_end();
        let last_line = trimmed.lines().last()?;
        let lower = last_line.to_lowercase();

        // Common patterns Claude uses before showing file content
        let has_file_context = lower.contains("edit")
            || lower.contains("writ")
            || lower.contains("updat")
            || lower.contains("modif")
            || lower.contains("chang")
            || lower.contains("creat")
            || lower.contains("add")
            || lower.contains("in `")
            || lower.contains("file:")
            || lower.ends_with(':');

        if !has_file_context {
            return None;
        }

        // Extract path from backticks
        if let Some(start) = last_line.find('`') {
            if let Some(end) = last_line[start + 1..].find('`') {
                let path = &last_line[start + 1..start + 1 + end];
                if Self::looks_like_path(path) {
                    return Some(path.to_string());
                }
            }
        }

        // Extract path from bold markers
        if let Some(start) = last_line.find("**") {
            if let Some(end) = last_line[start + 2..].find("**") {
                let path = &last_line[start + 2..start + 2 + end];
                if Self::looks_like_path(path) {
                    return Some(path.to_string());
                }
            }
        }

        // Look for bare file paths (contains / or \ and a file extension)
        for word in last_line.split_whitespace() {
            let clean = word.trim_matches(|c: char| c == '`' || c == '*' || c == '"' || c == '\'' || c == ':' || c == ',');
            if Self::looks_like_path(clean) {
                return Some(clean.to_string());
            }
        }

        None
    }

    /// Heuristic: does this string look like a file path?
    fn looks_like_path(s: &str) -> bool {
        if s.len() < 3 { return false; }
        let has_sep = s.contains('/') || s.contains('\\');
        let has_ext = s.contains('.') && {
            let after_dot = s.rsplit('.').next().unwrap_or("");
            matches!(after_dot, "rs" | "py" | "sh" | "js" | "ts" | "c" | "h" | "md"
                | "toml" | "yaml" | "yml" | "json" | "txt" | "cfg" | "conf"
                | "html" | "css" | "go" | "rb" | "java" | "idx" | "key" | "lock")
        };
        has_sep || has_ext
    }

    /// Classify a code block as FileEdit, Command, or plain Code
    fn classify_code_block(lang: &str, code_lines: &[String], file_hint: Option<String>) -> RichBlock {
        let effective_lang = if lang.is_empty() { "text" } else { lang };

        // Check if this is a command block (bash/sh/shell language or $ prefix)
        let is_command_lang = matches!(effective_lang, "bash" | "sh" | "shell" | "zsh" | "console" | "terminal");
        let starts_with_cmd = code_lines.first()
            .map(|l| l.starts_with("$ ") || l.starts_with("# ") || l.starts_with("% "))
            .unwrap_or(false);

        if is_command_lang || starts_with_cmd {
            // Split into command and output
            let mut cmd = String::new();
            let mut output = String::new();
            let mut past_cmd = false;

            for line in code_lines {
                if !past_cmd && (line.starts_with("$ ") || line.starts_with("# ") || line.starts_with("% ")) {
                    if !cmd.is_empty() { cmd.push('\n'); }
                    cmd.push_str(line.trim_start_matches(|c| c == '$' || c == '#' || c == '%').trim());
                } else {
                    // If first line has no prefix but lang is bash, treat as command
                    if !past_cmd && cmd.is_empty() && is_command_lang {
                        cmd.push_str(line);
                    } else {
                        past_cmd = true;
                        if !output.is_empty() { output.push('\n'); }
                        output.push_str(line);
                    }
                }
            }

            if !cmd.is_empty() {
                return RichBlock::Command { cmd, output };
            }
        }

        // Check if this looks like a diff / file edit
        let has_diff_markers = code_lines.iter().any(|l| l.starts_with('+') || l.starts_with('-'));
        let diff_line_ratio = if code_lines.is_empty() { 0.0 } else {
            code_lines.iter()
                .filter(|l| l.starts_with('+') || l.starts_with('-') || l.starts_with(' ') || l.starts_with("@@"))
                .count() as f64 / code_lines.len() as f64
        };

        // If there's a file hint and the content has diff-like lines, or high ratio of +/- lines
        if has_diff_markers && (file_hint.is_some() || diff_line_ratio > 0.3) {
            let path = file_hint.unwrap_or_default();
            let diff_lines: Vec<DiffLine> = code_lines.iter().map(|l| {
                if l.starts_with('+') {
                    DiffLine::Added(l[1..].to_string())
                } else if l.starts_with('-') {
                    DiffLine::Removed(l[1..].to_string())
                } else if l.starts_with("@@") {
                    DiffLine::Context(l.to_string())
                } else {
                    DiffLine::Context(l.trim_start_matches(' ').to_string())
                }
            }).collect();

            return RichBlock::FileEdit {
                path,
                lang: effective_lang.to_string(),
                lines: diff_lines,
            };
        }

        // If there's a file hint but no diff markers, still show as a file-context code block
        if let Some(path) = file_hint {
            return RichBlock::FileEdit {
                path,
                lang: effective_lang.to_string(),
                lines: code_lines.iter().map(|l| DiffLine::Context(l.to_string())).collect(),
            };
        }

        // Plain code block
        RichBlock::Code {
            lang: effective_lang.to_string(),
            content: code_lines.join("\n"),
        }
    }

    fn parse_show_line(line: &str) -> Option<RichBlock> {
        let after = line.strip_prefix("SHOW:")?;
        let after = after.trim_start();

        let (params, content) = if let Some(idx) = after.find('|') {
            (after[..idx].trim(), after[idx+1..].trim())
        } else {
            (after.trim(), "")
        };

        let mut show_type = "";
        let mut lang = "text";
        let mut labels_str = "";
        let mut data_str = "";

        for tok in params.split_whitespace() {
            if let Some(v) = tok.strip_prefix("type=") { show_type = v; }
            else if let Some(v) = tok.strip_prefix("lang=") { lang = v; }
            else if let Some(v) = tok.strip_prefix("labels=") { labels_str = v; }
            else if let Some(v) = tok.strip_prefix("data=") { data_str = v; }
        }

        match show_type {
            "code" => Some(RichBlock::Code {
                lang: lang.to_string(),
                content: content.replace("\\n", "\n"),
            }),
            "diagram" => Some(RichBlock::Diagram(content.replace("\\n", "\n"))),
            "table" => {
                let mut rows_iter = content.split("\\n");
                let headers: Vec<String> = rows_iter.next()
                    .unwrap_or("")
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect();
                let rows: Vec<Vec<String>> = rows_iter
                    .map(|r| r.split(',').map(|s| s.trim().to_string()).collect())
                    .collect();
                Some(RichBlock::Table { headers, rows })
            }
            "chart" => {
                let labels: Vec<String> = labels_str.split(',')
                    .map(|s| s.trim().to_string())
                    .collect();
                let data: Vec<f64> = data_str.split(',')
                    .filter_map(|s| s.trim().parse().ok())
                    .collect();
                Some(RichBlock::Chart {
                    title: content.to_string(),
                    labels,
                    data,
                })
            }
            _ => Some(RichBlock::Text(format!("[SHOW:{}] {}", show_type, content))),
        }
    }

    /// Total text length for cache invalidation
    pub fn total_text_len(&self) -> usize {
        self.blocks.iter().map(|b| match b {
            RichBlock::Text(t) => t.len(),
            RichBlock::Code { content, .. } => content.len(),
            RichBlock::FileEdit { lines, .. } => lines.len(),
            RichBlock::Command { cmd, output } => cmd.len() + output.len(),
            RichBlock::ToolCall { detail, .. } => detail.len(),
            RichBlock::Diagram(d) => d.len(),
            RichBlock::Table { rows, .. } => rows.len(),
            RichBlock::Chart { data, .. } => data.len(),
        }).sum()
    }

    /// Estimate line count for scroll calculation
    pub fn line_count(&self, _width: u16) -> u16 {
        let mut count: u16 = 1; // header line
        for block in &self.blocks {
            count += match block {
                RichBlock::Text(t) => t.lines().count() as u16,
                RichBlock::Code { content, .. } => content.lines().count() as u16 + 2,
                RichBlock::FileEdit { lines, .. } => lines.len() as u16 + 2, // header + border + lines
                RichBlock::Command { cmd, output } => {
                    1 + cmd.lines().count() as u16
                        + if output.is_empty() { 0 } else { output.lines().count() as u16 + 1 }
                }
                RichBlock::ToolCall { .. } => 1,
                RichBlock::Diagram(d) => d.lines().count() as u16 + 2,
                RichBlock::Table { rows, .. } => rows.len() as u16 + 3,
                RichBlock::Chart { .. } => 3,
            };
        }
        count + 1
    }
}
