use crate::embedding_storage::EmbeddingStorage;
use crate::ollama_client::OllamaClient;
use shared::types::Result;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct HybridSearchConfig {
    pub semantic_weight: f32,
    pub keyword_weight: f32,
    pub min_keyword_matches: usize,
    pub initial_limit: usize,
    pub final_limit: usize,
}

impl Default for HybridSearchConfig {
    fn default() -> Self {
        Self {
            semantic_weight: 0.6,
            keyword_weight: 0.4,
            min_keyword_matches: 1,
            initial_limit: 50,
            final_limit: 10,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HybridSearchResult {
    pub text: String,
    pub path: String,
    pub semantic_score: f32,
    pub keyword_score: f32,
    pub combined_score: f32,
}

pub struct HybridSearch {
    storage: Arc<EmbeddingStorage>,
    client: OllamaClient,
    config: HybridSearchConfig,
}

impl HybridSearch {
    pub fn new(storage: Arc<EmbeddingStorage>, client: OllamaClient) -> Self {
        Self::with_config(storage, client, HybridSearchConfig::default())
    }

    pub fn with_config(
        storage: Arc<EmbeddingStorage>,
        client: OllamaClient,
        config: HybridSearchConfig,
    ) -> Self {
        Self {
            storage,
            client,
            config,
        }
    }

    pub fn config(&self) -> &HybridSearchConfig {
        &self.config
    }

    pub async fn search(&self, query: &str) -> Result<Vec<HybridSearchResult>> {
        let query_lower = query.to_lowercase();
        let query_terms: Vec<&str> = query_lower
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|s| !s.is_empty() && s.len() > 2)
            .collect();

        let semantic_results = self.semantic_search(query).await?;
        let keyword_results = self.keyword_search(&query_terms).await?;

        let combined = self.merge_results(semantic_results, keyword_results, &query_terms);

        Ok(combined)
    }

    async fn semantic_search(&self, query: &str) -> Result<Vec<(String, String, f32)>> {
        let query_embedding = self.client.generate_embedding(query).await?;
        let all_embeddings = self.storage.get_all_embeddings().await?;

        let mut results: Vec<(String, String, f32)> = Vec::new();

        for embedding in all_embeddings {
            let similarity = cosine_similarity(&query_embedding, &embedding.vector);
            if similarity > 0.3 {
                results.push((
                    embedding.text.clone(),
                    embedding.document_path.clone(),
                    similarity,
                ));
            }
        }

        results.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
        results.truncate(self.config.initial_limit);

        Ok(results)
    }

    async fn keyword_search(
        &self,
        query_terms: &[&str],
    ) -> Result<Vec<(String, String, f32)>> {
        if query_terms.is_empty() {
            return Ok(Vec::new());
        }

        let all_embeddings = self.storage.get_all_embeddings().await?;
        let mut results: Vec<(String, String, f32)> = Vec::new();

        for embedding in all_embeddings {
            let text_lower = embedding.text.to_lowercase();
            let path_lower = embedding.document_path.to_lowercase();

            let mut match_count = 0;
            for term in query_terms {
                if text_lower.contains(term) || path_lower.contains(term) {
                    match_count += 1;
                }
            }

            if match_count >= self.config.min_keyword_matches {
                let score = match_count as f32 / query_terms.len() as f32;
                results.push((
                    embedding.text.clone(),
                    embedding.document_path.clone(),
                    score,
                ));
            }
        }

        results.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
        results.truncate(self.config.initial_limit);

        Ok(results)
    }

    fn merge_results(
        &self,
        semantic_results: Vec<(String, String, f32)>,
        keyword_results: Vec<(String, String, f32)>,
        query_terms: &[&str],
    ) -> Vec<HybridSearchResult> {
        let mut combined_map: std::collections::HashMap<String, HybridSearchResult> =
            std::collections::HashMap::new();

        for (text, path, sem_score) in semantic_results {
            let key = format!("{}:{}", path, text.chars().take(100).collect::<String>());
            combined_map.insert(
                key,
                HybridSearchResult {
                    text: text.clone(),
                    path,
                    semantic_score: sem_score,
                    keyword_score: 0.0,
                    combined_score: 0.0,
                },
            );
        }

        for (text, path, kw_score) in keyword_results {
            let key = format!("{}:{}", path, text.chars().take(100).collect::<String>());
            if let Some(entry) = combined_map.get_mut(&key) {
                entry.keyword_score = kw_score;
            } else {
                combined_map.insert(
                    key,
                    HybridSearchResult {
                        text: text.clone(),
                        path,
                        semantic_score: 0.0,
                        keyword_score: kw_score,
                        combined_score: 0.0,
                    },
                );
            }
        }

        let sem_w = self.config.semantic_weight;
        let kw_w = self.config.keyword_weight;

        let mut results: Vec<HybridSearchResult> = combined_map
            .into_values()
            .map(|mut r| {
                let norm_sem = normalize_score(r.semantic_score);
                let norm_kw = normalize_score(r.keyword_score);
                r.combined_score = (norm_sem * sem_w) + (norm_kw * kw_w);
                r
            })
            .filter(|r| {
                let has_keyword_matches = query_terms.iter().any(|t| {
                    r.text.to_lowercase().contains(t) || r.path.to_lowercase().contains(t)
                });
                has_keyword_matches || r.semantic_score > 0.5
            })
            .collect();

        results.sort_by(|a, b| b.combined_score.partial_cmp(&a.combined_score).unwrap());
        results.truncate(self.config.final_limit);

        results
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if mag_a == 0.0 || mag_b == 0.0 {
        return 0.0;
    }

    dot_product / (mag_a * mag_b)
}

fn normalize_score(score: f32) -> f32 {
    score.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_normalize_score() {
        assert_eq!(normalize_score(0.5), 0.5);
        assert_eq!(normalize_score(1.5), 1.0);
        assert_eq!(normalize_score(-0.5), 0.0);
    }
}
