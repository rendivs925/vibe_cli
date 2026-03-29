pub mod lifelong;
pub mod retrieval;

use std::path::PathBuf;

pub fn default_memory_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".config/vibe_cli/memory.db")
}
