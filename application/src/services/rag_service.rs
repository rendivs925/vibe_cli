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
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::path::PathBuf;

pub struct RagService {
    scanner: FileScanner,
    storage: EmbeddingStorage,
    embedder: Embedder,
    client: OllamaClient,
    config: Config,
}

struct ContextSnippet {
    id: String,
    path: String,
    text: String,
    score: i32,
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
        let snippets = self.collect_ranked_context(question, limit).await?;
        Ok(snippets
            .into_iter()
            .map(|snippet| {
                format!(
                    "[{}] FILE: {}\n{}",
                    snippet.id, snippet.path, snippet.text
                )
            })
            .collect())
    }

    pub async fn query_with_feedback(&self, question: &str, feedback: &str) -> Result<String> {
        let snippets = self.collect_ranked_context(question, 28).await?;
        if snippets.is_empty() {
            return Ok("No relevant code context found for this query.".to_string());
        }

        let context = snippets
            .iter()
            .map(|snippet| {
                format!(
                    "[{id}] FILE: {path}\n{body}",
                    id = snippet.id,
                    path = snippet.path,
                    body = snippet.text
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let feedback_part = if feedback.is_empty() {
            String::new()
        } else {
            format!("\nUser feedback: {}", feedback)
        };
        let system = "You are a codebase analysis assistant.\n\
Use only the provided context snippets.\n\
Never invent files, APIs, behavior, architecture, or versions.\n\
If context is insufficient, explicitly say so and ask for the most useful next query.\n\
Cite evidence snippet IDs like [S1], [S2] for every key claim.";

        let prompt = format!(
            "Question:\n{question}\n{feedback}\n\n\
Evidence Snippets:\n{context}\n\n\
Instructions:\n\
1. Answer directly and accurately from evidence.\n\
2. For each important claim, include snippet citations [Sx].\n\
3. If a claim is uncertain, mark it as uncertain.\n\
4. Do not claim anything not supported by snippets.\n\
5. If asked for project overview, cover: purpose, main components, architecture, and key flows.\n\n\
Output format:\n\
Answer: <concise answer with citations>\n\
Evidence:\n\
- [Sx] <why this snippet supports answer>\n\
Gaps:\n\
- <missing info or 'None'>\n\
Confidence: <High|Medium|Low>\n",
            question = question.trim(),
            feedback = feedback_part,
            context = context
        );

        self.client.generate_response_with_system(&prompt, system).await
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

    async fn collect_ranked_context(
        &self,
        question: &str,
        limit: usize,
    ) -> Result<Vec<ContextSnippet>> {
        let query_embedding = self.client.generate_embedding(question).await?;
        let all_embeddings = self.storage.get_all_embeddings().await?;
        let raw_limit = (limit.max(12) * 4).min(120);
        let mut chunks = SearchEngine::find_relevant_chunks(&query_embedding, &all_embeddings, raw_limit);

        if self.is_project_level_question(question) {
            if let Ok(readme_content) = std::fs::read_to_string("README.md") {
                chunks.insert(0, format!("FILE: README.md\n{}", readme_content));
            }
            let dir_overview = self.scanner.directory_overview(8, 2000);
            if !dir_overview.is_empty() {
                chunks.insert(0, format!("FILE: __dir_overview__\nDIRECTORY TREE:\n{}", dir_overview));
            }
        }

        let keywords = self.extract_query_keywords(question);
        let mut seen_content = HashSet::new();
        let mut per_path_count: HashMap<String, usize> = HashMap::new();
        let mut scored = Vec::new();

        for (idx, chunk) in chunks.into_iter().enumerate() {
            let normalized = chunk.trim().to_string();
            if normalized.is_empty() || !seen_content.insert(normalized.clone()) {
                continue;
            }

            let path = extract_path_from_chunk(&normalized).unwrap_or_else(|| "__unknown__".to_string());
            let path_key = path.to_lowercase();
            let count = per_path_count.entry(path_key).or_insert(0);
            if *count >= 3 {
                continue;
            }
            *count += 1;

            let snippet_text = truncate_chars(&normalized, 2200);
            let score = score_chunk(question, &keywords, &path, &snippet_text, idx);
            scored.push(ContextSnippet {
                id: format!("S{}", idx + 1),
                path,
                text: snippet_text,
                score,
            });
        }

        scored.sort_by(|a, b| b.score.cmp(&a.score));

        let mut selected = Vec::new();
        let mut total_chars = 0usize;
        let max_chars = 24_000usize;
        for snippet in scored {
            if selected.len() >= limit {
                break;
            }
            let next = snippet.text.len() + snippet.path.len() + snippet.id.len() + 16;
            if total_chars + next > max_chars {
                break;
            }
            total_chars += next;
            selected.push(snippet);
        }
        Ok(selected)
    }

    fn extract_query_keywords(&self, question: &str) -> Vec<String> {
        let tokens = question
            .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
            .filter(|token| !token.is_empty())
            .map(|token| token.to_string())
            .collect::<Vec<_>>();
        self.filter_relevant_keywords(&tokens)
    }

    fn is_project_level_question(&self, question: &str) -> bool {
        let q = question.to_lowercase();
        q.contains("project")
            || q.contains("what is")
            || q.contains("overview")
            || q.contains("architecture")
            || q.contains("codebase")
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

fn extract_path_from_chunk(chunk: &str) -> Option<String> {
    for line in chunk.lines().take(6) {
        if let Some(path) = line.strip_prefix("FILE: ") {
            return Some(path.trim().to_string());
        }
    }
    None
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut result = String::new();
    for ch in text.chars().take(max_chars) {
        result.push(ch);
    }
    result.push_str("\n...[truncated]");
    result
}

fn score_chunk(
    question: &str,
    keywords: &[String],
    path: &str,
    chunk: &str,
    original_rank: usize,
) -> i32 {
    let q = question.to_lowercase();
    let path_l = path.to_lowercase();
    let chunk_l = chunk.to_lowercase();
    let mut score = 100_i32.saturating_sub(original_rank as i32);

    if chunk_l.contains("ast structure") {
        score += 40;
    }
    if chunk_l.contains("directory tree") {
        score += 24;
    }
    if path_l.contains("readme") {
        score += 20;
    }
    if q.contains("architecture") && (chunk_l.contains("mod ") || chunk_l.contains("struct ") || chunk_l.contains("trait ")) {
        score += 18;
    }

    let mut keyword_hits = 0_i32;
    for keyword in keywords {
        let key = keyword.to_lowercase();
        if path_l.contains(&key) {
            keyword_hits += 2;
        }
        if chunk_l.contains(&key) {
            keyword_hits += 1;
        }
    }
    score + keyword_hits.min(30)
}
