pub mod advanced_scheduler;
pub mod advanced_qdrant;
pub mod agent_service;
pub mod build_service;
pub mod collection_partitioner;
pub mod context_aware_validator;
pub mod dynamic_scaling;
pub mod explain_service;
pub mod hallucination_detector;
pub mod health_monitor;
pub mod memory_cleanup;
pub mod memory_dashboard;
pub mod memory_summarizer;
pub mod metrics_collector;
pub mod parallel_agent;
pub mod rag_service;
pub mod result_aggregator;
pub mod safety_service;
pub mod semantic_memory;
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

/// Create agent service with semantic memory support
pub async fn create_agent_service_with_semantic_memory(
    qdrant_url: &str,
) -> shared::types::Result<agent_service::AgentService> {
    use infrastructure::{ollama_client::OllamaClient, InferenceEngine, embedder::Embedder};
    use std::sync::Arc;

    // Create Ollama inference engine
    let ollama_client = OllamaClient::new()?;
    let inference_engine = InferenceEngine::Ollama(ollama_client);

    // Create embedder for semantic memory
    let embedder = Arc::new(Embedder::new_with_inference_engine(inference_engine.clone()));

    // Create semantic memory service
    let semantic_memory = Arc::new(
        semantic_memory::SemanticMemoryService::new(qdrant_url, embedder).await?
    );

    // Create agent service with semantic memory
    Ok(agent_service::AgentService::new_with_semantic_memory(
        inference_engine,
        Some(semantic_memory),
    ))
}

/// Create health monitor for production monitoring
pub fn create_health_monitor(
    qdrant_url: &str,
    semantic_memory: Option<std::sync::Arc<semantic_memory::SemanticMemoryService>>,
) -> health_monitor::HealthMonitor {
    health_monitor::HealthMonitor::new(qdrant_url.to_string(), semantic_memory)
}

/// Create memory cleanup service with default policies
pub fn create_memory_cleanup_service(
    semantic_memory: std::sync::Arc<semantic_memory::SemanticMemoryService>,
) -> memory_cleanup::MemoryCleanupService {
    let policy = memory_cleanup::CleanupPolicy::default();
    memory_cleanup::MemoryCleanupService::new(semantic_memory, policy)
}

/// Create memory cleanup service with custom policies
pub fn create_memory_cleanup_service_with_policy(
    semantic_memory: std::sync::Arc<semantic_memory::SemanticMemoryService>,
    policy: memory_cleanup::CleanupPolicy,
) -> memory_cleanup::MemoryCleanupService {
    memory_cleanup::MemoryCleanupService::new(semantic_memory, policy)
}

/// Create memory summarizer for conversation compression
pub fn create_memory_summarizer(
    semantic_memory: std::sync::Arc<semantic_memory::SemanticMemoryService>,
    inference_engine: std::sync::Arc<infrastructure::InferenceEngine>,
) -> memory_summarizer::MemorySummarizer {
    memory_summarizer::MemorySummarizer::new(semantic_memory, inference_engine)
}

/// Create metrics collector for real-time monitoring
pub fn create_metrics_collector(
    semantic_memory: std::sync::Arc<semantic_memory::SemanticMemoryService>,
    health_monitor: std::sync::Arc<std::sync::Mutex<health_monitor::HealthMonitor>>,
) -> metrics_collector::MetricsCollector {
    metrics_collector::MetricsCollector::new(semantic_memory, health_monitor)
}

/// Create memory dashboard for visualization
pub fn create_memory_dashboard(
    metrics_collector: std::sync::Arc<std::sync::Mutex<metrics_collector::MetricsCollector>>,
    semantic_memory: std::sync::Arc<semantic_memory::SemanticMemoryService>,
) -> memory_dashboard::MemoryDashboard {
    memory_dashboard::MemoryDashboard::new(metrics_collector, semantic_memory)
}

/// Create collection partitioner for multi-language organization
pub fn create_collection_partitioner(
    qdrant_manager: std::sync::Arc<advanced_qdrant::AdvancedQdrantManager>,
    semantic_memory: std::sync::Arc<semantic_memory::SemanticMemoryService>,
) -> collection_partitioner::CollectionPartitioner {
    let config = collection_partitioner::PartitionConfig::default();
    collection_partitioner::CollectionPartitioner::new(qdrant_manager, semantic_memory, config)
}

/// Create collection partitioner with custom config
pub fn create_collection_partitioner_with_config(
    qdrant_manager: std::sync::Arc<advanced_qdrant::AdvancedQdrantManager>,
    semantic_memory: std::sync::Arc<semantic_memory::SemanticMemoryService>,
    config: collection_partitioner::PartitionConfig,
) -> collection_partitioner::CollectionPartitioner {
    collection_partitioner::CollectionPartitioner::new(qdrant_manager, semantic_memory, config)
}

/// Create advanced Qdrant manager for production optimization
pub fn create_advanced_qdrant_manager(qdrant_url: &str) -> advanced_qdrant::AdvancedQdrantManager {
    advanced_qdrant::AdvancedQdrantManager::new(qdrant_url)
}
