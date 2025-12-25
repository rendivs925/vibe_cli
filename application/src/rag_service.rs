use infrastructure::{
    config::Config,
    embedder::{Embedder, EmbeddingInput},
    embedding_storage::EmbeddingStorage,
    file_scanner::FileScanner,
    candle_inference::CandleInferenceService,
    search::SearchEngine,
    InferenceEngine,
};
use md5;
use shared::{types::Result, content_sanitizer::ContentSanitizer, secrets_detector::SecretsDetector};
use std::path::PathBuf;

pub struct RagService {
    scanner: FileScanner,
    storage: EmbeddingStorage,
    embedder: Embedder,
    inference_engine: infrastructure::InferenceEngine,
    config: Config,
    content_sanitizer: ContentSanitizer,
    secrets_detector: SecretsDetector,
}

impl RagService {
    pub async fn new(root_path: &str, db_path: &str, inference_engine: infrastructure::InferenceEngine, config: Config) -> Result<Self> {
        Ok(Self {
            scanner: FileScanner::new(root_path),
            storage: EmbeddingStorage::new(db_path).await?,
            embedder: Embedder::new_with_inference_engine(inference_engine.clone()),
            inference_engine,
            config,
            content_sanitizer: ContentSanitizer::new(),
            secrets_detector: SecretsDetector::new(),
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
                let keyword_lower: Vec<String> = filtered_keywords.iter().map(|k| k.to_lowercase()).collect();
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
                        keywords.iter()
                            .filter(|k| path_str.contains(&k.to_lowercase()))
                            .count()
                    };
                    (p, score)
                })
                .collect();

            files_with_scores.sort_by(|a, b| b.1.cmp(&a.1));
            files = files_with_scores.into_iter().take(MAX_FILES).map(|(p, _)| p).collect();
        }

        self.build_index_with_files(&files).await
    }

    pub async fn query(&self, question: &str) -> Result<String> {
        self.query_with_feedback(question, "").await
    }

    pub async fn query_with_feedback(&self, question: &str, feedback: &str) -> Result<String> {
        let query_embedding = self.inference_engine.generate_embeddings(question).await?;
        let all_embeddings = self.storage.get_all_embeddings().await?;
        let mut relevant_chunks =
            SearchEngine::find_relevant_chunks(&query_embedding, &all_embeddings, 50);

        // For project-level questions, include README and directory tree if available
        if question.to_lowercase().contains("project") || question.to_lowercase().contains("what is") {
            if let Ok(readme_content) = std::fs::read_to_string("README.md") {
                relevant_chunks.insert(0, format!("FILE: README.md\n{}", readme_content));
            }
            let dir_overview = self.scanner.directory_overview(8, 2000);
            if !dir_overview.is_empty() {
                relevant_chunks.insert(0, format!("DIRECTORY TREE:\n{}", dir_overview));
            }
        }

        // Check for secrets in retrieved content
        let mut contains_high_severity_secrets = false;
        for chunk in &relevant_chunks {
            if self.secrets_detector.contains_high_severity_secrets(chunk) {
                contains_high_severity_secrets = true;
                break;
            }
        }

        if contains_high_severity_secrets {
            return Ok("__SECRETS_DETECTED__: Retrieved content contains sensitive information. You may choose to continue with a sanitized version that masks secrets.".to_string());
        }

        // Sanitize all context chunks
        let sanitized_chunks: Vec<String> = relevant_chunks
            .into_iter()
            .map(|chunk| {
                // First sanitize content, then scan for secrets
                let sanitized = self.content_sanitizer.sanitize_rag_content(&chunk).content;
                // Scan again after sanitization (secrets should be masked)
                let secrets_scan = self.secrets_detector.scan_content(&sanitized);
                secrets_scan.sanitized_content
            })
            .collect();

        let context = sanitized_chunks.join("\n\n");
        if context.is_empty() {
            return Ok("No relevant code context found for this query.".to_string());
        }

        // Sanitize user inputs
        let sanitized_question = self.content_sanitizer.sanitize_user_input(question)
            .unwrap_or_else(|_| "Invalid question provided".to_string());

        let sanitized_feedback = if feedback.is_empty() {
            String::new()
        } else {
            match self.content_sanitizer.sanitize_user_input(feedback) {
                Ok(f) => format!("\n\nUser feedback for improvement: {}", f),
                Err(_) => String::new(),
            }
        };

        // Create secure prompt with sanitized content
        let context_refs: Vec<&str> = vec![&context];
        let prompt = self.content_sanitizer.create_secure_prompt(
            "Answer strictly from the provided context. If the context is insufficient, reply: 'Insufficient context to answer.'",
            &sanitized_question,
            &context_refs,
        ).unwrap_or_else(|_| format!(
            "SYSTEM: Answer strictly from the provided context. If insufficient, reply: 'Insufficient context to answer.'\n\nQUESTION: {}\n\nCONTEXT:\n{}\n\nRESPONSE:",
            sanitized_question, context
        ));
        self.inference_engine.generate(&prompt).await
    }

    /// Query with feedback, forcing continuation even if secrets are detected
    pub async fn query_with_feedback_force(&self, question: &str, feedback: &str) -> Result<String> {
        let query_embedding = self.inference_engine.generate_embeddings(question).await?;
        let all_embeddings = self.storage.get_all_embeddings().await?;
        let mut relevant_chunks =
            SearchEngine::find_relevant_chunks(&query_embedding, &all_embeddings, 50);

        // For project-level questions, include README and directory tree if available
        if question.to_lowercase().contains("project") || question.to_lowercase().contains("what is") {
            if let Ok(readme_content) = std::fs::read_to_string("README.md") {
                relevant_chunks.insert(0, format!("FILE: README.md\n{}", readme_content));
            }
            let dir_overview = self.scanner.directory_overview(8, 2000);
            if !dir_overview.is_empty() {
                relevant_chunks.insert(0, format!("DIRECTORY TREE:\n{}", dir_overview));
            }
        }

        // Force proceed with sanitization even if secrets detected

        // Sanitize all context chunks
        let sanitized_chunks: Vec<String> = relevant_chunks
            .into_iter()
            .map(|chunk| {
                // First sanitize content, then scan for secrets
                let sanitized = self.content_sanitizer.sanitize_rag_content(&chunk).content;
                // Scan again after sanitization (secrets should be masked)
                let secrets_scan = self.secrets_detector.scan_content(&sanitized);
                secrets_scan.sanitized_content
            })
            .collect();

        let context = sanitized_chunks.join("\n\n");
        if context.is_empty() {
            return Ok("No relevant code context found for this query.".to_string());
        }

        // Sanitize user inputs
        let sanitized_question = self.content_sanitizer.sanitize_user_input(question)
            .unwrap_or_else(|_| "Invalid question provided".to_string());

        let sanitized_feedback = if feedback.is_empty() {
            String::new()
        } else {
            match self.content_sanitizer.sanitize_user_input(feedback) {
                Ok(f) => format!("\n\nUser feedback for improvement: {}", f),
                Err(_) => String::new(),
            }
        };

        // Create secure prompt with sanitized content
        let context_refs: Vec<&str> = vec![&context];
        let prompt = self.content_sanitizer.create_secure_prompt(
            "Answer strictly from the provided context. If the context is insufficient, reply: 'Insufficient context to answer.'",
            &sanitized_question,
            &context_refs,
        ).unwrap_or_else(|_| format!(
            "SYSTEM: Answer strictly from the provided context. If insufficient, reply: 'Insufficient context to answer.'\n\nQUESTION: {}\n\nCONTEXT:\n{}\n\nRESPONSE:",
            sanitized_question, context
        ));
        self.inference_engine.generate(&prompt).await
    }

    fn filter_files_by_patterns(&self, files: &[PathBuf]) -> Vec<PathBuf> {
        files.iter()
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
            "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with", "by",
            "is", "are", "was", "were", "be", "been", "being", "have", "has", "had", "do", "does",
            "did", "will", "would", "could", "should", "may", "might", "must", "can", "shall",
            "this", "that", "these", "those", "i", "you", "he", "she", "it", "we", "they", "me",
            "him", "her", "us", "them", "my", "your", "his", "its", "our", "their", "what", "which",
            "who", "when", "where", "why", "how", "all", "any", "both", "each", "few", "more",
            "most", "other", "some", "such", "no", "nor", "not", "only", "own", "same", "so",
            "than", "too", "very", "just", "now", "here", "there", "then", "once", "also",
            "explain", "available", "list", "show", "get", "find", "search", "query", "select"
        ];

        keywords.iter()
            .filter(|k| {
                let k_lower = k.to_lowercase();
                k.len() >= 3 && !stop_words.contains(&k_lower.as_str())
            })
            .cloned()
            .collect()
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
