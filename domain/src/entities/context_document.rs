use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContextDocumentType {
    SessionHistory,
    LatestOutput,
    ExtractedFacts,
    Hypotheses,
    Constraints,
    LearningContext,
    CodeContext,
    KnowledgeBase,
    ConversationHistory,
    Plan,
    Summary,
    Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextDocument {
    pub id: String,
    pub doc_type: ContextDocumentType,
    pub label: String,
    pub content: String,
    pub source_ref: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

impl ContextDocument {
    pub fn new(id: String, doc_type: ContextDocumentType, label: &str, content: String) -> Self {
        Self {
            id,
            doc_type,
            label: label.to_string(),
            content,
            source_ref: None,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_source(mut self, source: &str) -> Self {
        if !source.trim().is_empty() {
            self.source_ref = Some(source.to_string());
        }
        self
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    pub fn to_markdown(&self) -> String {
        let mut output = format!("<doc id=\"{}\" label=\"{}\">\n", self.id, self.label);
        output.push_str(&self.content);
        output.push_str("\n</doc>\n");
        output
    }
}
