use crate::memory::lifelong::LifelongMemoryStore;
use domain::entities::react::ReactSession;
use std::error::Error;

pub struct MemoryConsolidator {
    store: LifelongMemoryStore,
}

impl MemoryConsolidator {
    pub fn new(store: LifelongMemoryStore) -> Self {
        Self { store }
    }

    pub fn consolidate_session(&self, session: &ReactSession) -> Result<i64, Box<dyn Error>> {
        let mut summary = String::new();
        summary.push_str(&format!("Goal: {}\n", session.query));
        if let Some(compacted) = &session.compacted_summary {
            if !compacted.trim().is_empty() {
                summary.push_str("Summary: ");
                summary.push_str(compacted.trim());
                summary.push('\n');
            }
        }
        if !session.memory.facts.is_empty() {
            summary.push_str("Facts:\n");
            for fact in &session.memory.facts {
                summary.push_str(&format!("- {}={}\n", fact.key, fact.value));
            }
        }
        if !session.memory.hypotheses.is_empty() {
            summary.push_str("Hypotheses:\n");
            for h in &session.memory.hypotheses {
                summary.push_str(&format!("- {} (confidence {:.2})\n", h.description, h.confidence));
            }
        }
        self.store.remember(summary.trim())
    }
}
