use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::anyhow;
use shared::types::Result;
use crate::services::research_summary::summarize_sources;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchSource {
    pub url: String,
    pub title: String,
    pub content: String,
    pub collected_at: u64,
    pub relevance_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchNote {
    pub id: String,
    pub content: String,
    pub source_url: Option<String>,
    pub tags: Vec<String>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchQuery {
    pub id: String,
    pub query: String,
    pub depth: ResearchDepth,
    pub sources_limit: usize,
    pub collected_sources: Vec<String>,
    pub status: ResearchStatus,
    pub created_at: u64,
    pub completed_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ResearchDepth {
    Quick,
    Standard,
    Deep,
    Comprehensive,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ResearchStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

impl ResearchQuery {
    pub fn new(query: String, depth: ResearchDepth) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let sources_limit = match depth {
            ResearchDepth::Quick => 3,
            ResearchDepth::Standard => 5,
            ResearchDepth::Deep => 10,
            ResearchDepth::Comprehensive => 20,
        };

        Self {
            id: format!("research_{}", now),
            query,
            depth,
            sources_limit,
            collected_sources: Vec::new(),
            status: ResearchStatus::Pending,
            created_at: now,
            completed_at: None,
        }
    }
}

pub struct ResearchAgent {
    storage_path: PathBuf,
    queries: HashMap<String, ResearchQuery>,
    sources: HashMap<String, ResearchSource>,
    notes: HashMap<String, ResearchNote>,
}

impl ResearchAgent {
    pub fn new() -> Self {
        let config_dir = infrastructure::storage::get_config_dir();
        let storage_path = config_dir.join("research");

        let (queries, sources, notes) = Self::load_research_data(&storage_path);

        Self {
            storage_path,
            queries,
            sources,
            notes,
        }
    }

    fn load_research_data(
        storage_path: &PathBuf,
    ) -> (
        HashMap<String, ResearchQuery>,
        HashMap<String, ResearchSource>,
        HashMap<String, ResearchNote>,
    ) {
        let queries_path = storage_path.join("queries.json");
        let sources_path = storage_path.join("sources.json");
        let notes_path = storage_path.join("notes.json");

        let queries = if queries_path.exists() {
            fs::read_to_string(&queries_path)
                .ok()
                .and_then(|c| serde_json::from_str(&c).ok())
                .unwrap_or_default()
        } else {
            HashMap::new()
        };

        let sources = if sources_path.exists() {
            fs::read_to_string(&sources_path)
                .ok()
                .and_then(|c| serde_json::from_str(&c).ok())
                .unwrap_or_default()
        } else {
            HashMap::new()
        };

        let notes = if notes_path.exists() {
            fs::read_to_string(&notes_path)
                .ok()
                .and_then(|c| serde_json::from_str(&c).ok())
                .unwrap_or_default()
        } else {
            HashMap::new()
        };

        (queries, sources, notes)
    }

    fn save_all(&self) {
        let _ = fs::create_dir_all(&self.storage_path);

        if let Ok(content) = serde_json::to_string_pretty(&self.queries) {
            let _ = fs::write(self.storage_path.join("queries.json"), content);
        }

        if let Ok(content) = serde_json::to_string_pretty(&self.sources) {
            let _ = fs::write(self.storage_path.join("sources.json"), content);
        }

        if let Ok(content) = serde_json::to_string_pretty(&self.notes) {
            let _ = fs::write(self.storage_path.join("notes.json"), content);
        }
    }

    pub fn start_research(&mut self, query: String, depth: ResearchDepth) -> String {
        let research = ResearchQuery::new(query, depth);
        let id = research.id.clone();

        self.queries.insert(id.clone(), research);
        self.save_all();

        id
    }

    pub fn get_query(&self, query_id: &str) -> Option<&ResearchQuery> {
        self.queries.get(query_id)
    }

    pub fn add_source(
        &mut self,
        query_id: &str,
        url: String,
        title: String,
        content: String,
    ) -> Result<()> {
        let query = self
            .queries
            .get_mut(query_id)
            .ok_or_else(|| anyhow!("Query not found: {}", query_id))?;

        if !query.collected_sources.contains(&url) {
            query.collected_sources.push(url.clone());

            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            let source = ResearchSource {
                url: url.clone(),
                title,
                content,
                collected_at: now,
                relevance_score: 0.5,
            };

            self.sources.insert(url, source);
        }

        if query.collected_sources.len() >= query.sources_limit {
            query.status = ResearchStatus::Completed;
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            query.completed_at = Some(now);
        }

        self.save_all();

        Ok(())
    }

    pub fn add_note(
        &mut self,
        content: String,
        source_url: Option<String>,
        tags: Vec<String>,
    ) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let note = ResearchNote {
            id: format!("note_{}", now),
            content,
            source_url,
            tags,
            created_at: now,
            updated_at: now,
        };

        let id = note.id.clone();
        self.notes.insert(id.clone(), note);
        self.save_all();

        id
    }

    pub fn get_note(&self, note_id: &str) -> Option<&ResearchNote> {
        self.notes.get(note_id)
    }

    pub fn search_notes(&self, query: &str) -> Vec<&ResearchNote> {
        let query_lower = query.to_lowercase();

        self.notes
            .values()
            .filter(|n| {
                n.content.to_lowercase().contains(&query_lower)
                    || n.tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&query_lower))
            })
            .collect()
    }

    pub fn get_sources_for_query(&self, query_id: &str) -> Vec<&ResearchSource> {
        let query = match self.queries.get(query_id) {
            Some(q) => q,
            None => return Vec::new(),
        };

        query
            .collected_sources
            .iter()
            .filter_map(|url| self.sources.get(url))
            .collect()
    }

    pub fn synthesize_findings(&self, query_id: &str) -> Result<String> {
        let query = self
            .queries
            .get(query_id)
            .ok_or_else(|| anyhow!("Query not found: {}", query_id))?;

        let sources = self.get_sources_for_query(query_id);

        let mut synthesis = String::new();
        synthesis.push_str(&format!("# Research Findings: {}\n\n", query.query));
        synthesis.push_str(&format!("**Depth:** {:?}\n", query.depth));
        synthesis.push_str(&format!("**Sources:** {}\n\n", sources.len()));

        synthesis.push_str("## Sources\n\n");

        for (i, source) in sources.iter().enumerate() {
            synthesis.push_str(&format!("{}. {}\n", i + 1, source.title));
            synthesis.push_str(&format!("   URL: {}\n\n", source.url));
        }

        synthesis.push_str("## Summary\n\n");
        synthesis.push_str("Based on the collected sources, here are the key findings:\n\n");

        let summary_lines = summarize_sources(&sources, &query.depth);
        if summary_lines.is_empty() {
            synthesis.push_str("- No summarized findings available yet.\n");
        } else {
            for line in summary_lines {
                synthesis.push_str(&format!("- {}\n", line));
            }
        }

        Ok(synthesis)
    }

    pub fn list_queries(&self) -> Vec<&ResearchQuery> {
        self.queries.values().collect()
    }

    pub fn cancel_query(&mut self, query_id: &str) -> Result<()> {
        let query = self
            .queries
            .get_mut(query_id)
            .ok_or_else(|| anyhow!("Query not found: {}", query_id))?;

        query.status = ResearchStatus::Cancelled;
        self.save_all();

        Ok(())
    }

    pub fn delete_query(&mut self, query_id: &str) -> Result<()> {
        if let Some(query) = self.queries.remove(query_id) {
            for url in &query.collected_sources {
                self.sources.remove(url);
            }
        }

        self.save_all();

        Ok(())
    }
}
