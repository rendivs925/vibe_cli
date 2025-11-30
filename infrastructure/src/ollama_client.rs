use reqwest::Client;
use serde::{Deserialize, Serialize};
use shared::types::Result;
use std::env;
use std::sync::Arc;

#[derive(Serialize)]
struct EmbeddingRequest {
    model: String,
    prompt: String,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    embedding: Vec<f32>,
}

#[derive(Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
}

#[derive(Deserialize)]
struct ChatResponse {
    message: Message,
    done: bool,
}

#[derive(Clone)]
pub struct OllamaClient {
    client: Arc<Client>,
    base_url: String,
    model: String,
}

impl OllamaClient {
    pub fn new() -> Result<Self> {
        let base_url = env::var("OLLAMA_BASE_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());
        let model = env::var("OLLAMA_MODEL").unwrap_or_else(|_| "qwen2.5-coder:7b".to_string());
        Ok(Self {
            client: Arc::new(Client::new()),
            base_url,
            model,
        })
    }

    pub async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let url = format!("{}/api/embeddings", self.base_url);
        let request = EmbeddingRequest {
            model: self.model.clone(),
            prompt: text.to_string(),
        };
        let response = self.client.post(&url).json(&request).send().await?;
        let embedding_response: EmbeddingResponse = response.json().await?;
        Ok(embedding_response.embedding)
    }

    pub async fn generate_response(&self, prompt: &str) -> Result<String> {
        let url = format!("{}/api/chat", self.base_url);
        let request = ChatRequest {
            model: self.model.clone(),
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            stream: false,
        };
        let response = self.client.post(&url).json(&request).send().await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(anyhow::anyhow!("Ollama API error: {}", text));
        }
        let mut full_content = String::new();
        for line in text.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(chat_resp) = serde_json::from_str::<ChatResponse>(line) {
                full_content.push_str(&chat_resp.message.content);
                if chat_resp.done {
                    break;
                }
            }
        }
        Ok(full_content)
    }
}