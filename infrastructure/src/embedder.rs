use super::{ollama_client::OllamaClient, InferenceEngine};
use domain::models::Embedding;
use futures::stream::{self, StreamExt};
use shared::types::Result;

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
        // In practice, this should be removed once all usage is migrated to Candle
        Self { inference_engine: InferenceEngine::Ollama(client) }
    }

    pub fn new_with_inference_engine(inference_engine: InferenceEngine) -> Self {
        Self { inference_engine }
    }

    pub async fn generate_embeddings(&self, inputs: &[EmbeddingInput]) -> Result<Vec<Embedding>> {
        const BATCH_SIZE: usize = 32;
        let mut embeddings = Vec::with_capacity(inputs.len());

        for chunk in inputs.chunks(BATCH_SIZE) {
            eprintln!("Generating embeddings for {} chunks...", chunk.len());
            let batch_embeddings = self.generate_batch_embeddings(chunk).await?;
            embeddings.extend(batch_embeddings);
        }
        Ok(embeddings)
    }

    async fn generate_batch_embeddings(&self, inputs: &[EmbeddingInput]) -> Result<Vec<Embedding>> {
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

        let results = stream::iter(futures)
            .buffer_unordered(8)
            .collect::<Vec<_>>()
            .await;

        results.into_iter().collect()
    }
}
