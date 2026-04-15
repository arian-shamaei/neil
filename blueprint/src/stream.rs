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
    Diagram(String),
    Table { headers: Vec<String>, rows: Vec<Vec<String>> },
    Chart { title: String, labels: Vec<String>, data: Vec<f64> },
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
    /// Detects SHOW: lines and code fences.
    fn parse_blocks(content: &str) -> Vec<RichBlock> {
        let mut blocks = Vec::new();
        let mut text_buf = String::new();
        let mut lines = content.lines().peekable();

        while let Some(line) = lines.next() {
            if line.starts_with("SHOW:") {
                // Flush text buffer
                if !text_buf.is_empty() {
                    blocks.push(RichBlock::Text(text_buf.trim_end().to_string()));
                    text_buf.clear();
                }

                // Parse SHOW: type=X [params] | content
                if let Some(block) = Self::parse_show_line(line) {
                    blocks.push(block);
                }
            } else if line.starts_with("```") {
                // Flush text buffer
                if !text_buf.is_empty() {
                    blocks.push(RichBlock::Text(text_buf.trim_end().to_string()));
                    text_buf.clear();
                }

                // Code fence
                let lang = line.trim_start_matches('`').trim().to_string();
                let mut code = String::new();
                while let Some(code_line) = lines.next() {
                    if code_line.starts_with("```") { break; }
                    if !code.is_empty() { code.push('\n'); }
                    code.push_str(code_line);
                }
                blocks.push(RichBlock::Code {
                    lang: if lang.is_empty() { "text".into() } else { lang },
                    content: code,
                });
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

    fn parse_show_line(line: &str) -> Option<RichBlock> {
        let after = line.strip_prefix("SHOW:")?;
        let after = after.trim_start();

        // Split on |
        let (params, content) = if let Some(idx) = after.find('|') {
            (after[..idx].trim(), after[idx+1..].trim())
        } else {
            (after.trim(), "")
        };

        // Parse type= and other params
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

    /// Total text length across all blocks (used for cache invalidation)
    pub fn total_text_len(&self) -> usize {
        self.blocks.iter().map(|b| match b {
            RichBlock::Text(t) => t.len(),
            RichBlock::Code { content, .. } => content.len(),
            RichBlock::Diagram(d) => d.len(),
            RichBlock::Table { rows, .. } => rows.len(),
            RichBlock::Chart { data, .. } => data.len(),
        }).sum()
    }

    /// Estimate line count for scroll calculation
    pub fn line_count(&self, width: u16) -> u16 {
        let mut count: u16 = 1; // header line
        for block in &self.blocks {
            count += match block {
                RichBlock::Text(t) => t.lines().count() as u16,
                RichBlock::Code { content, .. } => content.lines().count() as u16 + 2, // borders
                RichBlock::Diagram(d) => d.lines().count() as u16 + 2,
                RichBlock::Table { rows, .. } => rows.len() as u16 + 3, // header + border + rows
                RichBlock::Chart { data, .. } => 3, // label + bars + title
            };
        }
        count + 1 // trailing blank
    }
}
