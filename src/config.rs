use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

// Cache entries expire after 7 days (604800 seconds)
const CACHE_TTL_SECONDS: u64 = 604800;

// Semantic similarity threshold (0.0 to 1.0)
const SEMANTIC_SIMILARITY_THRESHOLD: f64 = 0.7;

fn find_project_root() -> Option<String> {
    let mut current = std::env::current_dir().ok()?;
    loop {
        if current.join("Cargo.toml").exists() {
            return Some(current.display().to_string());
        }
        if !current.pop() {
            break;
        }
    }
    None
}

fn project_cache_suffix() -> String {
    if let Some(root) = find_project_root() {
        let mut hasher = DefaultHasher::new();
        root.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    } else {
        "global".to_string()
    }
}

#[derive(Clone)]
pub struct Config {
    pub model: String,
    pub endpoint: String,
    pub safe_mode: bool,
    pub cache_enabled: bool,
    pub copy_to_clipboard: bool,
    cache_path: PathBuf,
}

#[derive(Serialize, Deserialize, Default)]
struct CacheFile {
    entries: Vec<CacheEntry>,
}

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    prompt: String,
    command: String,
    timestamp: u64,
}

impl Config {
    /// Normalize text for semantic comparison
    fn normalize_text(text: &str) -> String {
        text.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ")
    }

    /// Calculate semantic similarity between two prompts
    fn semantic_similarity(prompt1: &str, prompt2: &str) -> f64 {
        let norm1 = Self::normalize_text(prompt1);
        let norm2 = Self::normalize_text(prompt2);

        if norm1 == norm2 {
            return 1.0;
        }

        let words1: std::collections::HashSet<&str> = norm1.split_whitespace().collect();
        let words2: std::collections::HashSet<&str> = norm2.split_whitespace().collect();

        let intersection: std::collections::HashSet<&str> = words1.intersection(&words2).cloned().collect();
        let union: std::collections::HashSet<&str> = words1.union(&words2).cloned().collect();

        if union.is_empty() {
            return 0.0;
        }

        intersection.len() as f64 / union.len() as f64
    }
    /// Clean command output by removing markdown code blocks
    fn clean_command_output(raw: &str) -> String {
        let trimmed = raw.trim();
        if trimmed.starts_with("```") && trimmed.ends_with("```") {
            // Remove the first and last lines if they are ``` or ```sh
            let lines: Vec<&str> = trimmed.lines().collect();
            if lines.len() >= 3 {
                if lines[0].trim().starts_with("```") && lines.last().unwrap().trim() == "```" {
                    return lines[1..lines.len()-1].join("\n").trim().to_string();
                }
            }
        }
        trimmed.to_string()
    }
    pub fn new(safe_mode: bool, cache_enabled: bool, copy_to_clipboard: bool) -> Self {
        let model =
            std::env::var("BASE_MODEL").unwrap_or_else(|_| "qwen2.5:1.5b-instruct".to_string());
        let endpoint =
            std::env::var("OLLAMA_ENDPOINT").unwrap_or_else(|_| "http://localhost:11434/api/chat".to_string());

        let cache_path = Self::default_cache_path();

        Self {
            model,
            endpoint,
            safe_mode,
            cache_enabled,
            copy_to_clipboard,
            cache_path,
        }
    }

    fn default_cache_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let mut path = PathBuf::from(home);
        path.push(".local");
        path.push("share");
        path.push("vibe_cli");
        let suffix = project_cache_suffix();
        path.push(format!("{}_cache.bin", suffix));
        path
    }

    pub fn load_cached(&self, prompt: &str) -> Result<Option<String>> {
        if !self.cache_path.exists() {
            return Ok(None);
        }

        let data = fs::read(&self.cache_path)
            .with_context(|| format!("Failed to read cache file at {:?}", self.cache_path))?;

        let mut cache: CacheFile = bincode::deserialize(&data).unwrap_or_default();

        // Remove expired entries
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        cache.entries.retain(|entry| now - entry.timestamp < CACHE_TTL_SECONDS);

        // Save cleaned cache back to disk
        if let Some(parent) = self.cache_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let serialized = bincode::serialize(&cache)?;
        fs::write(&self.cache_path, serialized)?;

        // First try exact match
        for entry in &cache.entries {
            if entry.prompt == prompt {
                return Ok(Some(Self::clean_command_output(&entry.command)));
            }
        }

        // Then try semantic similarity
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
            return Ok(Some(Self::clean_command_output(&entry.command)));
        }

        Ok(None)
    }

    pub fn save_cached(&self, prompt: &str, command: &str) -> Result<()> {
        let mut cache = if self.cache_path.exists() {
            let data = fs::read(&self.cache_path).unwrap_or_default();
            bincode::deserialize::<CacheFile>(&data).unwrap_or_default()
        } else {
            CacheFile::default()
        };

        cache.entries.push(CacheEntry {
            prompt: prompt.to_string(),
            command: Self::clean_command_output(command),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        });

        if let Some(parent) = self.cache_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let serialized = bincode::serialize(&cache)?;
        fs::write(&self.cache_path, serialized)?;

        Ok(())
    }

    pub fn clear_cache(&self) -> Result<()> {
        if self.cache_path.exists() {
            fs::remove_file(&self.cache_path)?;
        }
        Ok(())
    }
}
