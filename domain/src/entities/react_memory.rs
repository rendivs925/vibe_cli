use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMemory {
    pub session_id: String,
    pub goal: String,
    pub created_at: DateTime<Utc>,

    pub constraints: Vec<Constraint>,
    pub facts: Vec<Fact>,
    pub hypotheses: Vec<Hypothesis>,
    pub key_insights: Vec<Insight>,

    pub embedding_id: Option<String>,
    pub semantic_tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    pub key: String,
    pub value: String,
    pub source: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fact {
    pub key: String,
    pub value: String,
    pub source_command: String,
    pub source_step: usize,
    pub verified: bool,
    pub embedding_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hypothesis {
    pub description: String,
    pub confidence: f32,
    pub supporting_facts: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Insight {
    pub text: String,
    pub importance: f32,
    pub created_at: DateTime<Utc>,
}

impl SessionMemory {
    pub fn new(session_id: String, goal: String) -> Self {
        Self {
            session_id,
            goal,
            created_at: Utc::now(),
            constraints: Vec::new(),
            facts: Vec::new(),
            hypotheses: Vec::new(),
            key_insights: Vec::new(),
            embedding_id: None,
            semantic_tags: Vec::new(),
        }
    }

    pub fn add_constraint(&mut self, constraint: Constraint) {
        let exists = self
            .constraints
            .iter()
            .any(|item| item.key == constraint.key && item.value == constraint.value);
        if !exists {
            self.constraints.push(constraint);
        }
    }

    pub fn add_fact(&mut self, fact: Fact) {
        let exists = self
            .facts
            .iter()
            .any(|item| item.key == fact.key && item.value == fact.value);
        if !exists {
            self.facts.push(fact);
        }
    }

    pub fn add_hypothesis(&mut self, hypothesis: Hypothesis) {
        self.hypotheses.push(hypothesis);
    }

    pub fn add_insight(&mut self, insight: Insight) {
        self.key_insights.push(insight);
    }

    pub fn reset_facts_and_hypotheses(&mut self) {
        self.facts.clear();
        self.hypotheses.clear();
        self.key_insights.clear();
    }

    pub fn add_tag(&mut self, tag: String) {
        if !self.semantic_tags.iter().any(|t| t == &tag) {
            self.semantic_tags.push(tag);
        }
    }
}

impl Default for SessionMemory {
    fn default() -> Self {
        Self {
            session_id: String::new(),
            goal: String::new(),
            created_at: Utc::now(),
            constraints: Vec::new(),
            facts: Vec::new(),
            hypotheses: Vec::new(),
            key_insights: Vec::new(),
            embedding_id: None,
            semantic_tags: Vec::new(),
        }
    }
}

impl Constraint {
    pub fn new(key: String, value: String, source: String) -> Self {
        Self {
            key,
            value,
            source,
            created_at: Utc::now(),
        }
    }
}

impl Fact {
    pub fn new(
        key: String,
        value: String,
        source_command: String,
        source_step: usize,
        verified: bool,
    ) -> Self {
        Self {
            key,
            value,
            source_command,
            source_step,
            verified,
            embedding_id: None,
        }
    }
}

impl Hypothesis {
    pub fn new(description: String, confidence: f32, supporting_facts: Vec<String>) -> Self {
        Self {
            description,
            confidence,
            supporting_facts,
            created_at: Utc::now(),
        }
    }
}

impl Insight {
    pub fn new(text: String, importance: f32) -> Self {
        Self {
            text,
            importance,
            created_at: Utc::now(),
        }
    }
}
