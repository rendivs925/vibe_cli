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
use serde::Deserialize;
use serde_json::Value;
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

#[derive(Debug, Deserialize)]
struct ToolPlan {
    tools: Vec<ToolCall>,
}

#[derive(Debug, Deserialize)]
struct ToolCall {
    name: String,
    #[serde(default)]
    args: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RagIntent {
    DirectoryStructure,
    ProjectOverview,
    Architecture,
    FileOrSymbolLookup,
    HowTo,
    Troubleshooting,
    General,
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
        let intent = self.analyze_intent(question);
        let snippets = self.collect_ranked_context(question, limit, intent).await?;
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
        let intent = self.analyze_intent(question);
        let dynamic_tool_evidence = self.run_dynamic_tool_plan(question).await?;

        let snippet_limit = match intent {
            RagIntent::ProjectOverview | RagIntent::Architecture => 32,
            RagIntent::FileOrSymbolLookup => 22,
            _ => 28,
        };
        let snippets = self.collect_ranked_context(question, snippet_limit, intent).await?;
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

        let tool_context = if dynamic_tool_evidence.is_empty() {
            String::new()
        } else {
            format!(
                "\nTool Evidence:\n{}",
                dynamic_tool_evidence
                    .iter()
                    .enumerate()
                    .map(|(idx, item)| format!("[T{}] {}", idx + 1, item))
                    .collect::<Vec<_>>()
                    .join("\n\n")
            )
        };

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

        let intent_instructions = match intent {
            RagIntent::ProjectOverview => "Intent: Project overview. Emphasize purpose, major components, architecture, and responsibilities.",
            RagIntent::Architecture => "Intent: Architecture analysis. Explain layers, dependencies, and core data/control flow only from evidence.",
            RagIntent::FileOrSymbolLookup => "Intent: File/symbol lookup. Focus on exact file paths, symbols, and where logic lives.",
            RagIntent::HowTo => "Intent: How-to guidance. Provide actionable steps tied directly to current code/config.",
            RagIntent::Troubleshooting => "Intent: Troubleshooting. Prioritize likely failure points and verification steps from evidence.",
            _ => "Intent: General codebase question.",
        };

        let prompt = format!(
            "Question:\n{question}\n{feedback}\n\n\
Intent:\n{intent}\n\n\
Evidence Snippets:\n{context}\n\n\
{tool_context}\n\
Instructions:\n\
1. Answer directly and accurately from evidence.\n\
2. For each important claim, include citations [Sx] and/or [Tx].\n\
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
            intent = intent_instructions,
            context = context,
            tool_context = tool_context
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
        intent: RagIntent,
    ) -> Result<Vec<ContextSnippet>> {
        let query_embedding = self.client.generate_embedding(question).await?;
        let all_embeddings = self.storage.get_all_embeddings().await?;
        let raw_limit = (limit.max(12) * 4).min(120);
        let mut chunks = SearchEngine::find_relevant_chunks(&query_embedding, &all_embeddings, raw_limit);

        if matches!(intent, RagIntent::ProjectOverview | RagIntent::Architecture | RagIntent::DirectoryStructure) {
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
            let score = score_chunk(question, &keywords, &path, &snippet_text, idx, intent);
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

    fn analyze_intent(&self, question: &str) -> RagIntent {
        let q = question.to_lowercase();
        if (q.contains("directory") && q.contains("structure")) || q.contains("folder structure") {
            return RagIntent::DirectoryStructure;
        }
        if q.contains("architecture") || q.contains("clean architecture") || q.contains("layer") {
            return RagIntent::Architecture;
        }
        if q.contains("project overview") || (self.is_project_level_question(question) && q.contains("explain")) {
            return RagIntent::ProjectOverview;
        }
        if q.contains("where is")
            || q.contains("which file")
            || q.contains("find function")
            || q.contains("symbol")
            || q.contains("class ")
            || q.contains("trait ")
            || q.contains("struct ")
        {
            return RagIntent::FileOrSymbolLookup;
        }
        if q.contains("how to")
            || q.contains("how do i")
            || q.contains("steps")
            || q.contains("configure")
            || q.contains("setup")
        {
            return RagIntent::HowTo;
        }
        if q.contains("error")
            || q.contains("failing")
            || q.contains("bug")
            || q.contains("issue")
            || q.contains("not working")
            || q.contains("debug")
        {
            return RagIntent::Troubleshooting;
        }
        if self.is_project_level_question(question) {
            return RagIntent::ProjectOverview;
        }
        RagIntent::General
    }

    async fn run_dynamic_tool_plan(&self, question: &str) -> Result<Vec<String>> {
        let tool_catalog = self.tool_catalog();
        let planner_system = "You are a retrieval planner. Choose the minimum useful tools.\n\
Return strict JSON only in the form:\n\
{\"tools\":[{\"name\":\"<tool>\",\"args\":{...}}]}\n\
No markdown or prose.";
        let planner_prompt = format!(
            "Question: {q}\n\nAvailable tools:\n{catalog}\n\n\
Rules:\n\
- Choose up to 6 tools.\n\
- Prefer semantic_chunks first for code understanding.\n\
- Use directory_tree for structure questions.\n\
- Use ast_summary only when a concrete file path is known.\n\
- Use read_file only with concrete path.\n",
            q = question.trim(),
            catalog = tool_catalog
        );

        let planner_output = self
            .client
            .generate_response_with_system(&planner_prompt, planner_system)
            .await
            .unwrap_or_default();
        let planned = parse_tool_plan(&planner_output).unwrap_or_else(|| {
            vec![ToolCall {
                name: "semantic_chunks".to_string(),
                args: Value::Object(serde_json::Map::new()),
            }]
        });

        let mut evidence = Vec::new();
        for tool in planned.into_iter().take(6) {
            if let Some(out) = self.execute_tool_call(question, &tool).await? {
                evidence.push(out);
            }
        }
        Ok(evidence)
    }

    async fn execute_tool_call(&self, question: &str, call: &ToolCall) -> Result<Option<String>> {
        let name = call.name.trim().to_lowercase();
        match name.as_str() {
            "semantic_chunks" => {
                let limit = call
                    .args
                    .get("limit")
                    .and_then(Value::as_u64)
                    .map(|v| v as usize)
                    .unwrap_or(8)
                    .clamp(3, 20);
                let snippets = self
                    .collect_ranked_context(question, limit, self.analyze_intent(question))
                    .await?;
                if snippets.is_empty() {
                    return Ok(None);
                }
                let body = snippets
                    .iter()
                    .map(|s| format!("FILE: {}\n{}", s.path, s.text))
                    .collect::<Vec<_>>()
                    .join("\n\n---\n\n");
                Ok(Some(format!("semantic_chunks(limit={}):\n{}", limit, body)))
            }
            "directory_tree" => {
                let max_depth = call
                    .args
                    .get("max_depth")
                    .and_then(Value::as_u64)
                    .map(|v| v as usize)
                    .unwrap_or(8)
                    .clamp(2, 12);
                let max_entries = call
                    .args
                    .get("max_entries")
                    .and_then(Value::as_u64)
                    .map(|v| v as usize)
                    .unwrap_or(2000)
                    .clamp(50, 5000);
                let tree = self.scanner.directory_overview(max_depth, max_entries);
                if tree.trim().is_empty() {
                    return Ok(None);
                }
                Ok(Some(format!(
                    "directory_tree(max_depth={}, max_entries={}):\n{}",
                    max_depth, max_entries, tree
                )))
            }
            "list_files" => {
                let max_results = call
                    .args
                    .get("max_results")
                    .and_then(Value::as_u64)
                    .map(|v| v as usize)
                    .unwrap_or(200)
                    .clamp(20, 2000);
                let include_ext = parse_string_list_arg(call.args.get("extensions"));
                let mut paths = self
                    .scanner
                    .collect_files()?
                    .into_iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect::<Vec<_>>();
                paths.sort();
                if !include_ext.is_empty() {
                    let ext_set = include_ext
                        .iter()
                        .map(|s| s.trim_start_matches('.').to_lowercase())
                        .collect::<HashSet<_>>();
                    paths.retain(|p| {
                        p.rsplit('.')
                            .next()
                            .map(|ext| ext_set.contains(&ext.to_lowercase()))
                            .unwrap_or(false)
                    });
                }
                paths.truncate(max_results);
                if paths.is_empty() {
                    return Ok(None);
                }
                Ok(Some(format!(
                    "list_files(max_results={}):\n{}",
                    max_results,
                    paths.join("\n")
                )))
            }
            "language_inventory" => {
                let files = self.scanner.collect_files()?;
                let mut by_ext: HashMap<String, usize> = HashMap::new();
                for path in files {
                    let s = path.to_string_lossy();
                    if let Some(ext) = s.rsplit('.').next() {
                        *by_ext.entry(ext.to_lowercase()).or_insert(0) += 1;
                    }
                }
                if by_ext.is_empty() {
                    return Ok(None);
                }
                let mut items = by_ext.into_iter().collect::<Vec<_>>();
                items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
                let lines = items
                    .into_iter()
                    .map(|(ext, count)| format!("{}. {}", ext, count))
                    .collect::<Vec<_>>()
                    .join("\n");
                Ok(Some(format!("language_inventory:\n{}", lines)))
            }
            "read_file" => {
                let Some(path) = call.args.get("path").and_then(Value::as_str) else {
                    return Ok(None);
                };
                let max_chars = call
                    .args
                    .get("max_chars")
                    .and_then(Value::as_u64)
                    .map(|v| v as usize)
                    .unwrap_or(6000)
                    .clamp(500, 20000);
                let text = std::fs::read_to_string(path).ok();
                if let Some(content) = text {
                    let body = truncate_chars(&content, max_chars);
                    return Ok(Some(format!("read_file(path={}):\n{}", path, body)));
                }
                Ok(None)
            }
            "preview_file" => {
                let Some(path) = call.args.get("path").and_then(Value::as_str) else {
                    return Ok(None);
                };
                let lines = call
                    .args
                    .get("lines")
                    .and_then(Value::as_u64)
                    .map(|v| v as usize)
                    .unwrap_or(120)
                    .clamp(20, 400);
                let offset = call
                    .args
                    .get("offset")
                    .and_then(Value::as_u64)
                    .map(|v| v as usize)
                    .unwrap_or(0);
                let Ok(content) = std::fs::read_to_string(path) else {
                    return Ok(None);
                };
                let preview = content
                    .lines()
                    .skip(offset)
                    .take(lines)
                    .enumerate()
                    .map(|(idx, line)| format!("{}: {}", offset + idx + 1, line))
                    .collect::<Vec<_>>()
                    .join("\n");
                if preview.trim().is_empty() {
                    return Ok(None);
                }
                Ok(Some(format!(
                    "preview_file(path={}, lines={}, offset={}):\n{}",
                    path, lines, offset, preview
                )))
            }
            "ast_summary" => {
                let Some(path) = call.args.get("path").and_then(Value::as_str) else {
                    return Ok(None);
                };
                if let Some(ast) = code_ast::summarize_file(path) {
                    return Ok(Some(format!(
                        "ast_summary(path={}, language={}):\n{}",
                        path, ast.language, ast.summary
                    )));
                }
                Ok(None)
            }
            "ast_symbols" => {
                let Some(path) = call.args.get("path").and_then(Value::as_str) else {
                    return Ok(None);
                };
                if let Some(ast) = code_ast::summarize_file(path) {
                    let symbols = ast
                        .summary
                        .lines()
                        .filter(|line| line.contains("_names:"))
                        .collect::<Vec<_>>()
                        .join("\n");
                    let body = if symbols.trim().is_empty() {
                        ast.summary
                    } else {
                        symbols
                    };
                    return Ok(Some(format!(
                        "ast_symbols(path={}, language={}):\n{}",
                        path, ast.language, body
                    )));
                }
                Ok(None)
            }
            "readme" => {
                let text = std::fs::read_to_string("README.md").ok();
                if let Some(content) = text {
                    return Ok(Some(format!(
                        "readme:\n{}",
                        truncate_chars(&content, 9000)
                    )));
                }
                Ok(None)
            }
            "search_content" => {
                let Some(pattern) = call.args.get("pattern").and_then(Value::as_str) else {
                    return Ok(None);
                };
                let max_results = call
                    .args
                    .get("max_results")
                    .and_then(Value::as_u64)
                    .map(|v| v as usize)
                    .unwrap_or(60)
                    .clamp(5, 300);
                let pattern_l = pattern.to_lowercase();
                let mut hits = Vec::new();
                for path in self.scanner.collect_files()? {
                    let spath = path.to_string_lossy().to_string();
                    if let Ok(content) = std::fs::read_to_string(&spath) {
                        for (ln, line) in content.lines().enumerate() {
                            if line.to_lowercase().contains(&pattern_l) {
                                hits.push(format!("{}:{}: {}", spath, ln + 1, line.trim()));
                                if hits.len() >= max_results {
                                    break;
                                }
                            }
                        }
                    }
                    if hits.len() >= max_results {
                        break;
                    }
                }
                if hits.is_empty() {
                    return Ok(None);
                }
                Ok(Some(format!(
                    "search_content(pattern={}, max_results={}):\n{}",
                    pattern,
                    max_results,
                    hits.join("\n")
                )))
            }
            "grep_paths" => {
                let Some(pattern) = call.args.get("pattern").and_then(Value::as_str) else {
                    return Ok(None);
                };
                let max_results = call
                    .args
                    .get("max_results")
                    .and_then(Value::as_u64)
                    .map(|v| v as usize)
                    .unwrap_or(40)
                    .clamp(5, 200);
                let mut results = Vec::new();
                for path in self.scanner.collect_files()? {
                    let s = path.to_string_lossy().to_string();
                    if s.to_lowercase().contains(&pattern.to_lowercase()) {
                        results.push(s);
                    }
                    if results.len() >= max_results {
                        break;
                    }
                }
                if results.is_empty() {
                    return Ok(None);
                }
                Ok(Some(format!(
                    "grep_paths(pattern={}, max_results={}):\n{}",
                    pattern,
                    max_results,
                    results.join("\n")
                )))
            }
            _ => Ok(None),
        }
    }

    fn tool_catalog(&self) -> String {
        [
            "- semantic_chunks {limit?: int} -> ranked semantic snippets from vector index",
            "- directory_tree {max_depth?: int, max_entries?: int} -> filesystem tree",
            "- list_files {max_results?: int, extensions?: [string]} -> list files (optionally filtered by extension)",
            "- language_inventory {} -> count files by extension/language hints",
            "- read_file {path: string, max_chars?: int} -> file content",
            "- preview_file {path: string, lines?: int, offset?: int} -> line-numbered preview",
            "- ast_summary {path: string} -> AST summary for a source file",
            "- ast_symbols {path: string} -> extracted symbol-name lists from AST summary",
            "- readme {} -> README content",
            "- search_content {pattern: string, max_results?: int} -> text search in file contents",
            "- grep_paths {pattern: string, max_results?: int} -> find matching file paths",
        ]
        .join("\n")
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
    intent: RagIntent,
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
    if intent == RagIntent::FileOrSymbolLookup && (path_l.ends_with(".rs") || path_l.ends_with(".py") || path_l.ends_with(".ts")) {
        score += 12;
    }
    if intent == RagIntent::Architecture && chunk_l.contains("ast structure") {
        score += 16;
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

fn parse_tool_plan(text: &str) -> Option<Vec<ToolCall>> {
    if let Ok(plan) = serde_json::from_str::<ToolPlan>(text.trim()) {
        return Some(plan.tools);
    }

    if let Some(json) = extract_json_object(text) {
        if let Ok(plan) = serde_json::from_str::<ToolPlan>(json) {
            return Some(plan.tools);
        }
    }
    None
}

fn extract_json_object(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let mut depth = 0_i32;
    let mut start = None;
    let mut in_string = false;
    let mut escape = false;

    for (i, b) in bytes.iter().enumerate() {
        if escape {
            escape = false;
            continue;
        }
        match *b {
            b'\\' if in_string => escape = true,
            b'"' => in_string = !in_string,
            b'{' if !in_string => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            b'}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    if let Some(s) = start {
                        return Some(&text[s..=i]);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_string_list_arg(value: Option<&Value>) -> Vec<String> {
    let Some(value) = value else {
        return Vec::new();
    };
    if let Some(array) = value.as_array() {
        return array
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
    }
    if let Some(single) = value.as_str() {
        return single
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
    }
    Vec::new()
}
