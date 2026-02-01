use super::ollama_client::OllamaClient;
use domain::value_objects::embedding::Embedding;
use futures::stream::{self, StreamExt};
use shared::error::AppError;

pub struct Embedder {
    client: OllamaClient,
}

#[derive(Clone)]
pub struct EmbeddingInput {
    pub id: String,
    pub path: String,
    pub text: String,
}

impl Embedder {
    pub fn new(client: OllamaClient) -> Self {
        Self { client }
    }

    pub async fn generate_embeddings(
        &self,
        inputs: &[EmbeddingInput],
    ) -> Result<Vec<Embedding>, AppError> {
        const BATCH_SIZE: usize = 32;
        let mut embeddings = Vec::with_capacity(inputs.len());

        for chunk in inputs.chunks(BATCH_SIZE) {
            eprintln!("Generating embeddings for {} chunks...", chunk.len());
            let batch_embeddings = self.generate_batch_embeddings(chunk).await?;
            embeddings.extend(batch_embeddings);
        }
        Ok(embeddings)
    }

    async fn generate_batch_embeddings(
        &self,
        inputs: &[EmbeddingInput],
    ) -> Result<Vec<Embedding>, AppError> {
        let futures: Vec<_> = inputs
            .iter()
            .map(|input| {
                let client = &self.client;
                async move {
                    let vector = client.generate_embedding(&input.text).await?;
                    Ok(Embedding::new(
                        input.id.clone(),
                        vector,
                        input.text.clone(),
                        input.path.clone(),
                    )) as Result<Embedding, AppError>
                }
            })
            .collect();

        let results: Vec<Result<Embedding, AppError>> = stream::iter(futures)
            .buffer_unordered(8)
            .collect::<Vec<_>>()
            .await;

        results.into_iter().collect()
    }
}
