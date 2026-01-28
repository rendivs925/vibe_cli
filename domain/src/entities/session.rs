use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Session entity representing a conversation context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    id: String,
    context: HashMap<String, String>,
    history: Vec<Message>,
}

impl Session {
    pub fn new(id: String) -> Self {
        Self {
            id,
            context: HashMap::new(),
            history: Vec::new(),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn context(&self) -> &HashMap<String, String> {
        &self.context
    }

    pub fn history(&self) -> &[Message] {
        &self.history
    }

    pub fn add_message(&mut self, message: Message) {
        self.history.push(message);
    }

    pub fn set_context(&mut self, key: String, value: String) {
        self.context.insert(key, value);
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    pub fn get_last_message(&self) -> Option<&Message> {
        self.history.last()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    role: MessageRole,
    content: String,
}

impl Message {
    pub fn new(role: MessageRole, content: String) -> Self {
        Self { role, content }
    }

    pub fn user(content: String) -> Self {
        Self::new(MessageRole::User, content)
    }

    pub fn assistant(content: String) -> Self {
        Self::new(MessageRole::Assistant, content)
    }

    pub fn role(&self) -> &MessageRole {
        &self.role
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn content_len(&self) -> usize {
        self.content.len()
    }

    pub fn is_empty(&self) -> bool {
        self.content.trim().is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

impl MessageRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
        }
    }
}
