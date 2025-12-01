use infrastructure::{
    embedder::{Embedder, EmbeddingInput},
    embedding_storage::EmbeddingStorage,
    file_scanner::FileScanner,
    ollama_client::OllamaClient,
    search::SearchEngine,
};
use md5;
use shared::types::Result;
use std::path::PathBuf;

pub struct RagService {
    scanner: FileScanner,
    storage: EmbeddingStorage,
    embedder: Embedder,
    client: OllamaClient,
}

impl RagService {
    pub async fn new(root_path: &str, db_path: &str, client: OllamaClient) -> Result<Self> {
        Ok(Self {
            scanner: FileScanner::new(root_path),
            storage: EmbeddingStorage::new(db_path).await?,
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
        if !keywords.is_empty() {
            let keyword_lower: Vec<String> = keywords.iter().map(|k| k.to_lowercase()).collect();
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
        let all_embeddings = self.storage.get_all_embeddings().await?;
        let relevant_chunks =
            SearchEngine::find_relevant_chunks(&query_embedding, &all_embeddings, 5);
        let context = relevant_chunks.join("\n\n");
        let prompt = format!("Context:\n{}\n\nQuestion: {}\nAnswer:", context, question);
        self.client.generate_response(&prompt).await
    }

    async fn build_index_with_files(&self, files: &[PathBuf]) -> Result<()> {
        eprintln!("Scanning {} files...", files.len());
        let mut inputs: Vec<EmbeddingInput> = Vec::new();

        // Add a small directory overview chunk to help the model understand layout.
        let dir_overview = self.scanner.directory_overview(4, 400);
        if !dir_overview.is_empty() {
            let dir_hash = format!("{:x}", md5::compute(dir_overview.as_bytes()));
            let meta = self.storage.get_file_hash("__dir_overview__".to_string()).await?;
            if meta.as_deref() != Some(dir_hash.as_str()) {
                self.storage
                    .delete_embeddings_for_path("__dir_overview__".to_string()).await?;
                inputs.push(EmbeddingInput {
                    id: format!("__dir_overview__:{dir_hash}"),
                    path: "__dir_overview__".to_string(),
                    text: format!("DIRECTORY TREE:\n{}", dir_overview),
                });
                self.storage
                    .upsert_file_hash("__dir_overview__".to_string(), dir_hash).await?;
            }
        }

        let scans = self.scanner.scan_paths(files)?;
        for scan in scans {
            if scan.hash.is_empty() || scan.chunks.is_empty() {
                continue;
            }

            eprintln!("Processing {}...", scan.path);
            let previous_hash = self.storage.get_file_hash(scan.path.clone()).await?;
            if previous_hash.as_deref() == Some(scan.hash.as_str()) {
                continue;
            }

            // File changed; drop old embeddings for this path.
            self.storage.delete_embeddings_for_path(scan.path.clone()).await?;

            for chunk in scan.chunks {
                let id = format!("{}:{}", chunk.path, chunk.start_offset);
                let text = format!(
                    "FILE: {}\nOFFSET: {}\n{}",
                    chunk.path, chunk.start_offset, chunk.text
                );
                inputs.push(EmbeddingInput {
                    id,
                    path: chunk.path,
                    text,
                });
            }

            self.storage.upsert_file_hash(scan.path, scan.hash).await?;
        }

        if !inputs.is_empty() {
            eprintln!("Generating embeddings for {} chunks...", inputs.len());
            let embeddings = self.embedder.generate_embeddings(&inputs).await?;
            eprintln!("Storing embeddings...");
            self.storage.insert_embeddings(embeddings).await?;
            eprintln!("Indexing complete - {} chunks processed", inputs.len());
        }
        Ok(())
    }
}
