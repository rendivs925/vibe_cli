use crate::memory::lifelong::{LifelongEntry, LifelongMemoryStore, PatternEntry};
use std::error::Error;

pub struct MemoryRetriever {
    store: LifelongMemoryStore,
}

impl MemoryRetriever {
    pub fn new(store: LifelongMemoryStore) -> Self {
        Self { store }
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<LifelongEntry>, Box<dyn Error>> {
        self.store.search(query, limit)
    }

    pub fn search_patterns(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<PatternEntry>, Box<dyn Error>> {
        self.store.search_patterns(query, limit)
    }
}
