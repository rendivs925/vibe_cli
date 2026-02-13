use infrastructure::{
    code_ast,
    config::Config,
    embedder::{Embedder, EmbeddingInput},
    embedding_storage::EmbeddingStorage,
    file_scanner::FileScanner,
    ollama_client::OllamaClient,
    search::SearchEngine,
};
use md5;
use shared::types::Result;
use std::io::{self, Write};
use std::path::PathBuf;

pub struct RagService {
    scanner: FileScanner,
    storage: EmbeddingStorage,
    embedder: Embedder,
    client: OllamaClient,
    config: Config,
}

impl RagService {
    pub async fn new(
        root_path: &str,
        db_path: &str,
        client: OllamaClient,
        config: Config,
    ) -> Result<Self> {
        Ok(Self {
            scanner: FileScanner::new(root_path),
            storage: EmbeddingStorage::new(db_path).await?,
            embedder: Embedder::new(client.clone()),
            client: client,
            config,
        })
    }

    pub async fn build_index(&self) -> Result<()> {
        self.build_index_with_files(&self.scanner.collect_files()?)
            .await
    }

    pub async fn build_index_for_keywords(&self, keywords: &[String]) -> Result<()> {
        let mut files = self.scanner.collect_files()?;

        // Apply include/exclude patterns first
        files = self.filter_files_by_patterns(&files);

        // Filter by keywords if provided
        if !keywords.is_empty() {
            let filtered_keywords = self.filter_relevant_keywords(keywords);
            if !filtered_keywords.is_empty() {
                let keyword_lower: Vec<String> =
                    filtered_keywords.iter().map(|k| k.to_lowercase()).collect();
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
        }

        // Limit scanned files to reduce latency
        const MAX_FILES: usize = 200;
        if files.len() > MAX_FILES {
            // Sort by relevance (prioritize files with more keyword matches)
            let mut files_with_scores: Vec<(PathBuf, usize)> = files
                .into_iter()
                .map(|p| {
                    let score = if keywords.is_empty() {
                        1
                    } else {
                        let path_str = p.to_string_lossy().to_lowercase();
                        keywords
                            .iter()
                            .filter(|k| path_str.contains(&k.to_lowercase()))
                            .count()
                    };
                    (p, score)
                })
                .collect();

            files_with_scores.sort_by(|a, b| b.1.cmp(&a.1));
            files = files_with_scores
                .into_iter()
                .take(MAX_FILES)
                .map(|(p, _)| p)
                .collect();
        }

        self.build_index_with_files(&files).await
    }

    pub async fn query(&self, question: &str) -> Result<String> {
        self.query_with_feedback(question, "").await
    }

    pub async fn relevant_chunks(&self, question: &str, limit: usize) -> Result<Vec<String>> {
        let query_embedding = self.client.generate_embedding(question).await?;
        let all_embeddings = self.storage.get_all_embeddings().await?;
        let mut chunks =
            SearchEngine::find_relevant_chunks(&query_embedding, &all_embeddings, limit);

        if question.to_lowercase().contains("project")
            || question.to_lowercase().contains("what is")
        {
            if let Ok(readme_content) = std::fs::read_to_string("README.md") {
                chunks.insert(0, format!("FILE: README.md\n{}", readme_content));
            }
            let dir_overview = self.scanner.directory_overview(8, 2000);
            if !dir_overview.is_empty() {
                chunks.insert(0, format!("DIRECTORY TREE:\n{}", dir_overview));
            }
        }

        Ok(chunks)
    }

    pub async fn query_with_feedback(&self, question: &str, feedback: &str) -> Result<String> {
        let query_embedding = self.client.generate_embedding(question).await?;
        let all_embeddings = self.storage.get_all_embeddings().await?;
        let mut relevant_chunks =
            SearchEngine::find_relevant_chunks(&query_embedding, &all_embeddings, 50);

        // For project-level questions, include README and directory tree if available
        if question.to_lowercase().contains("project")
            || question.to_lowercase().contains("what is")
        {
            if let Ok(readme_content) = std::fs::read_to_string("README.md") {
                relevant_chunks.insert(0, format!("FILE: README.md\n{}", readme_content));
            }
            let dir_overview = self.scanner.directory_overview(8, 2000);
            if !dir_overview.is_empty() {
                relevant_chunks.insert(0, format!("DIRECTORY TREE:\n{}", dir_overview));
            }
        }

        let context = relevant_chunks.join("\n\n");
        if context.is_empty() {
            return Ok("No relevant code context found for this query.".to_string());
        }
        let feedback_part = if feedback.is_empty() {
            String::new()
        } else {
            format!("\n\nUser feedback for improvement: {}", feedback)
        };
        let prompt = format!("You are an expert software engineer. Based on the provided code context and directory structure, {}{} \n\nContext:\n{}\n\nProvide a concise summary that includes:\n- Project purpose\n- Main features\n- Technologies used\n- Architecture\n- Complete directory structure (copy exactly from the DIRECTORY TREE section in the context)\n\nBe accurate and base your answer only on the provided context. Do not invent or modify the directory structure.", question, feedback_part, context);
        self.client.generate_response(&prompt).await
    }

    fn filter_files_by_patterns(&self, files: &[PathBuf]) -> Vec<PathBuf> {
        files
            .iter()
            .filter(|path| {
                let path_str = path.to_string_lossy();

                // Check exclude patterns first
                for pattern in &self.config.rag_exclude_patterns {
                    if self.matches_pattern(&path_str, pattern) {
                        return false;
                    }
                }

                // Check include patterns
                if self.config.rag_include_patterns.is_empty() {
                    return true; // If no include patterns, include all (except excluded)
                }

                for pattern in &self.config.rag_include_patterns {
                    if self.matches_pattern(&path_str, pattern) {
                        return true;
                    }
                }

                false
            })
            .cloned()
            .collect()
    }

    fn matches_pattern(&self, path: &str, pattern: &str) -> bool {
        // Simple glob-like matching
        if pattern.contains("**") {
            // Handle directory patterns like "target/**"
            let prefix = pattern.trim_end_matches("/**").trim_end_matches("**");
            if prefix.is_empty() {
                return true; // ** matches everything
            }
            path.contains(&format!("/{}", prefix)) || path.starts_with(prefix)
        } else if pattern.starts_with("*.") {
            // File extension pattern like "*.rs"
            let ext = &pattern[2..];
            path.ends_with(&format!(".{}", ext))
        } else {
            // Exact match or contains
            path.contains(pattern)
        }
    }

    fn filter_relevant_keywords(&self, keywords: &[String]) -> Vec<String> {
        // Filter out common stop words and very short words
        let stop_words = [
            "the",
            "a",
            "an",
            "and",
            "or",
            "but",
            "in",
            "on",
            "at",
            "to",
            "for",
            "of",
            "with",
            "by",
            "is",
            "are",
            "was",
            "were",
            "be",
            "been",
            "being",
            "have",
            "has",
            "had",
            "do",
            "does",
            "did",
            "will",
            "would",
            "could",
            "should",
            "may",
            "might",
            "must",
            "can",
            "shall",
            "this",
            "that",
            "these",
            "those",
            "i",
            "you",
            "he",
            "she",
            "it",
            "we",
            "they",
            "me",
            "him",
            "her",
            "us",
            "them",
            "my",
            "your",
            "his",
            "its",
            "our",
            "their",
            "what",
            "which",
            "who",
            "when",
            "where",
            "why",
            "how",
            "all",
            "any",
            "both",
            "each",
            "few",
            "more",
            "most",
            "other",
            "some",
            "such",
            "no",
            "nor",
            "not",
            "only",
            "own",
            "same",
            "so",
            "than",
            "too",
            "very",
            "just",
            "now",
            "here",
            "there",
            "then",
            "once",
            "also",
            "explain",
            "available",
            "list",
            "show",
            "get",
            "find",
            "search",
            "query",
            "select",
        ];

        keywords
            .iter()
            .filter(|k| {
                let k_lower = k.to_lowercase();
                k.len() >= 3 && !stop_words.contains(&k_lower.as_str())
            })
            .cloned()
            .collect()
    }

    async fn build_index_with_files(&self, files: &[PathBuf]) -> Result<()> {
        let mut inputs: Vec<EmbeddingInput> = Vec::new();

        // Add a small directory overview chunk to help the model understand layout.
        let dir_overview = self.scanner.directory_overview(4, 400);
        if !dir_overview.is_empty() {
            let dir_hash = format!("{:x}", md5::compute(dir_overview.as_bytes()));
            let meta: Option<String> = self
                .storage
                .get_file_hash("__dir_overview__".to_string())
                .await?;
            if meta.as_deref() != Some(dir_hash.as_str()) {
                self.storage
                    .delete_embeddings_for_path("__dir_overview__".to_string())
                    .await?;
                inputs.push(EmbeddingInput {
                    id: format!("__dir_overview__:{dir_hash}"),
                    path: "__dir_overview__".to_string(),
                    text: format!("DIRECTORY TREE:\n{}", dir_overview),
                });
                self.storage
                    .upsert_file_hash("__dir_overview__".to_string(), dir_hash)
                    .await?;
            }
        }

        let scans = self.scanner.scan_paths(files)?;
        let total_scans = scans.len();
        let mut scanned = 0usize;
        let mut changed = 0usize;
        for scan in scans {
            scanned += 1;
            render_progress("RAG scan", scanned, total_scans);

            if scan.hash.is_empty() || scan.chunks.is_empty() {
                continue;
            }

            let previous_hash: Option<String> =
                self.storage.get_file_hash(scan.path.clone()).await?;
            if previous_hash.as_deref() == Some(scan.hash.as_str()) {
                continue;
            }
            changed += 1;

            // File changed; drop old embeddings for this path.
            self.storage
                .delete_embeddings_for_path(scan.path.clone())
                .await?;

            if let Some(ast) = code_ast::summarize_file(&scan.path) {
                inputs.push(EmbeddingInput {
                    id: format!("{}:__ast__", scan.path),
                    path: scan.path.clone(),
                    text: format!(
                        "FILE: {}\nLANGUAGE: {}\nAST STRUCTURE:\n{}",
                        scan.path, ast.language, ast.summary
                    ),
                });
            }

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
        finish_progress_line(&format!(
            "RAG scan complete: {} file(s), {} changed",
            scanned, changed
        ));

        if !inputs.is_empty() {
            let mut last_tick = 0usize;
            let embeddings = self
                .embedder
                .generate_embeddings_with_progress(&inputs, |done, total| {
                    if done == total || done.saturating_sub(last_tick) >= 16 {
                        render_progress("RAG embed", done, total);
                        last_tick = done;
                    }
                })
                .await?;
            finish_progress_line(&format!("RAG embed complete: {} chunk(s)", embeddings.len()));

            render_spinner("RAG store", "writing embeddings...");
            self.storage.insert_embeddings(embeddings).await?;
            finish_progress_line("RAG store complete");
        }
        Ok(())
    }
}

fn render_progress(stage: &str, done: usize, total: usize) {
    if total == 0 {
        return;
    }
    let pct = ((done as f64 / total as f64) * 100.0).round() as usize;
    print!("\r{stage}: {done}/{total} ({pct}%)");
    let _ = io::stdout().flush();
}

fn render_spinner(stage: &str, message: &str) {
    print!("\r{stage}: {message}");
    let _ = io::stdout().flush();
}

fn finish_progress_line(message: &str) {
    print!("\r{message}\n");
    let _ = io::stdout().flush();
}
