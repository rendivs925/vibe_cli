//! Cache data structures and utilities for CLI operations
//!
//! This module contains cache structures for different CLI operations:
//! - ExplainCache: Caches file explanations
//! - RagCache: Caches RAG query responses
//! - CommandCache: Caches command suggestions

use serde::{Deserialize, Serialize};

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
