pub mod cmd;
pub mod explain;
pub mod rag;
pub mod storage;
pub mod types;
pub mod validator;

pub use cmd::CmdCache;
pub use explain::ExplainCache;
pub use rag::RagCache;
pub use storage::Storage;
pub use types::*;
pub use validator::Validator;

use shared::types::Result;
use std::path::PathBuf;

pub struct CacheManager {
    cache_dir: PathBuf,
}

impl CacheManager {
    pub fn new(cache_dir: PathBuf, _memory_mapped_io: bool) -> Self {
        Self { cache_dir }
    }

    pub fn cache_dir(&self) -> &PathBuf {
        &self.cache_dir
    }

    pub fn cache_path(&self, cache_type: &str) -> PathBuf {
        let mut path = self.cache_dir.clone();
        path.push(format!("{}.cache", cache_type));
        path
    }

    pub fn validate_command(&self, command: &str) -> bool {
        Validator::validate(command)
    }

    pub fn validate_command_syntax(&self, command: &str) -> bool {
        Validator::validate_syntax(command)
    }

    pub fn validate_command_exists(&self, command: &str) -> bool {
        Validator::validate_exists(command)
    }

    pub fn clean_command_output(raw: &str) -> String {
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

    pub fn load_command_cached(&self, prompt: &str) -> Result<Option<Vec<CommandCandidate>>> {
        let cache = CmdCache::new(self.cache_dir.clone());
        Ok(cache.get(prompt)?)
    }

    pub fn save_command_cached(
        &self,
        prompt: &str,
        candidates: Vec<CommandCandidate>,
    ) -> Result<()> {
        let cache = CmdCache::new(self.cache_dir.clone());
        Ok(cache.put(prompt, candidates)?)
    }

    pub fn load_explain_cached(&self, prompt: &str) -> Result<Option<String>> {
        let cache = ExplainCache::new(self.cache_dir.clone());
        Ok(cache.get(prompt)?)
    }

    pub fn save_explain_cached(&self, prompt: &str, response: &str) -> Result<()> {
        let cache = ExplainCache::new(self.cache_dir.clone());
        Ok(cache.put(prompt, response)?)
    }

    pub fn load_rag_cached(&self, question: &str) -> Result<Option<String>> {
        let cache = RagCache::new(self.cache_dir.clone());
        Ok(cache.get(question)?)
    }

    pub fn save_rag_cached(&self, question: &str, response: &str) -> Result<()> {
        let cache = RagCache::new(self.cache_dir.clone());
        Ok(cache.put(question, response)?)
    }

    pub fn load_command_cached_enhanced(
        &self,
        prompt: &str,
        _embedding: Option<&[f32]>,
    ) -> Result<Option<Vec<CommandCandidate>>> {
        self.load_command_cached(prompt)
    }

    pub fn get_analytics(&self) -> Result<CacheAnalytics> {
        let cmd_cache = CmdCache::new(self.cache_dir.clone());
        let explain_cache = ExplainCache::new(self.cache_dir.clone());
        let rag_cache = RagCache::new(self.cache_dir.clone());

        let cmd_entries = cmd_cache.count()?;
        let explain_entries = explain_cache.count()?;
        let rag_entries = rag_cache.count()?;

        Ok(CacheAnalytics {
            command_entries: cmd_entries,
            explain_entries,
            rag_entries,
            total_entries: cmd_entries + explain_entries + rag_entries,
            expired_entries: 0,
        })
    }
}
