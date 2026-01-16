pub mod advanced_scheduler;
pub mod agent_service;
pub mod build_service;
pub mod context_aware_validator;
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

/// Default agent service creation - uses Ollama (recommended)
pub async fn create_agent_service() -> shared::types::Result<agent_service::AgentService> {
    create_agent_service_with_ollama()
}



/// Convenience function to create an AgentService with Ollama (for backward compatibility)
pub fn create_agent_service_with_ollama() -> shared::types::Result<agent_service::AgentService> {
    use infrastructure::{ollama_client::OllamaClient, InferenceEngine};

    let ollama_client = OllamaClient::new()?;
    let inference_engine = InferenceEngine::Ollama(ollama_client);

    Ok(agent_service::AgentService::new(inference_engine))
}

/// Convenience function to create a RagService with Ollama inference
pub async fn create_rag_service(
    root_path: &str,
    db_path: &str,
) -> shared::types::Result<rag_service::RagService> {
    create_rag_service_with_qdrant(root_path, db_path, None).await
}

/// Create RAG service with optional Qdrant support
pub async fn create_rag_service_with_qdrant(
    root_path: &str,
    db_path: &str,
    qdrant_url: Option<String>,
) -> shared::types::Result<rag_service::RagService> {
    use infrastructure::{
        config::Config,
        ollama_client::OllamaClient,
        InferenceEngine,
    };

    // Create default config for RAG
    let config = Config::load();

    // Create Ollama inference service for RAG
    let ollama_client = OllamaClient::new()?;
    let inference_engine = InferenceEngine::Ollama(ollama_client);

    // Create RAG service with hybrid storage (Qdrant + SQLite fallback)
    let rag_service =
        rag_service::RagService::new(root_path, db_path, qdrant_url, inference_engine, config).await?;

    Ok(rag_service)
}
