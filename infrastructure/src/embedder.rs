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
        self.generate_embeddings_with_progress(inputs, |_, _| {})
            .await
    }

    pub async fn generate_embeddings_with_progress<F>(
        &self,
        inputs: &[EmbeddingInput],
        mut on_progress: F,
    ) -> Result<Vec<Embedding>, AppError>
    where
        F: FnMut(usize, usize),
    {
        let total = inputs.len();
        if total == 0 {
            return Ok(Vec::new());
        }

        let concurrency = std::env::var("RAG_EMBED_CONCURRENCY")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(24);

        let mut stream = stream::iter(inputs.iter().cloned().map(|input| {
            let client = self.client.clone();
            async move {
                let vector = client.generate_embedding(&input.text).await?;
                Ok(Embedding::new(input.id, vector, input.text, input.path))
                    as Result<Embedding, AppError>
            }
        }))
        .buffer_unordered(concurrency);

        let mut done = 0usize;
        let mut embeddings = Vec::with_capacity(total);
        while let Some(result) = stream.next().await {
            let embedding = result?;
            done += 1;
            on_progress(done, total);
            embeddings.push(embedding);
        }
        Ok(embeddings)
    }
}
