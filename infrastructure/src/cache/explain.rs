use std::time::{SystemTime, UNIX_EPOCH};

use shared::types::Result;

use crate::cache::storage::Storage;
use crate::cache::types::{ExplainEntries, ExplainEntry, CACHE_TTL_SECONDS};

pub struct ExplainCache {
    storage: Storage,
}

impl ExplainCache {
    pub fn new(cache_dir: std::path::PathBuf) -> Self {
        Self {
            storage: Storage::new(cache_dir),
        }
    }

    pub fn get(&self, prompt: &str) -> Result<Option<String>> {
        let mut cache: ExplainEntries = self.storage.load("explain")?;

        let now = now_secs();
        cache
            .entries
            .retain(|e| now - e.timestamp < CACHE_TTL_SECONDS);
        self.storage.save("explain", &cache)?;

        for entry in &cache.entries {
            if entry.prompt == prompt {
                return Ok(Some(entry.response.clone()));
            }
        }
        Ok(None)
    }

    pub fn put(&self, prompt: &str, response: &str) -> Result<()> {
        let mut cache: ExplainEntries = self.storage.load("explain")?;
        cache.entries.push(ExplainEntry {
            prompt: prompt.to_string(),
            response: response.to_string(),
            timestamp: now_secs(),
        });
        self.storage.save("explain", &cache)?;
        Ok(())
    }

    pub fn count(&self) -> Result<usize> {
        let cache: ExplainEntries = self.storage.load("explain")?;
        Ok(cache.entries.len())
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
