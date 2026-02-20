use std::time::{SystemTime, UNIX_EPOCH};

use shared::types::Result;

use crate::cache::storage::Storage;
use crate::cache::types::{
    CmdEntries, CmdEntry, CommandCandidate, CACHE_TTL_SECONDS, SEMANTIC_SIMILARITY_THRESHOLD,
};
use crate::cache::validator::Validator;

pub struct CmdCache {
    storage: Storage,
}

impl CmdCache {
    pub fn new(cache_dir: std::path::PathBuf) -> Self {
        Self {
            storage: Storage::new(cache_dir),
        }
    }

    pub fn get(&self, prompt: &str) -> Result<Option<Vec<CommandCandidate>>> {
        let mut cache: CmdEntries = self.storage.load("cmd")?;

        let now = now_secs();
        cache
            .entries
            .retain(|e| now - e.timestamp < CACHE_TTL_SECONDS);
        self.storage.save("cmd", &cache)?;

        for entry in &cache.entries {
            if entry.prompt == prompt {
                if entry.candidates.is_empty() {
                    return Ok(None);
                }
                let valid: Vec<_> = entry
                    .candidates
                    .iter()
                    .filter(|c| Validator::validate(&c.command))
                    .cloned()
                    .collect();
                if valid.is_empty() {
                    self.remove(prompt)?;
                    return Ok(None);
                }
                return Ok(Some(valid));
            }
        }

        let mut best: Option<&CmdEntry> = None;
        let mut best_sim = 0.0;
        for entry in &cache.entries {
            let sim = Validator::semantic_similarity(prompt, &entry.prompt);
            if sim > best_sim && sim >= SEMANTIC_SIMILARITY_THRESHOLD {
                best_sim = sim;
                best = Some(entry);
            }
        }

        if let Some(entry) = best {
            if entry.candidates.is_empty() {
                return Ok(None);
            }
            let valid: Vec<_> = entry
                .candidates
                .iter()
                .filter(|c| Validator::validate(&c.command))
                .cloned()
                .collect();
            if valid.is_empty() {
                self.remove(&entry.prompt)?;
                return Ok(None);
            }
            Ok(Some(valid))
        } else {
            Ok(None)
        }
    }

    pub fn put(&self, prompt: &str, candidates: Vec<CommandCandidate>) -> Result<()> {
        let mut cache: CmdEntries = self.storage.load("cmd")?;
        cache.entries.push(CmdEntry {
            prompt: prompt.to_string(),
            candidates,
            timestamp: now_secs(),
        });
        self.storage.save("cmd", &cache)?;
        Ok(())
    }

    pub fn remove(&self, prompt: &str) -> Result<()> {
        let mut cache: CmdEntries = self.storage.load("cmd")?;
        cache.entries.retain(|e| e.prompt != prompt);
        self.storage.save("cmd", &cache)?;
        Ok(())
    }

    pub fn count(&self) -> Result<usize> {
        let cache: CmdEntries = self.storage.load("cmd")?;
        Ok(cache.entries.len())
    }

    pub fn clean_expired(&self) -> Result<usize> {
        let mut cache: CmdEntries = self.storage.load("cmd")?;
        let before = cache.entries.len();
        let now = now_secs();
        cache
            .entries
            .retain(|e| now - e.timestamp < CACHE_TTL_SECONDS);
        let removed = before - cache.entries.len();
        self.storage.save("cmd", &cache)?;
        Ok(removed)
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
