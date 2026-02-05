use bincode::{deserialize, serialize};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use memmap2::Mmap;
use serde::{Deserialize, Serialize};
use shared::types::Result;
use std::collections::HashSet;
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
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
const COMPRESSION_THRESHOLD_BYTES: usize = 1024; // Compress files larger than 1KB

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

pub struct CacheManager {
    cache_dir: PathBuf,
    memory_mapped_io: bool,
}

impl CacheManager {
    pub fn new(cache_dir: PathBuf, memory_mapped_io: bool) -> Self {
        Self {
            cache_dir,
            memory_mapped_io,
        }
    }

    fn validate_command_syntax(command: &str) -> bool {
        // Check if command contains any dangerous patterns
        let dangerous_patterns = [
            "rm -rf", "rm -r", "dd if=", "mkfs", "format", "shred", "wipe", "fdisk", "sfdisk",
            "parted", "dd of=", "> /dev", "< /dev", "2> /dev",
        ];

        if dangerous_patterns
            .iter()
            .any(|pattern| command.to_lowercase().contains(pattern))
        {
            return false;
        }

        // Check for shell injection patterns
        let injection_patterns = [
            "; rm", "&& rm", "|| rm", "$(rm", "`rm`", "| rm", "> rm", "< rm",
        ];

        if injection_patterns
            .iter()
            .any(|pattern| command.contains(pattern))
        {
            return false;
        }

        true
    }

    fn validate_command_exists(command: &str) -> bool {
        // Extract the first word as the command
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return false;
        }

        let cmd_name = parts[0];

        // Skip validation for common built-in commands
        let builtins = [
            "echo", "cd", "pwd", "ls", "cat", "grep", "find", "which", "type",
        ];
        if builtins.contains(&cmd_name) {
            return true;
        }

        // Check if command exists in PATH without executing it
        // Use `which` command to check availability without running the command
        match std::process::Command::new("which")
            .arg(cmd_name)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
        {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    fn validate_command(command: &str) -> bool {
        if command.trim().is_empty() {
            return false;
        }

        if !Self::validate_command_syntax(command) {
            return false;
        }

        if !Self::validate_command_exists(command) {
            return false;
        }

        true
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

        let data = std::fs::read(&cache_path)?;

        if data.first().map(|&b| b == 0x1f).unwrap_or(false) {
            let decoder = GzDecoder::new(&data[..]);
            let mut decoder = decoder;
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed)?;
            let cache: T = deserialize(&decompressed).unwrap_or_default();
            Ok(cache)
        } else {
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

        if serialized.len() > COMPRESSION_THRESHOLD_BYTES {
            let file = File::create(&cache_path)?;
            let mut encoder = GzEncoder::new(file, flate2::Compression::default());
            encoder.write_all(&serialized)?;
            encoder.finish()?;
        } else {
            std::fs::write(&cache_path, serialized)?;
        }

        Ok(())
    }

    fn load_cache_file<T: serde::de::DeserializeOwned>(&self, cache_type: &str) -> Result<T> {
        let cache_path = self.get_cache_path(cache_type);

        if !cache_path.exists() {
            return Ok(T::default());
        }

        let data = std::fs::read(&cache_path)?;

        if data.first().map(|&b| b == 0x1f).unwrap_or(false) {
            let decoder = GzDecoder::new(&data[..]);
            let mut decoder = decoder;
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed)?;
            let cache: T = deserialize(&decompressed).unwrap_or_default();
            Ok(cache)
        } else {
            let cache: T = deserialize(&data).unwrap_or_default();
            Ok(cache)
        }
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

                // Validate cached commands
                let valid_candidates: Vec<CommandCandidate> = entry
                    .candidates
                    .iter()
                    .filter(|candidate| Self::validate_command(&candidate.command))
                    .cloned()
                    .collect();

                if valid_candidates.is_empty() {
                    // All cached commands are invalid, remove this entry
                    self.remove_cache_entry("commands", prompt)?;
                    return Ok(None);
                }

                return Ok(Some(valid_candidates));
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
                // Validate cached commands
                let valid_candidates: Vec<CommandCandidate> = entry
                    .candidates
                    .iter()
                    .filter(|candidate| Self::validate_command(&candidate.command))
                    .cloned()
                    .collect();

                if valid_candidates.is_empty() {
                    // All cached commands are invalid, remove this entry
                    self.remove_cache_entry("commands", &entry.prompt)?;
                    return Ok(None);
                }

                Ok(Some(valid_candidates))
            }
        } else {
            Ok(None)
        }
    }

    fn remove_cache_entry(&self, cache_type: &str, prompt: &str) -> Result<()> {
        let mut cache: CacheFile = self.load_cache_file(cache_type)?;
        cache.entries.retain(|entry| entry.prompt != prompt);
        self.save_cache_file(cache_type, &cache)?;
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_command_syntax() {
        let cache_manager = CacheManager::new(PathBuf::from("/tmp"), false);

        assert!(cache_manager.validate_command_syntax("ls -la"));
        assert!(cache_manager.validate_command_syntax("echo hello"));
        assert!(!cache_manager.validate_command_syntax("rm -rf /"));
        assert!(!cache_manager.validate_command_syntax("dd if=/dev/zero"));
        assert!(!cache_manager.validate_command_syntax("echo; rm -rf /"));
    }

    #[test]
    fn test_validate_command_exists() {
        let cache_manager = CacheManager::new(PathBuf::from("/tmp"), false);

        assert!(cache_manager.validate_command_exists("ls"));
        assert!(cache_manager.validate_command_exists("echo"));
        assert!(cache_manager.validate_command_exists("cat"));
        assert!(!cache_manager.validate_command_exists("nonexistent_command_xyz123"));
    }

    #[test]
    fn test_validate_command() {
        let cache_manager = CacheManager::new(PathBuf::from("/tmp"), false);

        assert!(cache_manager.validate_command("ls -la"));
        assert!(cache_manager.validate_command("echo hello world"));
        assert!(!cache_manager.validate_command("rm -rf /"));
        assert!(!cache_manager.validate_command("nonexistent_command_xyz123"));
        assert!(!cache_manager.validate_command(""));
        assert!(!cache_manager.validate_command("   "));
    }
}
