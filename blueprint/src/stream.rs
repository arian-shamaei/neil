use chrono::{DateTime, Local};

#[derive(Debug, Clone)]
pub enum EntryKind {
    Neil,    // Neil's responses
    Human,   // Human's messages
    System,  // System notifications (thinking, errors, status)
}

#[derive(Debug, Clone)]
pub struct StreamEntry {
    pub kind: EntryKind,
    pub content: String,
    pub time: DateTime<Local>,
}

impl StreamEntry {
    pub fn new(kind: EntryKind, content: String) -> Self {
        Self {
            kind,
            content,
            time: Local::now(),
        }
    }
}
