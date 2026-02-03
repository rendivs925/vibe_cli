use bincode::{deserialize, serialize};
use memmap2::Mmap;
use serde::{Deserialize, Serialize};
use shared::types::Result;
use std::collections::HashSet;
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

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

pub struct UnifiedCacheManager {
    cache_dir: PathBuf,
    memory_mapped_io: bool,
}

impl UnifiedCacheManager {
    pub fn new(cache_dir: PathBuf, memory_mapped_io: bool) -> Self {
        Self {
            cache_dir,
            memory_mapped_io,
        }
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

    fn get_cache_path(&self, cache_type: &str) -> PathBuf {
        let mut path = self.cache_dir.clone();
        path.push(format!("{}.cache", cache_type));
        path
    }

    fn load_cache_file<T: serde::de::DeserializeOwned>(&self, cache_type: &str) -> Result<T> {
        let cache_path = self.get_cache_path(cache_type);

        if !cache_path.exists() {
            return Ok(T::default());
        }

        if self.memory_mapped_io {
            let file = File::open(&cache_path)?;
            let mmap = unsafe { Mmap::map(&file)? };
            let cache: T = deserialize(&mmap).unwrap_or_default();
            Ok(cache)
        } else {
            let data = std::fs::read(&cache_path)?;
            let cache: T = deserialize(&data).unwrap_or_default();
            Ok(cache)
        }
    }

    fn save_cache_file<T: serde::Serialize>(&self, cache_type: &str, cache: &T) -> Result<()> {
        let cache_path = self.get_cache_path(cache_type);

        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let serialized = serialize(cache)?;

        if self.memory_mapped_io {
            // For mmap, we write directly to file
            let mut file = File::create(&cache_path)?;
            file.write_all(&serialized)?;
        } else {
            std::fs::write(&cache_path, serialized)?;
        }

        Ok(())
    }

    pub fn load_command_cached(&self, prompt: &str) -> Result<Option<Vec<CommandCandidate>>> {
        let mut cache: CacheFile = self.load_cache_file("commands")?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        cache
            .entries
            .retain(|entry| now - entry.timestamp < CACHE_TTL_SECONDS);

        // Save cleaned cache
        self.save_cache_file("commands", &cache)?;

        // Exact match
        for entry in &cache.entries {
            if entry.prompt == prompt {
                if entry.candidates.is_empty() {
                    return Ok(None);
                }
                return Ok(Some(entry.candidates.clone()));
            }
        }

        // Semantic match
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

    pub fn save_command_cached(
        &self,
        prompt: &str,
        candidates: Vec<CommandCandidate>,
    ) -> Result<()> {
        let mut cache: CacheFile = self.load_cache_file("commands")?;

        cache.entries.push(CacheEntry {
            prompt: prompt.to_string(),
            candidates,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        });

        self.save_cache_file("commands", &cache)?;
        Ok(())
    }

    pub fn load_explain_cached(&self, prompt: &str) -> Result<Option<String>> {
        let mut cache: ExplainCacheFile = self.load_cache_file("explain")?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        cache.entries.retain(|entry| now - entry.timestamp < 604800);

        self.save_cache_file("explain", &cache)?;

        for entry in &cache.entries {
            if entry.prompt == prompt {
                return Ok(Some(entry.response.clone()));
            }
        }
        Ok(None)
    }

    pub fn save_explain_cached(&self, prompt: &str, response: &str) -> Result<()> {
        let mut cache: ExplainCacheFile = self.load_cache_file("explain")?;

        cache.entries.push(ExplainCacheEntry {
            prompt: prompt.to_string(),
            response: response.to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        });

        self.save_cache_file("explain", &cache)?;
        Ok(())
    }

    pub fn load_rag_cached(&self, question: &str) -> Result<Option<String>> {
        let mut cache: RagCacheFile = self.load_cache_file("rag")?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        cache.entries.retain(|entry| now - entry.timestamp < 604800);

        self.save_cache_file("rag", &cache)?;

        for entry in &cache.entries {
            if entry.question == question {
                return Ok(Some(entry.response.clone()));
            }
        }
        Ok(None)
    }

    pub fn save_rag_cached(&self, question: &str, response: &str) -> Result<()> {
        let mut cache: RagCacheFile = self.load_cache_file("rag")?;

        cache.entries.push(RagCacheEntry {
            question: question.to_string(),
            response: response.to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        });

        self.save_cache_file("rag", &cache)?;
        Ok(())
    }
}
