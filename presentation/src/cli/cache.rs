use bincode::{deserialize, serialize};
use serde::{Deserialize, Serialize};
use shared::types::Result;
use std::collections::HashSet;
use std::path::PathBuf;

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

const CACHE_TTL_SECONDS: u64 = 604800;
const SEMANTIC_SIMILARITY_THRESHOLD: f64 = 0.7;

#[derive(Serialize, Deserialize, Default)]
pub struct CacheFile {
    pub entries: Vec<CacheEntry>,
}

// Legacy cache entry for backward compatibility
#[derive(Serialize, Deserialize)]
pub struct LegacyCacheEntry {
    pub prompt: String,
    pub command: String,
    pub timestamp: u64,
}

impl From<LegacyCacheEntry> for CacheEntry {
    fn from(legacy: LegacyCacheEntry) -> Self {
        Self {
            prompt: legacy.prompt,
            candidates: vec![CommandCandidate::new(legacy.command)],
            timestamp: legacy.timestamp,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct CacheEntry {
    pub prompt: String,
    pub candidates: Vec<CommandCandidate>,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Default)]
pub struct ExplainCacheFile {
    pub entries: Vec<ExplainCacheEntry>,
}

#[derive(Serialize, Deserialize)]
pub struct ExplainCacheEntry {
    pub prompt: String,
    pub response: String,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Default)]
pub struct RagCacheFile {
    pub entries: Vec<RagCacheEntry>,
}

#[derive(Serialize, Deserialize)]
pub struct RagCacheEntry {
    pub question: String,
    pub response: String,
    pub timestamp: u64,
}

pub struct CacheManager {
    cache_path: PathBuf,
}

impl CacheManager {
    pub fn new(cache_path: PathBuf) -> Self {
        Self { cache_path }
    }

    pub fn cache_path(&self) -> &PathBuf {
        &self.cache_path
    }

    fn normalize_text(text: &str) -> String {
        text.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ")
    }

    fn semantic_similarity(prompt1: &str, prompt2: &str) -> f64 {
        let norm1 = Self::normalize_text(prompt1);
        let norm2 = Self::normalize_text(prompt2);

        if norm1 == norm2 {
            return 1.0;
        }

        let words1: HashSet<&str> = norm1.split_whitespace().collect();
        let words2: HashSet<&str> = norm2.split_whitespace().collect();

        let intersection: HashSet<&str> = words1.intersection(&words2).cloned().collect();
        let union: HashSet<&str> = words1.union(&words2).cloned().collect();

        if union.is_empty() {
            return 0.0;
        }

        intersection.len() as f64 / union.len() as f64
    }

    #[allow(dead_code)]
    fn clean_command_output(raw: &str) -> String {
        let trimmed = raw.trim();
        if trimmed.starts_with("```") && trimmed.ends_with("```") {
            let lines: Vec<&str> = trimmed.lines().collect();
            if lines.len() >= 3 {
                if lines[0].trim().starts_with("```") && lines.last().unwrap().trim() == "```" {
                    return lines[1..lines.len() - 1].join("\n").trim().to_string();
                }
            }
        }
        trimmed.to_string()
    }

    pub fn load_cached(&self, prompt: &str) -> Result<Option<Vec<CommandCandidate>>> {
        if !self.cache_path.exists() {
            return Ok(None);
        }

        let data = std::fs::read(&self.cache_path)?;

        let mut cache: CacheFile = deserialize(&data).unwrap_or_default();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        cache
            .entries
            .retain(|entry| now - entry.timestamp < CACHE_TTL_SECONDS);

        if let Some(parent) = self.cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let serialized = serialize(&cache)?;
        std::fs::write(&self.cache_path, serialized)?;

        for entry in &cache.entries {
            if entry.prompt == prompt {
                if entry.candidates.is_empty() {
                    return Ok(None);
                }
                return Ok(Some(entry.candidates.clone()));
            }
        }

        let mut best_match: Option<&CacheEntry> = None;
        let mut best_similarity = 0.0;

        for entry in &cache.entries {
            let similarity = Self::semantic_similarity(prompt, &entry.prompt);
            if similarity > best_similarity && similarity >= SEMANTIC_SIMILARITY_THRESHOLD {
                best_similarity = similarity;
                best_match = Some(entry);
            }
        }

        if let Some(entry) = best_match {
            if entry.candidates.is_empty() {
                Ok(None)
            } else {
                Ok(Some(entry.candidates.clone()))
            }
        } else {
            Ok(None)
        }
    }

    pub fn save_cached(&self, prompt: &str, candidates: Vec<CommandCandidate>) -> Result<()> {
        let mut cache = if self.cache_path.exists() {
            let data = std::fs::read(&self.cache_path).unwrap_or_default();
            deserialize::<CacheFile>(&data).unwrap_or_default()
        } else {
            CacheFile::default()
        };

        cache.entries.push(CacheEntry {
            prompt: prompt.to_string(),
            candidates,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        });

        if let Some(parent) = self.cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let serialized = serialize(&cache)?;
        std::fs::write(&self.cache_path, serialized)?;

        Ok(())
    }
}

pub struct ExplainCacheManager {
    cache_path: PathBuf,
}

impl ExplainCacheManager {
    pub fn new(cache_path: PathBuf) -> Self {
        Self { cache_path }
    }

    pub fn cache_path(&self) -> &PathBuf {
        &self.cache_path
    }

    pub fn load_cached(&self, prompt: &str) -> Result<Option<String>> {
        if !self.cache_path.exists() {
            return Ok(None);
        }

        let data = std::fs::read(&self.cache_path)?;
        let mut cache: ExplainCacheFile = deserialize(&data).unwrap_or_default();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        cache.entries.retain(|entry| now - entry.timestamp < 604800);

        if let Some(parent) = self.cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let serialized = serialize(&cache)?;
        std::fs::write(&self.cache_path, serialized)?;

        for entry in &cache.entries {
            if entry.prompt == prompt {
                return Ok(Some(entry.response.clone()));
            }
        }
        Ok(None)
    }

    pub fn save_cached(&self, prompt: &str, response: &str) -> Result<()> {
        let mut cache = if self.cache_path.exists() {
            let data = std::fs::read(&self.cache_path).unwrap_or_default();
            deserialize::<ExplainCacheFile>(&data).unwrap_or_default()
        } else {
            ExplainCacheFile::default()
        };

        cache.entries.push(ExplainCacheEntry {
            prompt: prompt.to_string(),
            response: response.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        });

        if let Some(parent) = self.cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let serialized = serialize(&cache)?;
        std::fs::write(&self.cache_path, serialized)?;

        Ok(())
    }
}

pub struct RagCacheManager {
    cache_path: PathBuf,
}

impl RagCacheManager {
    pub fn new(cache_path: PathBuf) -> Self {
        Self { cache_path }
    }

    pub fn cache_path(&self) -> &PathBuf {
        &self.cache_path
    }

    pub fn load_cached(&self, question: &str) -> Result<Option<String>> {
        if !self.cache_path.exists() {
            return Ok(None);
        }

        let data = std::fs::read(&self.cache_path)?;
        let mut cache: RagCacheFile = deserialize(&data).unwrap_or_default();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        cache.entries.retain(|entry| now - entry.timestamp < 604800);

        if let Some(parent) = self.cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let serialized = serialize(&cache)?;
        std::fs::write(&self.cache_path, serialized)?;

        for entry in &cache.entries {
            if entry.question == question {
                return Ok(Some(entry.response.clone()));
            }
        }
        Ok(None)
    }

    pub fn save_cached(&self, question: &str, response: &str) -> Result<()> {
        let mut cache = if self.cache_path.exists() {
            let data = std::fs::read(&self.cache_path).unwrap_or_default();
            deserialize::<RagCacheFile>(&data).unwrap_or_default()
        } else {
            RagCacheFile::default()
        };

        cache.entries.push(RagCacheEntry {
            question: question.to_string(),
            response: response.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        });

        if let Some(parent) = self.cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let serialized = serialize(&cache)?;
        std::fs::write(&self.cache_path, serialized)?;

        Ok(())
    }
}
