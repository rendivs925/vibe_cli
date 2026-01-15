pub mod agent_control;
pub mod ast_parser;
pub mod background_supervisor;
pub mod candle_inference;
pub mod chatgpt_browser;
pub mod chatgpt_ocr;
pub mod command_interpreter;
pub mod compilation_watcher;
pub mod config;
pub mod embedder;
pub mod embedding_storage;
pub mod error_analyzer;
pub mod expert_resolver;
pub mod feature_flags;
pub mod file_scanner;
pub mod fix_applier;
pub mod hybrid_storage;
pub mod input_classifier;
pub mod log_tailer;
pub mod lsp_client;
pub mod network_security;
pub mod observability;
pub mod ollama_client;
pub mod policy_engine;
pub mod privacy_controls;
pub mod qdrant_advanced;
pub mod qdrant_storage;
pub mod resource_enforcement;
pub mod safety;
pub mod sandbox;
pub mod search;
pub mod session_store;
pub mod shell_monitor;
pub mod smart_router;
pub mod test_watcher;
pub mod tools;
pub mod web_search;

/// Common inference enum for different backends (Candle, Ollama, etc.)
#[derive(Clone)]
pub enum InferenceEngine {
    Ollama(ollama_client::OllamaClient),
    Candle(candle_inference::CandleInferenceService),
}

impl InferenceEngine {
    /// Generate text completion
    pub async fn generate(&self, prompt: &str) -> shared::types::Result<String> {
        match self {
            InferenceEngine::Ollama(client) => client.generate_response(prompt).await,
            InferenceEngine::Candle(service) => service.generate(prompt).await,
        }
    }

    /// Generate embeddings for text
    pub async fn generate_embeddings(&self, text: &str) -> shared::types::Result<Vec<f32>> {
        match self {
            InferenceEngine::Ollama(client) => client.generate_embedding(text).await,
            InferenceEngine::Candle(service) => service.generate_embeddings(text).await,
        }
    }

    /// Generate text completion with streaming for real-time feedback
    pub async fn generate_streaming<F>(
        &self,
        prompt: &str,
        on_chunk: F,
    ) -> shared::types::Result<String>
    where
        F: FnMut(&str) + Send,
    {
        match self {
            InferenceEngine::Ollama(client) => client.generate_response_streaming(prompt, on_chunk).await,
            InferenceEngine::Candle(service) => {
                // For Candle, we fall back to non-streaming for now
                // TODO: Implement streaming for Candle inference
                service.generate(prompt).await
            }
        }
    }

    /// Get model information
    pub async fn get_model_info(&self) -> ModelInfo {
        match self {
            InferenceEngine::Ollama(client) => ModelInfo {
                model_id: client.model().to_string(),
                architecture: "Unknown".to_string(),
                backend: "Ollama".to_string(),
                device: "Remote".to_string(),
            },
            InferenceEngine::Candle(service) => {
                let info = service.get_model_info().await;
                ModelInfo {
                    model_id: info.model_id,
                    architecture: format!("{:?}", info.architecture),
                    backend: "Candle".to_string(),
                    device: if service.config().use_gpu {
                        "GPU".to_string()
                    } else {
                        "CPU".to_string()
                    },
                }
            }
        }
    }
}

/// Model information
#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub model_id: String,
    pub architecture: String,
    pub backend: String,
    pub device: String,
}
