use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub context: HashMap<String, String>,
    pub history: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Session {
    pub fn new(id: String) -> Self {
        Self {
            id,
            context: HashMap::new(),
            history: Vec::new(),
        }
    }

    pub fn add_message(&mut self, role: &str, content: &str) {
        self.history.push(Message {
            role: role.to_string(),
            content: content.to_string(),
        });
    }
}