pub mod advanced_scheduler;
pub mod agent_service;
pub mod build_service;
pub mod dynamic_scaling;
pub mod explain_service;
pub mod hallucination_detector;
pub mod parallel_agent;
pub mod rag_service;
pub mod result_aggregator;
pub mod safety_service;
pub mod streaming_agent;
pub mod task_decomposer;
pub mod transaction;

/// Convenience function to create an AgentService with Candle inference (default)
pub async fn create_agent_service_with_candle() -> shared::types::Result<agent_service::AgentService> {
    use infrastructure::{candle_inference::{CandleInferenceService, ModelConfig, ModelArchitecture, QuantizationLevel}, InferenceEngine, config::Config};

    // Create Candle inference service
    let cache_dir = std::env::temp_dir().join("vibe_candle_cache");
    let config = ModelConfig {
        architecture: ModelArchitecture::Mistral,
        model_id: "mistralai/Mistral-7B-Instruct-v0.2".to_string(),
        quantization: QuantizationLevel::Q4,
        use_gpu: false,
        max_seq_len: 2048,
        temperature: 0.7,
        top_p: 0.9,
        repeat_penalty: 1.1,
    };

    let candle_service = CandleInferenceService::new(&cache_dir, config)?;
    let inference_engine = InferenceEngine::Candle(candle_service);

    // Create agent service with Candle backend
    let agent_service = agent_service::AgentService::new(inference_engine);

    Ok(agent_service)
}



/// Default agent service creation - uses Ollama (recommended)
pub async fn create_agent_service() -> shared::types::Result<agent_service::AgentService> {
    create_agent_service_with_ollama()
}

/// Convenience function to create a RagService with Candle inference (default)
pub async fn create_rag_service(root_path: &str, db_path: &str) -> shared::types::Result<rag_service::RagService> {
    use infrastructure::{candle_inference::{CandleInferenceService, ModelConfig, ModelArchitecture, QuantizationLevel}, InferenceEngine, config::Config};

    // Create default config for RAG
    let config = Config::load();

    // Create Candle inference service for RAG
    let cache_dir = std::env::temp_dir().join("vibe_candle_cache");
    let model_config = ModelConfig {
        architecture: ModelArchitecture::Mistral,
        model_id: "mistralai/Mistral-7B-Instruct-v0.2".to_string(),
        quantization: QuantizationLevel::Q4,
        use_gpu: false,
        max_seq_len: 2048,
        temperature: 0.7,
        top_p: 0.9,
        repeat_penalty: 1.1,
    };

    let candle_service = CandleInferenceService::new(&cache_dir, model_config)?;
    let inference_engine = InferenceEngine::Candle(candle_service);

    // Create RAG service with Candle backend
    let rag_service = rag_service::RagService::new(root_path, db_path, inference_engine, config).await?;

    Ok(rag_service)
}

/// Convenience function to create an AgentService with Ollama (for backward compatibility)
pub fn create_agent_service_with_ollama() -> shared::types::Result<agent_service::AgentService> {
    use infrastructure::{ollama_client::OllamaClient, InferenceEngine};

    let ollama_client = OllamaClient::new()?;
    let inference_engine = InferenceEngine::Ollama(ollama_client);

    Ok(agent_service::AgentService::new(inference_engine))
}
