use super::ollama_client::OllamaClient;
use domain::models::Embedding;
use futures::stream::{self, StreamExt};
use shared::types::Result;

pub struct Embedder {
    client: OllamaClient,
}

impl Embedder {
    pub fn new(client: OllamaClient) -> Self {
        Self { client }
    }

    pub async fn generate_embeddings(&self, texts: &[&str]) -> Result<Vec<Embedding>> {
        const BATCH_SIZE: usize = 10;
        let mut embeddings = Vec::new();

        for chunk in texts.chunks(BATCH_SIZE) {
            let batch_embeddings = self.generate_batch_embeddings(chunk).await?;
            embeddings.extend(batch_embeddings);
        }
        Ok(embeddings)
    }

    async fn generate_batch_embeddings(&self, texts: &[&str]) -> Result<Vec<Embedding>> {
        let futures: Vec<_> = texts
            .iter()
            .map(|text| {
                let client = &self.client;
                async move {
                    let vector = client.generate_embedding(text).await?;
                    Ok(Embedding {
                        id: format!("{:x}", md5::compute(text)),
                        vector,
                        text: text.to_string(),
                    }) as Result<Embedding>
                }
            })
            .collect();

        let results = stream::iter(futures)
            .buffer_unordered(5)
            .collect::<Vec<_>>()
            .await;

        results.into_iter().collect()
    }
}
