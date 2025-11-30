use infrastructure::{
    embedder::Embedder, embedding_storage::EmbeddingStorage, file_scanner::FileScanner,
    ollama_client::OllamaClient, search::SearchEngine,
};
use shared::types::Result;
use std::path::PathBuf;

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
        self.build_index_with_files(&self.scanner.collect_files()?)
            .await
    }

    pub async fn build_index_for_keywords(&self, keywords: &[String]) -> Result<()> {
        // Filter files by keyword in path; fallback to full list if nothing matches.
        let mut files = self.scanner.collect_files()?;
        let keyword_lower: Vec<String> = keywords.iter().map(|k| k.to_lowercase()).collect();
        if !keyword_lower.is_empty() {
            let filtered: Vec<PathBuf> = files
                .iter()
                .filter(|p| {
                    let path_str = p.to_string_lossy().to_lowercase();
                    keyword_lower.iter().any(|k| path_str.contains(k))
                })
                .cloned()
                .collect();
            if !filtered.is_empty() {
                files = filtered;
            }
        }
        // Limit scanned files to reduce latency.
        const MAX_FILES: usize = 200;
        if files.len() > MAX_FILES {
            files.truncate(MAX_FILES);
        }

        self.build_index_with_files(&files).await
    }

    pub async fn query(&self, question: &str) -> Result<String> {
        let query_embedding = self.client.generate_embedding(question).await?;
        let all_embeddings = self.storage.get_all_embeddings()?;
        let relevant_chunks =
            SearchEngine::find_relevant_chunks(&query_embedding, &all_embeddings, 5);
        let context = relevant_chunks.join("\n\n");
        let prompt = format!("Context:\n{}\n\nQuestion: {}\nAnswer:", context, question);
        self.client.generate_response(&prompt).await
    }

    async fn build_index_with_files(&self, files: &[PathBuf]) -> Result<()> {
        // Add a small directory overview chunk to help the model understand layout.
        let mut texts = Vec::new();
        let dir_overview = self.scanner.directory_overview(4, 400);
        if !dir_overview.is_empty() {
            texts.push(format!("DIRECTORY TREE:\n{}", dir_overview));
        }

        let chunks = self.scanner.scan_paths(files)?;
        for chunk in chunks {
            texts.push(format!(
                "FILE: {}\nOFFSET: {}\n{}",
                chunk.path, chunk.start_offset, chunk.text
            ));
        }

        let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        let embeddings = self.embedder.generate_embeddings(&text_refs).await?;
        self.storage.insert_embeddings(&embeddings)?;
        Ok(())
    }
}
