use serde::{Deserialize, Serialize};

pub const CACHE_TTL_SECONDS: u64 = 604800;
pub const SEMANTIC_SIMILARITY_THRESHOLD: f64 = 0.7;
pub const COMPRESSION_THRESHOLD_BYTES: usize = 1024;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CommandCandidate {
    pub command: String,
    pub label: Option<String>,
    pub requires: Vec<String>,
}

impl CommandCandidate {
    pub fn new(command: String) -> Self {
        Self {
            command,
            label: None,
            requires: Vec::new(),
        }
    }

    pub fn with_label(mut self, label: String) -> Self {
        self.label = Some(label);
        self
    }

    pub fn with_requires(mut self, requires: Vec<String>) -> Self {
        self.requires = requires;
        self
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct CmdEntries {
    pub entries: Vec<CmdEntry>,
}

#[derive(Serialize, Deserialize)]
pub struct CmdEntry {
    pub prompt: String,
    pub candidates: Vec<CommandCandidate>,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Default)]
pub struct ExplainEntries {
    pub entries: Vec<ExplainEntry>,
}

#[derive(Serialize, Deserialize)]
pub struct ExplainEntry {
    pub prompt: String,
    pub response: String,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Default)]
pub struct RagEntries {
    pub entries: Vec<RagEntry>,
}

#[derive(Serialize, Deserialize)]
pub struct RagEntry {
    pub question: String,
    pub response: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CacheAnalytics {
    pub command_entries: usize,
    pub explain_entries: usize,
    pub rag_entries: usize,
    pub total_entries: usize,
    pub expired_entries: usize,
}
