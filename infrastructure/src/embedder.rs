use super::{ollama_client::OllamaClient, InferenceEngine};
use domain::models::Embedding;
use futures::stream::{self, StreamExt};
use shared::types::Result;
use shared::performance_monitor::GLOBAL_METRICS;
use std::time::Instant;

pub struct Embedder {
    inference_engine: InferenceEngine,
}

#[derive(Clone)]
pub struct EmbeddingInput {
    pub id: String,
    pub path: String,
    pub text: String,
}

impl Embedder {
    pub fn new(client: OllamaClient) -> Self {
        // For backward compatibility, wrap OllamaClient in InferenceEngine
        Self {
            inference_engine: InferenceEngine::Ollama(client),
        }
    }

    pub fn new_with_inference_engine(inference_engine: InferenceEngine) -> Self {
        Self { inference_engine }
    }

    pub async fn generate_embeddings(&self, inputs: &[EmbeddingInput]) -> Result<Vec<Embedding>> {
        let mut embeddings = Vec::with_capacity(inputs.len());

        let mut start_idx = 0;
        while start_idx < inputs.len() {
            let batch_size = self.calculate_dynamic_batch_size(inputs.len() - start_idx).await;
            let end_idx = (start_idx + batch_size).min(inputs.len());
            let chunk = &inputs[start_idx..end_idx];

            eprintln!("Generating embeddings for {} chunks (batch size: {})...", chunk.len(), batch_size);

            let batch_start = Instant::now();
            let batch_embeddings = self.generate_batch_embeddings(chunk).await?;
            let batch_duration = batch_start.elapsed();

            // Record performance metrics for dynamic adjustment
            GLOBAL_METRICS.end_operation("embedding_batch").await;

            embeddings.extend(batch_embeddings);
            start_idx = end_idx;
        }
        Ok(embeddings)
    }

    async fn generate_batch_embeddings(&self, inputs: &[EmbeddingInput]) -> Result<Vec<Embedding>> {
        // Use pipelined inference for small batches (better HTTP/2 utilization)
        if inputs.len() <= 32 {
            return self.generate_batch_embeddings_pipelined(inputs).await;
        }

        // Ultra-high performance: use all CPU cores with work-stealing for larger batches
        let num_concurrent = std::cmp::min(inputs.len(), num_cpus::get() * 4);

        let futures: Vec<_> = inputs
            .iter()
            .map(|input| {
                let inference_engine = &self.inference_engine;
                async move {
                    let vector = inference_engine.generate_embeddings(&input.text).await?;
                    Ok(Embedding {
                        id: input.id.clone(),
                        vector,
                        text: input.text.clone(),
                        path: input.path.clone(),
                    }) as Result<Embedding>
                }
            })
            .collect();

        // Maximize concurrency for ultra-fast processing
        let results = stream::iter(futures)
            .buffer_unordered(num_concurrent)
            .collect::<Vec<_>>()
            .await;

        results.into_iter().collect()
    }

    /// Generate batch embeddings using HTTP/2 pipelining for optimal concurrency
    async fn generate_batch_embeddings_pipelined(&self, inputs: &[EmbeddingInput]) -> Result<Vec<Embedding>> {
        let texts: Vec<String> = inputs.iter().map(|input| input.text.clone()).collect();

        // Use OllamaClient's pipelined method if available
        let embeddings = match &self.inference_engine {
            InferenceEngine::Ollama(client) => {
                client.generate_embeddings_pipelined(texts).await?
            }
            // Fallback to individual requests for other inference engines
            _ => {
                let mut embeddings = Vec::with_capacity(inputs.len());
                for input in inputs {
                    let vector = self.inference_engine.generate_embeddings(&input.text).await?;
                    embeddings.push(vector);
                }
                embeddings
            }
        };

        // Convert to Embedding structs
        let results: Vec<Embedding> = inputs
            .iter()
            .zip(embeddings.into_iter())
            .map(|(input, vector)| Embedding {
                id: input.id.clone(),
                vector,
                text: input.text.clone(),
                path: input.path.clone(),
            })
            .collect();

        Ok(results)
    }

    /// Calculate optimal batch size based on system load and performance metrics
    async fn calculate_dynamic_batch_size(&self, remaining_items: usize) -> usize {
        // Start with performance monitoring
        GLOBAL_METRICS.start_operation("embedding_batch").await;

        // Base parameters
        let min_batch_size = 16;
        let max_batch_size = 512;
        let default_batch_size = 128;

        // Get system information
        let num_cpus = num_cpus::get();
        let available_parallelism = num_cpus as f32;

        // Get recent performance metrics to inform batch sizing
        let recent_latency = GLOBAL_METRICS.average_latency("embedding_batch").await;
        let recent_throughput = GLOBAL_METRICS.throughput("embedding_batch").await;

        // Estimate current system load (simplified heuristic)
        // In a production system, this would use actual CPU/memory monitoring
        let estimated_load_factor = self.estimate_system_load().await;

        // Calculate adaptive batch size
        let mut optimal_batch_size = default_batch_size;

        // Adjust based on CPU availability
        if available_parallelism >= 8.0 {
            // High-core systems can handle larger batches
            optimal_batch_size = (optimal_batch_size as f32 * 1.5) as usize;
        } else if available_parallelism <= 2.0 {
            // Low-core systems need smaller batches
            optimal_batch_size = (optimal_batch_size as f32 * 0.7) as usize;
        }

        // Adjust based on system load
        if estimated_load_factor > 0.8 {
            // High load - reduce batch size to be more responsive
            optimal_batch_size = (optimal_batch_size as f32 * 0.6) as usize;
        } else if estimated_load_factor < 0.3 {
            // Low load - increase batch size for better throughput
            optimal_batch_size = (optimal_batch_size as f32 * 1.3) as usize;
        }

        // Adjust based on recent performance
        if let Some(latency) = recent_latency {
            if latency.as_millis() > 2000 {
                // Slow performance - reduce batch size
                optimal_batch_size = (optimal_batch_size as f32 * 0.8) as usize;
            } else if latency.as_millis() < 500 {
                // Fast performance - can handle larger batches
                optimal_batch_size = (optimal_batch_size as f32 * 1.2) as usize;
            }
        }

        // Adjust based on throughput
        if let Some(throughput) = recent_throughput {
            if throughput < 2.0 {
                // Low throughput - reduce batch size
                optimal_batch_size = (optimal_batch_size as f32 * 0.9) as usize;
            } else if throughput > 10.0 {
                // High throughput - increase batch size
                optimal_batch_size = (optimal_batch_size as f32 * 1.1) as usize;
            }
        }

        // Ensure batch size is within bounds
        optimal_batch_size = optimal_batch_size.clamp(min_batch_size, max_batch_size);

        // Don't exceed remaining items
        optimal_batch_size.min(remaining_items)
    }

    /// Estimate current system load (simplified heuristic)
    async fn estimate_system_load(&self) -> f32 {
        // This is a simplified load estimation
        // In production, would use actual system monitoring (CPU, memory, I/O)

        let system_stats = GLOBAL_METRICS.system_stats().await;
        let active_operations = system_stats.active_operations as f32;
        let total_operations = system_stats.total_operations as f32;

        // Simple load factor based on active operations
        if total_operations > 0.0 {
            (active_operations / total_operations.min(100.0)).clamp(0.0, 1.0)
        } else {
            0.1 // Base load assumption
        }
    }
}
