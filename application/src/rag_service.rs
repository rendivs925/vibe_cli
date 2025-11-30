use infrastructure::{
    embedder::Embedder, embedding_storage::EmbeddingStorage, file_scanner::FileScanner,
    ollama_client::OllamaClient, search::SearchEngine,
};
use shared::types::Result;

pub struct RagService {
    scanner: FileScanner,
    storage: EmbeddingStorage,
    embedder: Embedder,
    client: OllamaClient,
}

impl RagService {
    pub fn new(root_path: &str, db_path: &str, client: OllamaClient) -> Result<Self> {
        Ok(Self {
            scanner: FileScanner::new(root_path),
            storage: EmbeddingStorage::new(db_path)?,
            embedder: Embedder::new(client.clone()),
            client: client,
        })
    }

    pub async fn build_index(&self) -> Result<()> {
        let chunks = self.scanner.scan_files()?;
        let texts: Vec<&str> = chunks.iter().map(|c| c.text.as_str()).collect();
        let embeddings = self.embedder.generate_embeddings(&texts).await?;
        self.storage.insert_embeddings(&embeddings)?;
        Ok(())
    }

    pub async fn query(&self, question: &str) -> Result<String> {
        let query_embedding = self.client.generate_embedding(question).await?;
        let all_embeddings = self.storage.get_all_embeddings()?;
        let relevant_chunks =
            SearchEngine::find_relevant_chunks(&query_embedding, &all_embeddings, 5);
        let context = relevant_chunks.join("\n");
        let prompt = format!("Context:\n{}\n\nQuestion: {}\nAnswer:", context, question);
        self.client.generate_response(&prompt).await
    }
}
