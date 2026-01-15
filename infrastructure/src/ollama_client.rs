use reqwest::{Client, ClientBuilder};
use serde::{Deserialize, Serialize};
use shared::types::Result;
use std::env;
use std::sync::Arc;
use std::time::Duration;

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
        let base_url =
            env::var("OLLAMA_BASE_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());
        let model = env::var("BASE_MODEL").unwrap_or_else(|_| "qwen2.5:1.5b-instruct".to_string());

        // Ultra-high performance HTTP client with connection pooling
        let client = ClientBuilder::new()
            .pool_max_idle_per_host(10) // Connection pool for concurrent requests
            .pool_idle_timeout(Duration::from_secs(30)) // Keep connections alive
            .tcp_nodelay(true) // Disable Nagle's algorithm for low latency
            .timeout(Duration::from_secs(300)) // 5 minute timeout for long inferences
            .build()?;

        Ok(Self {
            client: Arc::new(client),
            base_url,
            model,
        })
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// Pre-warm the model by sending a minimal request to ensure it's loaded
    pub async fn prewarm_model(&self) -> Result<()> {
        // Send a minimal request to load the model into memory
        let _ = self.generate_response_with_system("ping", "").await?;
        Ok(())
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
        self.generate_response_with_system(prompt, "").await
    }

    /// Generate response with streaming for real-time feedback
    pub async fn generate_response_streaming<F>(
        &self,
        prompt: &str,
        mut on_chunk: F,
    ) -> Result<String>
    where
        F: FnMut(&str) + Send,
    {
        self.generate_response_with_system_streaming(prompt, "", on_chunk).await
    }

    pub async fn generate_response_with_system(
        &self,
        prompt: &str,
        system: &str,
    ) -> Result<String> {
        let url = format!("{}/api/chat", self.base_url);
        let mut messages = Vec::new();
        if !system.is_empty() {
            messages.push(Message {
                role: "system".to_string(),
                content: system.to_string(),
            });
        }
        messages.push(Message {
            role: "user".to_string(),
            content: prompt.to_string(),
        });
        let request = ChatRequest {
            model: self.model.clone(),
            messages,
            stream: false,
        };
        let response = self.client.post(&url).json(&request).send().await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(anyhow::anyhow!("Ollama API error: {}", text));
        }

        // Ultra-fast response parsing with minimal allocations
        let mut full_content = String::with_capacity(4096); // Pre-allocate for performance
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

    /// Generate response with system message and streaming support
    pub async fn generate_response_with_system_streaming<F>(
        &self,
        prompt: &str,
        system: &str,
        mut on_chunk: F,
    ) -> Result<String>
    where
        F: FnMut(&str) + Send,
    {
        let url = format!("{}/api/chat", self.base_url);
        let mut messages = Vec::new();
        if !system.is_empty() {
            messages.push(Message {
                role: "system".to_string(),
                content: system.to_string(),
            });
        }
        messages.push(Message {
            role: "user".to_string(),
            content: prompt.to_string(),
        });

        // Enable streaming for real-time feedback
        let request = ChatRequest {
            model: self.model.clone(),
            messages,
            stream: true, // Enable streaming
        };

        let response = self.client.post(&url).json(&request).send().await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(anyhow::anyhow!("Ollama API error: {}", text));
        }

        let mut full_content = String::with_capacity(4096); // Pre-allocate for performance
        for line in text.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(chat_resp) = serde_json::from_str::<ChatResponse>(line) {
                let chunk = &chat_resp.message.content;
                if !chunk.is_empty() {
                    // Call the callback with each chunk for real-time display
                    on_chunk(chunk);
                    full_content.push_str(chunk);
                }
                if chat_resp.done {
                    break;
                }
            }
        }
        Ok(full_content)
    }
}
