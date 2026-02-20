use std::time::{SystemTime, UNIX_EPOCH};

use shared::types::Result;

use crate::cache::storage::Storage;
use crate::cache::types::{RagEntries, RagEntry, CACHE_TTL_SECONDS};

pub struct RagCache {
    storage: Storage,
}

impl RagCache {
    pub fn new(cache_dir: std::path::PathBuf) -> Self {
        Self {
            storage: Storage::new(cache_dir),
        }
    }

    pub fn get(&self, question: &str) -> Result<Option<String>> {
        let mut cache: RagEntries = self.storage.load("rag")?;

        let now = now_secs();
        cache
            .entries
            .retain(|e| now - e.timestamp < CACHE_TTL_SECONDS);
        self.storage.save("rag", &cache)?;

        for entry in &cache.entries {
            if entry.question == question {
                return Ok(Some(entry.response.clone()));
            }
        }
        Ok(None)
    }

    pub fn put(&self, question: &str, response: &str) -> Result<()> {
        let mut cache: RagEntries = self.storage.load("rag")?;
        cache.entries.push(RagEntry {
            question: question.to_string(),
            response: response.to_string(),
            timestamp: now_secs(),
        });
        self.storage.save("rag", &cache)?;
        Ok(())
    }

    pub fn count(&self) -> Result<usize> {
        let cache: RagEntries = self.storage.load("rag")?;
        Ok(cache.entries.len())
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
