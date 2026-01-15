use crate::types::{CacheFile, CacheEntry, ExplainCacheFile, ExplainCacheEntry, RagCacheFile, RagCacheEntry};
use crate::utils::clean_command_output;
use crate::analysis::validate_command_syntax;
use std::path::PathBuf;
use anyhow::Result;

// Cache entries expire after 7 days (604800 seconds)
const CACHE_TTL_SECONDS: u64 = 604800;

// Semantic similarity threshold (0.0 to 1.0)
const SEMANTIC_SIMILARITY_THRESHOLD: f64 = 0.7;

/// Get the default cache path
pub fn default_cache_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let mut path = PathBuf::from(home);
    path.push(".local");
    path.push("share");
    path.push("vibe_cli");
    let suffix = super::utils::project_cache_suffix();
    path.push(format!("{}_cli_cache.json", suffix));
    path
}

/// Normalize text for semantic comparison
pub fn normalize_text(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

/// Calculate semantic similarity between two prompts
pub fn semantic_similarity(prompt1: &str, prompt2: &str) -> f64 {
    let norm1 = normalize_text(prompt1);
    let norm2 = normalize_text(prompt2);

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

/// Load cached command for a prompt
pub fn load_cached(cache_path: &PathBuf, prompt: &str) -> Result<Option<String>> {
    if !cache_path.exists() {
        return Ok(None);
    }

    let data = std::fs::read_to_string(cache_path)?;
    let mut cache: CacheFile = serde_json::from_str(&data).unwrap_or_default();

    // Remove expired entries
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    cache
        .entries
        .retain(|entry| now - entry.timestamp < CACHE_TTL_SECONDS);

    // Save cleaned cache back to disk
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let serialized = serde_json::to_string_pretty(&cache)?;
    std::fs::write(cache_path, serialized)?;

    // First try exact match
    let mut entries_to_remove = Vec::new();
    let mut exact_match: Option<&CacheEntry> = None;

    for entry in &cache.entries {
        if entry.prompt == prompt {
            let cleaned_command = clean_command_output(&entry.command);
            // Validate cached command syntax before returning
            if validate_command_syntax(&cleaned_command).is_ok() {
                return Ok(Some(cleaned_command));
            } else {
                eprintln!(
                    "{}",
                    format!("Warning: Cached command has syntax issues, regenerating").yellow()
                );
                // Mark invalid entry for removal
                entries_to_remove.push(entry.prompt.clone());
                break;
            }
        }
    }

    // Then try semantic similarity
    let mut best_match: Option<&CacheEntry> = None;
    let mut best_similarity = 0.0;

    for entry in &cache.entries {
        let similarity = semantic_similarity(prompt, &entry.prompt);
        if similarity > best_similarity && similarity >= SEMANTIC_SIMILARITY_THRESHOLD {
            best_similarity = similarity;
            best_match = Some(entry);
        }
    }

    let result = if let Some(entry) = best_match {
        let cleaned_command = clean_command_output(&entry.command);
        // Validate semantically similar cached command syntax before returning
        if validate_command_syntax(&cleaned_command).is_ok() {
            Ok(Some(cleaned_command))
        } else {
            eprintln!(
                "{}",
                format!("Warning: Similar cached command has syntax issues, regenerating")
                    .yellow()
            );
            // Mark invalid entry for removal
            entries_to_remove.push(entry.prompt.clone());
            Ok(None)
        }
    } else {
        Ok(None)
    };

    // Remove invalid entries from cache after determining result
    if !entries_to_remove.is_empty() {
        cache
            .entries
            .retain(|e| !entries_to_remove.contains(&e.prompt));
        let serialized = serde_json::to_string_pretty(&cache)?;
        std::fs::write(cache_path, serialized)?;
    }

    result
}

/// Save command to cache
pub fn save_cached(cache_path: &PathBuf, prompt: &str, command: &str) -> Result<()> {
    let mut cache = if cache_path.exists() {
        let data = std::fs::read_to_string(cache_path).unwrap_or_default();
        serde_json::from_str::<CacheFile>(&data).unwrap_or_default()
    } else {
        CacheFile::default()
    };

    cache.entries.push(CacheEntry {
        prompt: prompt.to_string(),
        command: clean_command_output(command),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    });

    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let serialized = serde_json::to_string_pretty(&cache)?;
    std::fs::write(cache_path, serialized)?;

    Ok(())
}

/// Get explain cache path
pub fn explain_cache_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let mut path = PathBuf::from(home);
    path.push(".local");
    path.push("share");
    path.push("vibe_cli");
    let suffix = super::utils::project_cache_suffix();
    path.push(format!("{}_explain_cache.bin", suffix));
    path
}

/// Load cached explanation
pub fn load_cached_explain(cache_path: &PathBuf, prompt: &str) -> Result<Option<String>> {
    if !cache_path.exists() {
        return Ok(None);
    }

    let data = std::fs::read(cache_path)?;
    let mut cache: ExplainCacheFile = bincode::deserialize(&data).unwrap_or_default();

    // Remove expired entries (7 days)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    cache.entries.retain(|entry| now - entry.timestamp < 604800);

    // Save cleaned cache
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let serialized = bincode::serialize(&cache)?;
    std::fs::write(cache_path, serialized)?;

    // Find exact match
    for entry in &cache.entries {
        if entry.prompt == prompt {
            return Ok(Some(entry.response.clone()));
        }
    }
    Ok(None)
}

/// Save explanation to cache
pub fn save_cached_explain(cache_path: &PathBuf, prompt: &str, response: &str) -> Result<()> {
    let mut cache = if cache_path.exists() {
        let data = std::fs::read(cache_path).unwrap_or_default();
        bincode::deserialize::<ExplainCacheFile>(&data).unwrap_or_default()
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

    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let serialized = bincode::serialize(&cache)?;
    std::fs::write(cache_path, serialized)?;

    Ok(())
}

/// Get RAG cache path
pub fn rag_cache_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let mut path = PathBuf::from(home);
    path.push(".local");
    path.push("share");
    path.push("vibe_cli");
    let suffix = super::utils::project_cache_suffix();
    path.push(format!("{}_rag_cache.bin", suffix));
    path
}

/// Load cached RAG response
pub fn load_cached_rag(cache_path: &PathBuf, question: &str) -> Result<Option<String>> {
    if !cache_path.exists() {
        return Ok(None);
    }

    let data = std::fs::read(cache_path)?;
    let mut cache: RagCacheFile = bincode::deserialize(&data).unwrap_or_default();

    // Remove expired entries (7 days)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    cache.entries.retain(|entry| now - entry.timestamp < 604800);

    // Save cleaned cache
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let serialized = bincode::serialize(&cache)?;
    std::fs::write(cache_path, serialized)?;
    // Find exact match
    for entry in &cache.entries {
        if entry.question == question {
            return Ok(Some(entry.response.clone()));
        }
    }
    Ok(None)
}

/// Save RAG response to cache
pub fn save_cached_rag(cache_path: &PathBuf, question: &str, response: &str) -> Result<()> {
    let mut cache = if cache_path.exists() {
        let data = std::fs::read(cache_path).unwrap_or_default();
        bincode::deserialize::<RagCacheFile>(&data).unwrap_or_default()
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

    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let serialized = bincode::serialize(&cache)?;
    std::fs::write(cache_path, serialized)?;

    Ok(())
}