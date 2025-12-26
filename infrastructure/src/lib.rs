pub mod config;
pub mod embedder;
pub mod embedding_storage;
pub mod file_scanner;
pub mod ollama_client;
pub mod search;
pub mod qdrant_storage;
pub mod hybrid_storage;
pub mod ast_parser;
pub mod web_search;
pub mod expert_resolver;
pub mod safety;
pub mod qdrant_advanced;
pub mod input_classifier;
pub mod shell_monitor;
pub mod sandbox;
pub mod tools;
pub mod command_interpreter;
pub mod network_security;
pub mod resource_enforcement;
pub mod policy_engine;
pub mod agent_control;
pub mod observability;
pub mod feature_flags;
pub mod candle_inference;
pub mod session_store;
pub mod background_supervisor;
pub mod lsp_client;
pub mod test_watcher;
pub mod log_tailer;
pub mod error_analyzer;
pub mod chatgpt_browser;
pub mod smart_router;
pub mod compilation_watcher;
pub mod fix_applier;

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
                    device: if service.config().use_gpu { "GPU".to_string() } else { "CPU".to_string() },
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
