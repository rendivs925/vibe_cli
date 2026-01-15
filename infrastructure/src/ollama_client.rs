use reqwest::{Client, ClientBuilder};
use serde::{Deserialize, Serialize};
use shared::types::Result;
use std::env;
use std::sync::Arc;
use std::time::Duration;
use futures::future::join_all;

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

        // Ultra-high performance HTTP client with HTTP/2 and connection pooling
        let client = ClientBuilder::new()
            .pool_max_idle_per_host(20) // Increased connection pool for HTTP/2 multiplexing
            .pool_idle_timeout(Duration::from_secs(60)) // Keep connections alive longer for HTTP/2
            .tcp_nodelay(true) // Disable Nagle's algorithm for low latency
            .timeout(Duration::from_secs(300)) // 5 minute timeout for long inferences
            .http2_prior_knowledge() // Enable HTTP/2 with prior knowledge
            .http2_max_frame_size(Some(32768)) // Larger frames for better throughput
            .http2_adaptive_window(true) // Adaptive window sizing
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

    /// Generate multiple embeddings concurrently with HTTP/2 pipelining
    pub async fn generate_embeddings_pipelined(
        &self,
        texts: Vec<String>,
    ) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // Create futures for all embedding requests
        let futures: Vec<_> = texts
            .into_iter()
            .map(|text| {
                let client = Arc::clone(&self.client);
                let base_url = self.base_url.clone();
                let model = self.model.clone();

                async move {
                    let url = format!("{}/api/embeddings", base_url);
                    let request = EmbeddingRequest {
                        model: model.clone(),
                        prompt: text,
                    };

                    let response = client.post(&url).json(&request).send().await?;
                    let embedding_response: EmbeddingResponse = response.json().await?;
                    Ok(embedding_response.embedding)
                }
            })
            .collect();

        // Execute all requests concurrently with HTTP/2 multiplexing
        let results: Vec<Result<Vec<f32>>> = join_all(futures).await;

        // Collect results, maintaining order
        let mut embeddings = Vec::with_capacity(results.len());
        for result in results {
            embeddings.push(result?);
        }

        Ok(embeddings)
    }

    /// Execute multiple inference requests in parallel with HTTP/2 pipelining
    pub async fn generate_responses_pipelined(
        &self,
        requests: Vec<InferenceRequest>,
    ) -> Result<Vec<String>> {
        if requests.is_empty() {
            return Ok(Vec::new());
        }

        // Create futures for all requests
        let futures: Vec<_> = requests
            .into_iter()
            .map(|req| async move {
                match req {
                    InferenceRequest::Embedding { text } => {
                        // For embeddings, generate and return as string representation
                        let embedding = self.generate_embedding(&text).await?;
                        Ok(serde_json::to_string(&embedding)?)
                    }
                    InferenceRequest::Chat { prompt, system } => {
                        self.generate_response_with_system(&prompt, &system).await
                    }
                }
            })
            .collect();

        // Execute all requests concurrently with HTTP/2 multiplexing
        let results = join_all(futures).await;

        // Collect results, maintaining order
        let mut responses = Vec::with_capacity(results.len());
        for result in results {
            responses.push(result?);
        }

        Ok(responses)
    }
}

/// Request types for pipelined inference
#[derive(Clone)]
pub enum InferenceRequest {
    Embedding { text: String },
    Chat { prompt: String, system: String },
}
