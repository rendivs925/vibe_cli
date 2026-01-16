//! Cache data structures and utilities for CLI operations
//!
//! This module contains cache structures for different CLI operations:
//! - ExplainCache: Caches file explanations
//! - RagCache: Caches RAG query responses
//! - CommandCache: Caches command suggestions

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Cache file structure for explain operations
#[derive(Serialize, Deserialize, Default)]
pub struct ExplainCacheFile {
    pub entries: Vec<ExplainCacheEntry>,
}

/// Individual explain cache entry
#[derive(Serialize, Deserialize)]
pub struct ExplainCacheEntry {
    pub prompt: String,
    pub response: String,
    pub timestamp: u64,
}

/// Cache file structure for RAG operations
#[derive(Serialize, Deserialize, Default)]
pub struct RagCacheFile {
    pub entries: Vec<RagCacheEntry>,
}

/// Individual RAG cache entry
#[derive(Serialize, Deserialize)]
pub struct RagCacheEntry {
    pub question: String,
    pub response: String,
    pub timestamp: u64,
}

/// Cache file structure for command operations
#[derive(Serialize, Deserialize, Default)]
pub struct CommandCacheFile {
    pub entries: Vec<CommandCacheEntry>,
}

/// Individual command cache entry
#[derive(Serialize, Deserialize)]
pub struct CommandCacheEntry {
    pub query: String,
    pub command: String,
    pub timestamp: u64,
}

/// Load cached command from file
pub fn load_cached_command(cache_path: &PathBuf, query: &str) -> Result<Option<String>> {
    if !cache_path.exists() {
        return Ok(None);
    }

    let data = std::fs::read(cache_path)?;
    let mut cache: CommandCacheFile = bincode::deserialize(&data).unwrap_or_default();

    // Remove expired entries (7 days)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    cache.entries.retain(|entry| now - entry.timestamp < 604800);

    // Find exact match
    for entry in &cache.entries {
        if entry.query == query {
            return Ok(Some(entry.command.clone()));
        }
    }
    Ok(None)
}

/// Save command to cache file
pub fn save_cached_command(cache_path: &PathBuf, query: &str, command: &str) -> Result<()> {
    let mut cache = if cache_path.exists() {
        let data = std::fs::read(cache_path).unwrap_or_default();
        bincode::deserialize::<CommandCacheFile>(&data).unwrap_or_default()
    } else {
        CommandCacheFile::default()
    };

    cache.entries.push(CommandCacheEntry {
        query: query.to_string(),
        command: command.to_string(),
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
